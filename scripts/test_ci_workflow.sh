#!/usr/bin/env bash
set -euo pipefail

workflow=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/.github/workflows/managed-runtime.yml
fail() { echo "FAIL: $*" >&2; exit 1; }

# The `on:` block, up to the next top-level key.
triggers=$(awk '/^on:/ { inside = 1; next } inside && /^[^[:space:]#]/ { exit } inside' "$workflow")
events=$(grep -E '^  [a-z_]+:' <<<"$triggers" | sed -E 's/^  ([a-z_]+):.*/\1/' | sort | tr '\n' ' ')
branches=$(awk '/^    branches:/ { inside = 1; next } inside && /^    [a-z_-]+:/ { exit } inside' <<<"$triggers" |
  sed -E 's/^ *- *"?([^"]*)"? *$/\1/' | sort | tr '\n' ' ')

[ "$events" = "push workflow_dispatch " ] || fail "CI must trigger only on push and workflow_dispatch, got: $events"
[ "$branches" = "main release/** " ] || fail "CI push must be limited to main and release/**, got: $branches"
! grep -Eq '^    (branches-ignore|tags):' <<<"$triggers" || fail "CI push must not add other ref filters"
! grep -Fq "github.event_name != 'pull_request'" "$workflow" || fail "pull_request guards are dead without a pull_request trigger"
echo 'PASS: CI runs only on main and release/** branches'
