# AGENTS.md — Mandatory working contract for SQL Judge

These rules apply to ALL AI agents, including subagents, code reviewers, PR agents, and pack generators. Read before editing. Task-specific instructions may add constraints but cannot weaken safety, testing, or acceptance requirements.

## Project facts (not guesses)
- Project name: `sql-judge`; user already ran `create-tauri-ui` with Vite, preset b0, Starter dashboard, Size optimization, Invoke example, Release workflow; NSIS/Portable, both macOS DMGs, AppImage/DEB selected.
- Preserve existing frontend and native structure from the **actual checkout**. Do not assume previous illustrative `apps/desktop` tree is present. Do not change package manager (Bun) or replace shadcn/ui with another kit.
- SQL Judge executes database input. Treat editor SQL, external DB connections, downloaded packs, generated SQL, and any metadata supplied by AI as untrusted.
- Current document contract version: v1.0. Stable spec IDs: `REQ-*`, `AC-*`, `SEC-*`.

## Read order / source of truth
1. `docs/requirements/PRD.md` and `docs/requirements/ACCEPTANCE.md`.
2. `docs/architecture/ARCHITECTURE.md`; for changes crossing boundaries, read ADRs.
3. Feature contract under `docs/contracts/` and `docs/security/THREAT-MODEL.md`.
4. `docs/testing/TDD-RULES.md` and current task in `tasks/todo.md`.
5. For frontend changes, `docs/design/SQL Judge — UI redesign.html` plus the matching screen export under `docs/design/` (for example `docs/design/Home/Main.dc.html`).
6. Exact code and dependency versions in current checkout. Consult current upstream official docs for framework APIs.

If docs conflict: explicit current user instruction > accepted PRD/AC > contracts > ADRs > tasks > examples. Identify conflict and update the appropriate contract **before** changing implementation.

## Default engineering workflow
1. Inspect `git status`, tree, manifests (`package.json`, `bun.lock`, `src-tauri/Cargo.toml`, `tauri.conf.json`), existing tests, CI workflow and current task. Do not destroy uncommitted changes.
2. Select ONE small task (ideally <=5 touched files); state requirements/acceptance IDs and the negative or boundary case it must cover.
3. RED: author a meaningful failing test and run it to show expected failure; record command/evidence.
4. GREEN: smallest secure change to make it pass; keep UI/app behavior outside scope unchanged.
5. REFACTOR with tests green. Run focused and affected suites, fmt/lint, typecheck/build when appropriate.
6. Review authorization, credentials, query isolation, rollback/cleanup, and regression risk for every DB-facing change.
7. Update docs/tasks, report changed files, tests/results and any omissions. Do not mark a task complete with tests skipped or unverified.

## Architecture boundaries
- `judge-core`: pure types, policies, comparator, verdict aggregation; **no Tauri, OS, SQL driver or storage dependency**.
- PostgreSQL and SQL Server runners/adapters implement capability-oriented interfaces; do not assume dialect equivalence.
- Tauri command handler is thin orchestration with typed args/results, auth/policy enforcement and error mapping.
- SQL fixtures and admin/setup queries run only in trusted, ephemeral environments; user SQL runs only under constrained runner identities.
- SQLite metadata/submission history is NEVER used as an SQL practice backend.
- External arbitrary user-supplied servers default to **Explorer / read-only**; never implicitly grant reset/drop/judge-admin permissions.

## Hard safety invariants
- NEVER issue `DROP DATABASE`, `DROP SCHEMA ... CASCADE`, `TRUNCATE`, or `DELETE` against unowned external database/schema as part of a reset or a fallback path.
- NEVER dynamically execute user text with admin/provisioner credentials. Parser/regex/`READ ONLY` are defense-in-depth, not a sandbox.
- Require managed ownership marker + scoped resource ID + displayed impact summary + explicit confirmation for destructive lifecycle actions; verify ownership again in Rust after the UI click. Fail closed.
- No default connection to production or real business data, no implicit external schema creation, no automatic probe that writes or alters data.
- Linux installer downloads only from the official release, verifies the published SHA-256 manifest before mutation, supports x86_64 only until other artifacts are verified, and must not run the entire installer as root.
- No plaintext secrets in SQLite, JSON, frontend storage, logs, crash reports or commit. Use OS credential vault or prompt per session; TLS verification on external connections.
- Limit CPU/time/memory/rows/result bytes; cancel and clean up after interruptions; do not silently turn infrastructure failure into Wrong Answer.
- Untrusted pack imports must reject path traversal, symlinks, zip bombs, invalid manifests, unexpected paths, excessive SQL and unsafe setup. Imports cannot modify user external DBs.
- Hidden tests on an offline client are not secret; do not present them as tamper-proof competitive exam grading.

## Build, quality and security rules
- Preserve generated tauri-ui dashboard + native defaults, debug panel, external-link guard, release workflow; adapt by incremental composition.
- Implement accessibility: keyboard navigation, labels, focus, contrast, resize, no color-only verdicts; responsive desktop layouts and empty/error/loading states.
- Structured error codes, actionable messages, no leaking connection strings/passwords. Log operations with IDs/timings, never raw credentials.
- Must have unit + integration + negative/security tests for DB operations; minimize mocking of semantics-sensitive behavior.
- No `#[allow(...)]`, `@ts-ignore`, eslint-disable, skipped tests, coverage reductions or fake verification without explicit justified review.
- New dependency / workflow / migration / capability changes require a short decision record and review; do not pin arbitrary versions from memory.
- No rewrite of generated `release.yml` or claimed successful six-platform packaging until actual matrix builds were run with PostgreSQL runtime included or explicitly documented as external prerequisite.
- CI runs only on pushes to `main` and `release/**` (ADR-029). Feature branches and pull requests get no CI, so run the affected local suites before merging, and keep `bash scripts/test_ci_workflow.sh` passing when editing triggers.
- A release bumps the app version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` together, adds a dated `CHANGELOG.md` section on top, passes `bash scripts/test_release_version.sh` and is tagged `vX.Y.Z` on `main` or `release/**`. Before creating or moving a release tag, review and update every affected requirement, acceptance, architecture/ADR, security, operator instruction and release document under `docs/`; record deliberate no-change decisions in the release documentation checklist. Pushing the tag publishes the release; do not push it without the user's approval.
- Release documentation must distinguish Tauri artifact packaging from PostgreSQL runtime installation; a checksum-verified installer is not evidence that the database runtime is bundled.
- Never quietly lower thresholds to get green. Never claim test/build success without command output.

## UI design fidelity
- Design exports in `docs/design/` are mandatory visual contracts for frontend work. Inspect the application-wide export and the target screen artboard before editing UI.
- Treat exact exported colors, dimensions, spacing, typography, radii, borders, shadows, interaction states and responsive rules as requirements. Match them through the existing component system; do not use defaults or improvised values when the export specifies a value.
- Exported HTML and vendor scripts are reference material only. Reimplement with repository React components and styles; never ship copied design-export runtime markup or scripts.
- When a shared primitive needs a different design value, inspect all callers and update it deliberately. Otherwise scope the override to the target screen.
- Validate the rendered screen against the reference before claiming visual fidelity. If a reference is incomplete or conflicts with accessibility/security requirements, stop and ask for direction.

## Agent handoff format
`Task ID | Files changed | Requirements | Tests (command, result) | Security assumptions | Known limitations | Next safe task`.

## Decision triggers
- Ambiguity that changes data safety or irreversible effects => stop and request approval.
- Purely reversible product details => choose simplest consistent option, document assumption, implement against tests.
- Architecture modifications => update ADR and mapping before changing code.
