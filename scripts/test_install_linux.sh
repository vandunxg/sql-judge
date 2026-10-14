#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
INSTALLER="$ROOT/scripts/install-linux.sh"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "PASS: $*"; }

if ! bash "$INSTALLER" --help >"$TMP/help" 2>&1; then
  fail "--help should succeed"
fi
grep -Fq -- '--user' "$TMP/help" || fail "help does not describe --user"
pass "help"

mkdir -p "$TMP/bin"
cat > "$TMP/bin/uname" <<'STUB'
#!/usr/bin/env bash
case "${1:-}" in
  -s) printf '%s\n' Linux ;;
  -m) printf '%s\n' aarch64 ;;
esac
STUB
chmod +x "$TMP/bin/uname"
if PATH="$TMP/bin:$PATH" bash "$INSTALLER" --user >"$TMP/arch.out" 2>&1; then
  fail "unsupported architecture should fail"
fi
grep -Fq 'Unsupported Linux architecture' "$TMP/arch.out" || fail "unsupported architecture message missing"
pass "architecture guard"

mkdir -p "$TMP/home/.local/bin" "$TMP/home/.local/share/applications"
cat > "$TMP/bin/uname" <<'STUB'
#!/usr/bin/env bash
case "${1:-}" in
  -s) printf '%s\n' Linux ;;
  -m) printf '%s\n' x86_64 ;;
esac
STUB
cat > "$TMP/bin/curl" <<'STUB'
#!/usr/bin/env bash
set -eu
out=
url=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
case "$url" in
  */releases/latest) cat "$FIXTURE_ROOT/release.json" >"$out" ;;
  *SHA256SUMS) cat "$FIXTURE_ROOT/checksums" >"$out" ;;
  *linux-x86_64.AppImage) cat "$FIXTURE_ROOT/appimage" >"$out" ;;
  *) exit 1 ;;
esac
STUB
chmod +x "$TMP/bin/uname" "$TMP/bin/curl"
printf 'verified appimage\n' > "$TMP/appimage"
sha=$(sha256sum "$TMP/appimage" | awk '{print $1}')
cat > "$TMP/release.json" <<EOF
{"tag_name": "v0.1.0", "assets": [{"name": "sql-judge-0.1.0-linux-x86_64.AppImage", "browser_download_url": "https://example.invalid/sql-judge-0.1.0-linux-x86_64.AppImage"},{"name": "sql-judge-linux-x86_64.SHA256SUMS", "browser_download_url": "https://example.invalid/sql-judge-linux-x86_64.SHA256SUMS"}]}
EOF
printf '%s  sql-judge-0.1.0-linux-x86_64.AppImage\n' "$sha" > "$TMP/checksums"
if ! HOME="$TMP/home" FIXTURE_ROOT="$TMP" PATH="$TMP/bin:$PATH" bash "$INSTALLER" --user >"$TMP/install.out" 2>&1; then
  cat "$TMP/install.out" >&2
  fail "verified user installation should succeed"
fi
test -x "$TMP/home/.local/bin/sql-judge" || fail "user launcher missing"
test -f "$TMP/home/.local/share/applications/sql-judge.desktop" || fail "desktop entry missing"
pass "verified user installation"

printf 'bad  sql-judge-0.1.0-linux-x86_64.AppImage\n' > "$TMP/checksums"
rm -f "$TMP/home/.local/bin/sql-judge"
if HOME="$TMP/home" FIXTURE_ROOT="$TMP" PATH="$TMP/bin:$PATH" bash "$INSTALLER" --user >"$TMP/checksum.out" 2>&1; then
  fail "checksum mismatch should fail"
fi
grep -Fq 'checksum' "$TMP/checksum.out" || fail "checksum failure message missing"
test ! -e "$TMP/home/.local/bin/sql-judge" || fail "checksum failure installed a file"
pass "checksum failure"
