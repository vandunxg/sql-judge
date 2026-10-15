# Acceptance criteria, traceability and Definition of Done

No capability is complete until its AC passes on the implemented target; documentation-only/spec validator checks do not prove database or application behavior.

| AC ID | Requirements | Scenario / observable pass condition |
|---|---|---|
| AC-001 | REQ-001 | Existing tauri-ui dashboard/theme/invoke/debug/release config unaffected; `git diff` shows additive edits. |
| AC-002 | REQ-002,003 | Search/filter/select problem; syntax editor draft retained when navigating away and back. |
| AC-003 | REQ-004 | Public sample Run returns typed columns/rows within caps; malformed SQL surfaces SyntaxError. |
| AC-004 | REQ-005,008 | Correlated subquery and window solution both Accepted for equivalent semantics across fixtures. |
| AC-005 | REQ-006 | Bag comparator rejects `[(1),(1),(2)]` vs `[(1),(2),(2)]`; NULL distinct from empty. |
| AC-006 | REQ-006 | Ordered mode rejects reordered rows; unordered mode ignores row order only, not row multiplicity. |
| AC-007 | REQ-007,017 | Syntax/timeout/resource/infra errors get distinct verdicts; reference crash is JudgeError. |
| AC-008 | REQ-009,010 | Example pack satisfies manifest and structural validation; invalid traversal/symlink/duplicate ID rejected. |
| AC-009 | REQ-009 | Atomic package import: failed schema/test validation leaves no registered pack nor mutable residue. |
| AC-010 | REQ-011,012 | Two packs with identically named tables do not interfere; reset one preserves the other. |
| AC-011 | REQ-011 | Reset/delete of unmanaged DB is refused even if caller forges frontend request/ownership ID. |
| AC-012 | REQ-011 | Managed rebuild fails halfway and old workspace remains usable; stale temp env cleaned. |
| AC-013 | REQ-013 | SQLite history persists after restart; same submission stores pack hash + engine version. |
| AC-014 | REQ-014,015 | PG and MSSQL explorer connect to test DB using low-privilege accounts, inspect schema, SELECT only; deny INSERT/DROP. |
| AC-015 | REQ-015 | Wrong creds/TLS failure/offline server yield actionable code with no secret leak; no destructive probe. |
| AC-016 | REQ-016,023 | Airplane-mode launch works with installed runtime; when missing, clear guided error rather than claim full offline runtime. |
| AC-017 | REQ-017 | Cancel/kill judge halfway does not persist a false Accepted and leaves no active run or leaked temp schema. |
| AC-018 | REQ-018 | Logs include IDs/duration/operation/status but never password/connection string/raw SQL unless user opted in. |
| AC-019 | REQ-019 | Keyboard-only path covers Home -> Practice -> editor -> Run/Submit -> results and explorer flows; meaningful focus and screen reader labels; Home is the default application route. |
| AC-020 | REQ-020 | Backup import round-trip preserves pack metadata/history but omits secrets and vault keys. |
| AC-021 | REQ-022 | An explicitly confirmed PostgreSQL sandbox is created only in a generated, marker-owned schema on the selected verified profile; forged IDs, changed targets, unmarked schemas, and profile deletion while referenced are denied without modifying the external database. |
| AC-022 | REQ-001,023 | Selected OS artifacts produced by CI and tested with actual runtime packaging; otherwise documented as *not yet verified*. |
| AC-023 | REQ-019,025; NFR-011 | At the artboard viewport and supported responsive widths, each implemented screen matches its corresponding design export for shell geometry, tokens, typography, spacing, radii, borders, shadows, icons, interaction states and loading/empty/error states. Evidence is a rendered comparison; exported HTML is not shipped as production UI. |
| AC-024 | REQ-026, NFR-010 | On Linux x86_64, the installer selects the verified DEB on Debian-family systems and verified AppImage otherwise, rejects missing/mismatched checksums and unsupported architectures before system mutation, supports `--user` without sudo and `--system` with sudo, and documents the separate PostgreSQL runtime prerequisite. |
| AC-025 | NFR-012 | A push to a feature branch or a pull request starts no CI run, while a push to `main` or `release/**` that touches the runtime paths starts `managed-runtime.yml`; a release tag `vX.Y.Z` matches the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and the newest `CHANGELOG.md` section, and follows the documented release review for affected requirements, acceptance, architecture, security and operator instructions. |

## Required negative/security cases
- User SQL attempts: two statements; DDL via CTE/function; `pg_sleep`; extension/privileged function; SQL Server `EXEC`, `xp_cmdshell`, cross-DB access; unauthorized metadata access; recursive explosion; oversized rows; invalid encodings; cancellation; connection lost.
- Import attempts: `../evil.sql`, absolute paths, symlink, duplicate ZIP entries, ZIP bomb, manifest unknown properties, path case collisions, SQL setup exception, mismatched dialect/engine version, broken reference query, wrong expected typed columns.
- Environment attempts: guessed ID, copied ownership marker, stale marker, cross-pack resource, reset while judging, mixed schemas, orphaned workspace after app crash.

## Definition of Done (all tasks)
- [ ] Linked to a requirement and acceptance ID, with tests covering a negative or edge case.
- [ ] RED observed (test fails for expected behavior), GREEN observed, regression run after refactor.
- [ ] Rust fmt/clippy/tests and TS lint/typecheck/build relevant to changes pass with fresh output.
- [ ] No secret leakage or unmanaged destructive DB operation; security review for SQL/connection changes.
- [ ] Error/loading/empty state and keyboard use considered for UI changes.
- [ ] Matching artboard inspected before frontend changes; rendered comparison evidence confirms visual fidelity at target and responsive widths.
- [ ] Documentation/ADR changed if contract changed; no disabled tests or lowered gates.
- [ ] Handoff includes commands, actual output summary and residual risks.

## Exit milestones
- **P0:** AC-001 (preservation plan), AC-008 structural portion plus validator unit tests.
- **P1:** AC-003–007 and AC-010 for managed test runner, single end-to-end desktop slice.
- **P2:** AC-009–013, AC-017–020.
- **P3:** AC-014/015 with PG and MSSQL external integration environments.
- **P4:** AC-016/022 verified per-platform and AC-021 only after extra scoped design.
