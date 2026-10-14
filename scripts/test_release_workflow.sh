#!/usr/bin/env bash
set -euo pipefail

workflow=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/.github/workflows/release.yml
fail() { echo "FAIL: $*" >&2; exit 1; }

grep -Fq 'sql-judge-linux-x86_64.SHA256SUMS' "$workflow" || fail "stable checksum asset name missing"
grep -Fq 'sha256sum' "$workflow" || fail "checksum generation missing"
grep -Fq 'gh release upload' "$workflow" || fail "checksum upload missing"
grep -Fq "if: matrix.platform == 'ubuntu-22.04'" "$workflow" || fail "checksum step is not Linux-only"
echo 'PASS: release workflow checksum contract'
