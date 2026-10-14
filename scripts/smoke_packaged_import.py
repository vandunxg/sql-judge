"""Imports a pack through the packaged app's real UI over WebDriver (tauri-driver).

Usage: python scripts/smoke_packaged_import.py --app <installed executable> --pack <pack path>

It types the pack path into the Packs screen, presses "Review pack" and "Install pack",
and waits for the review heading to report the install. Exits non-zero with the page text
when the import fails or times out.
"""

import argparse
import json
import re
import sys
import time
import urllib.error
import urllib.request

INSTALLED = re.compile(r" is (already )?installed$")
NOT_INSTALLED = re.compile(r" was not installed$")


class WebDriver:
    def __init__(self, url):
        self.url = url.rstrip("/")
        self.session = None

    def call(self, method, path, body=None):
        request = urllib.request.Request(
            f"{self.url}{path}",
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method=method,
        )
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return json.loads(response.read() or b"{}").get("value")
        except urllib.error.HTTPError as error:
            raise RuntimeError(f"{method} {path}: {error.read().decode(errors='replace')}") from error

    def start(self, application):
        capabilities = {"alwaysMatch": {"tauri:options": {"application": application}}}
        value = self.call("POST", "/session", {"capabilities": capabilities})
        self.session = value["sessionId"]

    def stop(self):
        if self.session:
            self.call("DELETE", f"/session/{self.session}")

    def run(self, script):
        return self.call("POST", f"/session/{self.session}/execute/sync", {"script": script, "args": []})

    def find(self, xpath):
        try:
            value = self.call(
                "POST", f"/session/{self.session}/element", {"using": "xpath", "value": xpath}
            )
        except RuntimeError:
            return None
        return next(iter(value.values()))

    def click(self, element):
        self.call("POST", f"/session/{self.session}/element/{element}/click", {})

    def type(self, element, text):
        self.call("POST", f"/session/{self.session}/element/{element}/value", {"text": text})


def wait_for(description, probe, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        found = probe()
        if found:
            return found
        time.sleep(1)
    raise TimeoutError(f"timed out after {timeout}s waiting for {description}")


def enabled_button(driver, label):
    return driver.find(f"//button[normalize-space()='{label}' and not(@disabled)]")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", required=True)
    parser.add_argument("--pack", required=True)
    parser.add_argument("--driver-url", default="http://127.0.0.1:4444")
    args = parser.parse_args()

    driver = WebDriver(args.driver_url)
    driver.start(args.app)
    try:
        wait_for("the app shell", lambda: driver.run("return document.readyState === 'complete'"), 60)
        driver.run("window.location.hash = '#packs'")
        field = wait_for(
            "the pack path field",
            lambda: driver.find("//input[@aria-label='Local pack path' and not(@disabled)]"),
            60,
        )
        driver.type(field, args.pack)
        driver.click(wait_for("Review pack", lambda: enabled_button(driver, "Review pack"), 30))
        driver.click(wait_for("Install pack", lambda: enabled_button(driver, "Install pack"), 30))

        def outcome():
            headings = driver.run(
                "return Array.from(document.querySelectorAll('h1'), (h) => h.innerText.trim())"
            )
            return next(
                (h for h in headings or [] if INSTALLED.search(h) or NOT_INSTALLED.search(h)),
                None,
            )

        heading = wait_for("the install result", outcome, 300)
        page = driver.run("return document.body.innerText")
        print(heading)
        if NOT_INSTALLED.search(heading):
            print(page, file=sys.stderr)
            return 1
        print(page[:2000])
        return 0
    except Exception:
        try:
            print(driver.run("return document.body.innerText"), file=sys.stderr)
        except Exception:
            pass
        raise
    finally:
        driver.stop()


if __name__ == "__main__":
    sys.exit(main())
