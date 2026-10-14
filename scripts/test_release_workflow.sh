#!/usr/bin/env bash
set -euo pipefail

workflow=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/.github/workflows/release.yml
fail() { echo "FAIL: $*" >&2; exit 1; }

grep -Fq 'sql-judge-linux-x86_64.SHA256SUMS' "$workflow" || fail "stable checksum asset name missing"
grep -Fq 'sha256sum' "$workflow" || fail "checksum generation missing"
grep -Fq 'gh release upload' "$workflow" || fail "checksum upload missing"
grep -Fq "if: matrix.platform == 'ubuntu-22.04'" "$workflow" || fail "checksum step is not Linux-only"
grep -Fq 'prepare-release:' "$workflow" || fail "release must be prepared once before the matrix"
grep -Fq 'needs: prepare-release' "$workflow" || fail "matrix must wait for the prepared release"
grep -Eq '^        uses: tauri-apps/tauri-action@action-v1\.0\.0$' "$workflow" || fail "release action must use v1"
grep -Fq 'releaseId: ${{ needs.prepare-release.outputs.release_id }}' "$workflow" || fail "matrix must upload to the prepared release ID"
grep -Eq '^          releaseAssetNamePattern: \$\{\{ matrix\.assetPattern \}\}$' "$workflow" || fail "release asset pattern input missing"
grep -Eq '^          uploadUpdaterJson: false$' "$workflow" || fail "updater upload must stay disabled"
! grep -Fq 'releaseBody:' "$workflow" || fail "matrix must not overwrite existing release notes"
! grep -Fq 'generateReleaseNotes:' "$workflow" || fail "matrix must not regenerate existing release notes"
grep -Fq 'publish-release:' "$workflow" || fail "release must publish after all matrix uploads"
grep -Fq 'gh release create "$tag" --title "$tag" --generate-notes --draft' "$workflow" || fail "new releases must start as drafts"
grep -Fq 'gh release edit "${{ needs.prepare-release.outputs.tag }}" --draft=false' "$workflow" || fail "release must publish only after uploads"
grep -Fq 'assetPattern: "[name]-[version]-linux-x86_64[ext]"' "$workflow" || fail "Linux asset names must match the installer architecture"
grep -Fq 'sql-judge-${{ steps.tauri.outputs.appVersion }}-linux-x86_64.AppImage' "$workflow" || fail "AppImage checksum name must match the uploaded asset"
grep -Fq 'sql-judge-${{ steps.tauri.outputs.appVersion }}-linux-x86_64.deb' "$workflow" || fail "DEB checksum name must match the uploaded asset"
echo 'PASS: release workflow checksum contract'
