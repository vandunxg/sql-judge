# ADR-028: Published release asset uploads

Status: Accepted

## Decision

The release workflow creates or locates one release before the build matrix,
then passes its immutable release ID to `tauri-apps/tauri-action@action-v1.0.0`.
The matrix uploads assets only; it cannot update release notes. A final job
publishes a draft only after every platform upload succeeds. It uses v1 input
names `releaseAssetNamePattern` and `uploadUpdaterJson`.

The prior v0.6 action hid the failed release-asset upload behind retry output.
This design aligns with v1's release-ID upload path and keeps an existing
published release's title/body untouched. A repository `GITHUB_TOKEN` with
`contents: write` remains the credential; no PAT is added without an explicit
authorization failure.

## Consequences

- A workflow rerun can replace same-named assets on an already published release
  without replacing its release notes.
- New releases remain drafts if any matrix job fails; successful runs publish
  only after all assets and the Linux checksum manifest upload.
- Linux uploaded filenames and manifest entries use `x86_64`, matching the
  verified installer lookup.
- Any release workflow change must use the current action input names and pass
  `scripts/test_release_workflow.sh`.
