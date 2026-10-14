# ADR-028: Published release asset uploads

Status: Accepted

## Decision

The release matrix uses `tauri-apps/tauri-action@action-v1.0.0` and uploads to
the published release for its version (`releaseDraft: false`). It uses v1 input
names `releaseAssetNamePattern` and `uploadUpdaterJson`.

The existing `v0.1.0` release is published. The prior v0.6 configuration asked
the action to find a draft release, so reruns could build artifacts then fail
during release-asset upload without a useful API error. A repository
`GITHUB_TOKEN` with `contents: write` remains the credential; no PAT is added.

## Consequences

- A workflow rerun can replace same-named assets on an already published release.
- Tag-triggered releases publish automatically rather than waiting for a manual
  draft publication step.
- Any release workflow change must use the current action input names and pass
  `scripts/test_release_workflow.sh`.
