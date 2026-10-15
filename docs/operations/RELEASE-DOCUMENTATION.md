# Release Documentation Contract

Complete this checklist before creating, updating or pushing a release tag.
The versioned commit must contain the documentation changes required by the
release; a tag never substitutes for that review.

## Required Release Records

1. Set one version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   the `tauri-native` entry in `src-tauri/Cargo.lock`.
2. Add the newest dated `CHANGELOG.md` section with fixed, changed and known
   limitation notes.
3. Review `docs/requirements/PRD.md` and
   `docs/requirements/ACCEPTANCE.md`; update them when product behavior,
   requirements or observable acceptance changes.
4. Review affected `docs/architecture/adr/`, contracts and
   `docs/security/THREAT-MODEL.md`; add or amend an ADR before changing an
   architecture boundary, and record security-impacting decisions.
5. Review `docs/operations/` and project instructions (`AGENTS.md`) for
   changed build, runtime, installation, migration, safety or support steps.
6. Record no-change decisions in the release pull request or release commit
   message when a reviewed document is not affected.

## Tag Gate

Run `bash scripts/test_release_version.sh`,
`bash scripts/test_ci_workflow.sh` and
`bash scripts/test_release_workflow.sh` before creating `vX.Y.Z`. Do not push
the tag without explicit approval. A release tag must point to the commit that
contains the matching version, changelog and documentation review.
