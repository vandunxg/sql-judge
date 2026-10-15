#!/usr/bin/env bash
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fail() { echo "FAIL: $*" >&2; exit 1; }

version=$(sed -nE 's/^  "version": "([^"]+)",$/\1/p' "$root/src-tauri/tauri.conf.json")
[ -n "$version" ] || fail "src-tauri/tauri.conf.json has no version"

cargo_version=$(sed -nE '/^\[package\]/,/^\[/ s/^version = "([^"]+)"$/\1/p' "$root/src-tauri/Cargo.toml")
[ "$cargo_version" = "$version" ] || fail "src-tauri/Cargo.toml is $cargo_version, expected $version"

lock_version=$(awk '/^name = "tauri-native"$/ { getline; print }' "$root/src-tauri/Cargo.lock" | sed -nE 's/^version = "([^"]+)"$/\1/p')
[ "$lock_version" = "$version" ] || fail "src-tauri/Cargo.lock is $lock_version, expected $version"

[ -f "$root/CHANGELOG.md" ] || fail "CHANGELOG.md is missing"
newest=$(grep -m1 '^## ' "$root/CHANGELOG.md" || true)
grep -Eq "^## \[${version//./\\.}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" <<<"$newest" ||
  fail "newest CHANGELOG.md section is '$newest', expected '## [$version] - YYYY-MM-DD'"

echo "PASS: release version $version is consistent"
