#!/usr/bin/env bash
set -euo pipefail

workflow=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/.github/workflows/release.yml
fail() { echo "FAIL: $*" >&2; exit 1; }

grep -Fq 'sql-judge-linux-x86_64.SHA256SUMS' "$workflow" || fail "stable checksum asset name missing"
grep -Fq 'sha256sum' "$workflow" || fail "checksum generation missing"
grep -Fq 'gh release upload' "$workflow" || fail "checksum upload missing"
grep -Fq "if: matrix.platform == 'ubuntu-22.04'" "$workflow" || fail "checksum step is not Linux-only"
grep -Fq 'uses: tauri-apps/tauri-action@action-v1.0.0' "$workflow" || fail "release action must use v1"
grep -Fq 'releaseDraft: false' "$workflow" || fail "published releases cannot be uploaded as drafts"
grep -Fq 'releaseAssetNamePattern:' "$workflow" || fail "release asset name input missing"
grep -Fq 'uploadUpdaterJson: false' "$workflow" || fail "updater upload must stay disabled"
echo 'PASS: release workflow checksum contract'
