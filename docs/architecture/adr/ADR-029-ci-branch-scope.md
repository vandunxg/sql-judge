# ADR-029: CI runs only on main and release branches

Status: Accepted

## Decision

`.github/workflows/managed-runtime.yml` triggers on `push` to `main` and
`release/**` (still narrowed by its path filter) and on `workflow_dispatch`.
It has no `pull_request` trigger, so pushes to other branches and pull requests
run no CI. The `pull_request` guards on the `bundle` and `e2e-linux` jobs were
removed because that event can no longer reach them.

The workflow runs on macOS, Windows and Linux runners and builds three
installers; running it for every feature-branch push and pull request spent
that cost on work that is verified again once it reaches `main` or a release
branch.

## Consequences

- Feature branches and pull requests get no CI status check; a change is first
  verified after it is pushed to `main` or `release/**`.
- A run on any other branch must be started manually with `workflow_dispatch`.
- `release.yml` is unchanged: it publishes on `v*` tags and manual dispatch.
- Any change to the CI triggers must pass `scripts/test_ci_workflow.sh`.
