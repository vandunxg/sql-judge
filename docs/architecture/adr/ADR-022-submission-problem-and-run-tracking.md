# ADR-022: Track Problem and Sample Runs in Submission History

- Status: Accepted
- Date: 2026-10-14
- Requirements: REQ-013, REQ-020
- Acceptance: AC-013, AC-020
- Security: SEC-004
- Amends: ADR-008 (history schema), ADR-016 (backup contract version)

## Context

The practice workspace needs per-problem status, a history of sample runs and
per-fixture results. `submissions` stored only pack identity and an aggregate
verdict, and only Submit wrote history. Existing user databases were created
with `CREATE TABLE IF NOT EXISTS`, and `schema_version` is pinned by
`CHECK (version = 1)`, so new DDL alone would never reach them.

## Decision

- Add `problem_id TEXT NULL` and
  `mode TEXT NOT NULL DEFAULT 'submit' CHECK (mode IN ('run', 'submit'))` to
  `submissions`.
- Migrate by checking columns (`pragma_table_info`) and running
  `ALTER TABLE ... ADD COLUMN` only for missing columns, inside the same
  `BEGIN IMMEDIATE` transaction that creates the base tables. Fresh and
  upgraded databases follow the same path, so their schemas match. The
  migration is idempotent and atomic, and the write lock stops two openers
  from adding the same column twice. It reuses the column-check approach
  already used for the environment ledger.
  `schema_version` is not bumped: changing its CHECK would mean rebuilding the
  table and would add nothing beyond column checks.
- Existing rows read as `problem_id = NULL`, `mode = 'submit'`.
- Rust checks `mode` against `run | submit` and checks that `problem_id`, when
  present, is a bounded ASCII identifier. SQLite enforces `mode` again with a
  CHECK constraint.
- Managed Run now saves a history record with `mode = 'run'`. Run and Submit
  both save the digest-verified problem ID. Like submissions, runs store only
  IDs, verdicts and timings: never SQL text, result rows, fixture data or
  credentials.
- Backup contract version 3 adds `problemId` (nullable) and `mode` to each
  submission. Restore still accepts version 2. A version 2 entry must not
  carry `problemId` or `mode` and restores as an untracked submission. A
  version 3 entry must carry a valid `mode`. Size limits, strict unknown-field
  rejection, unstable-state rejection and single-transaction replacement are
  unchanged. Restore runs the history migration before it writes.

## Consequences

- The UI can derive per-problem status and show runs separately from
  submissions.
- History grows on every sample run. No retention limit is added yet.
- Backups from this version cannot be restored by older builds, which accept
  only version 2.
- `TestOutcome` in `judge-postgres` exposes no per-fixture timing. Every
  test row therefore stores the whole judge call's duration. Summing
  `elapsedMs` across tests overstates time; the largest value equals the run's
  wall time. Accurate per-fixture timing needs the orchestrator to time each
  case, verified against a real PostgreSQL instance.
