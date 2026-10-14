import tomllib
import unittest
from pathlib import Path


class WindowsSqliteConfigTest(unittest.TestCase):
    def test_rusqlite_bundles_sqlite_for_windows_release_builds(self):
        manifest = Path("crates/app-storage/Cargo.toml")
        config = tomllib.loads(manifest.read_text())
        dependency = config["dependencies"]["rusqlite"]
        self.assertIsInstance(dependency, dict)
        self.assertIn("bundled", dependency["features"])


if __name__ == "__main__":
    unittest.main()
