# ADR-024: Track Sample Runs in Submission History

- Status: Accepted
- Date: 2026-10-14
- Requirements: REQ-013, REQ-020
- Acceptance: AC-013, AC-020
- Security: SEC-004
- Amends: ADR-008 (history schema), ADR-016 (backup contract), ADR-023 (problem identity)

## Context

The History and Progress screens need to show sample runs separately from
submissions, and History needs the per-fixture results of every record.
ADR-023 already added a nullable, digest-verified `problem_id` and per-fixture
timing, but only Submit wrote history and `list_submission_history` returned
no test rows. Existing user databases were created with
`CREATE TABLE IF NOT EXISTS`, and `schema_version` is pinned by
`CHECK (version = 1)`, so new DDL alone would never reach them.

This ADR was first written as ADR-022 on the `feature/refactor-design`
branch, in parallel with ADR-023 on `main`. On merge, ADR-023 took precedence
on every point the two ADRs both covered: the problem ID pattern, backup
version 2, and per-fixture timing.

## Decision

- Add `mode TEXT NOT NULL DEFAULT 'submit' CHECK (mode IN ('run', 'submit'))`
  to `submissions`, alongside ADR-023's `problem_id`.
- Migrate both columns by checking `pragma_table_info` and running
  `ALTER TABLE ... ADD COLUMN` only for missing columns, inside the same
  `BEGIN IMMEDIATE` transaction that creates the base tables. Fresh and
  upgraded databases follow the same path. The migration is idempotent and
  atomic, and the write lock stops two openers from adding the same column
  twice. `schema_version` is not bumped (see ADR-023).
- Existing rows read as `mode = 'submit'`.
- Rust checks `mode` against `run | submit`; SQLite enforces it again with a
  CHECK constraint. `problem_id` keeps ADR-023's `problem.schema.json` pattern.
- Managed Run now saves a history record with `mode = 'run'`. Run and Submit
  both save the digest-verified problem ID and per-fixture timings. Runs, like
  submissions, store only IDs, verdicts and timings, never SQL text, result
  rows, fixture data or credentials.
- `list_submission_history` returns `mode` and each record's
  `tests: { testId, verdict, elapsedMs }[]`.
- Backup stays at **version 2**, as ADR-023 decided. `BackupSubmission` gains
  an optional `mode` with `#[serde(default, skip_serializing_if = ...)]`. Export
  writes it only for runs, and restore reads a missing `mode` as `submit`. This
  means:
  - A snapshot with no runs serializes exactly as it did under ADR-023.
  - Every existing v2 backup restores, with or without `problemId`.
  - A backup that contains runs is rejected by older builds through
    `deny_unknown_fields`. This fails closed.
  - Restore runs the history migration before it writes. Invalid `mode` or
    `problemId` values reject the whole restore, and nothing is replaced.
- Screens that count submissions must ignore `mode = 'run'` records. The
  Problems and Solve screens filter them out with `submissionsOnly`.

## Consequences

- The UI can show runs separately from submissions, with per-fixture results.
- History grows on every sample run. No retention limit is added yet.
- Older builds cannot restore backups that contain runs.
