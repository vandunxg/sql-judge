# ADR-023: Problem Identity on Submissions and Per-Fixture Judge Results

Status: Accepted (amended by ADR-024: sample runs are recorded with `mode`)

## Context

Submission history recorded the exact pack identity (ID, version, digest) but
not which problem inside the pack was submitted, so the History and Problems
screens could not show per-problem status. The judge IPC response carried only
the aggregate verdict, so the Solve screen could not show which fixtures
passed ("3 of 4 fixtures passed", "Fixture 2 · salary ties").

## Decision

### Storage migration

- `submissions` gains a nullable `problem_id TEXT` column. `HistoryStore::open`
  adds it idempotently with `PRAGMA table_info` + `ALTER TABLE ... ADD COLUMN`,
  the same additive pattern ADR-020 used for `environment_ledger`. The
  `schema_version` row stays at `1`: its `CHECK (version = 1)` constraint
  predates column-level migrations, and every migration so far is a
  presence-checked, additive, nullable/defaulted column that is safe to re-run.
- Rows written before this change keep `NULL`; nothing is backfilled or guessed.
- `SubmissionRecord` and `SubmissionSummary` carry `problem_id: Option<String>`.
  When present it must match the problem ID pattern from
  `problem.schema.json` (`^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`, max 80 bytes);
  storage and backup restore reject anything else.
- `submit_solution` records the ID of the problem that Rust loaded from the
  exact digest-verified pack. A problem ID that is not in that pack is rejected
  as `PolicyViolation` before judging, and nothing is written to history.
- `list_submission_history` returns `problemId: string | null`.

### Backup format

`BackupSubmission` gains `problemId`, with
`#[serde(default, skip_serializing_if = "Option::is_none")]`. The backup version
stays **2**:

- Existing v2 backups (no `problemId`) still parse and restore, with `NULL`
  problem IDs.
- Snapshots whose submissions all lack a problem ID serialize exactly as
  before.
- A newer backup that contains `problemId` is rejected by older builds because
  `deny_unknown_fields` fails closed, which is the same outcome a version bump
  would give. A bump would also have made every new backup unreadable by older
  builds, even ones without problem IDs, and would have forced a v2→v3
  migration path for no safety gain.

### Per-fixture judge results

`JudgeRunResult` gains `tests: { id, verdict, elapsedMs, hidden }[]` in
manifest order. `run_sample` lists the visible tests that ran and
`submit_solution` lists every test that ran. The list is empty on error,
policy, or unavailable paths, and is truncated if the judge is cancelled.
`elapsedMs` is the wall-clock time of the user's query for that fixture: 0 when
the user query never ran, for example when the reference failed. The
orchestrator's `TestOutcome` now carries `elapsed_ms`, and stored
`submission_tests.elapsed_ms` uses the per-test value instead of the whole
request duration.

No expected rows, fixture SQL, reference SQL, or comparison-mismatch details
are added to the IPC response. The `hidden` flag and test IDs come from the
manifest, which is already public on an offline client (ADR-022, `AGENTS.md`).

## Consequences

- History can be grouped per problem for new submissions; legacy rows show no
  problem.
- The Solve screen can render per-fixture pass/fail chips without learning
  hidden fixture contents.
- Downgrading after submitting with a newer build makes the older build reject
  the newer backups, by design.
