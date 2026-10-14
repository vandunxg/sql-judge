# Pack Catalog in SQLite, Pack Deletion and Pack Sandbox Design

## Goal

Make installed packs fast to browse and judge, let the user remove a pack, and
give every pack a ready practice environment that Run uses automatically.

Requested by the user on 2026-10-14:

1. Import loads problems into the local SQLite database, so the catalog is never
   re-read from pack files or JSON on each use. User data (submission history,
   connection profiles, environments) already lives in that database.
2. Deleting a pack actually deletes it.
3. Importing a pack automatically creates a dedicated environment for it and
   runs the pack's schema there.
4. Opening any problem automatically uses that pack's environment.

## Decisions agreed with the user

- **Storage:** SQLite becomes the only runtime source of pack content
  (approach A). Approaches rejected: a SQLite display index while judging
  still reads files (B), and an in-memory cache loaded from files (C).
- **Environment purpose:** a sample-data sandbox. Run executes in the pack's
  environment with the problem's public sample data. Submit keeps judging in
  isolated fixtures, including hidden tests.
- **Deleting a pack** removes the pack's environments and catalog rows, and
  keeps submission history.
- **Order:** catalog first, then deletion, then sandbox.

## Current state (2026-10-14)

- Packs are published as content-addressed directories under
  `<app data>/packs/sha256/<digest>`. `list_installed_packs` reads and re-hashes
  every directory on every call. Run, Submit, environment provisioning,
  `managed_pack_identity`, `inspect_managed_pack` and backup validation each
  reload and re-validate the pack from disk (`PackImporter`).
- The pack `schema` holds DDL only. Each problem's `schema_sql` is the pack's
  schema. Data lives in each test's fixture SQL.
- Judging never uses managed environments. Each case builds a temporary
  `sj_fixture_*` schema inside a transaction and rolls it back. User SQL must
  pass `validate_read_only_sql`, and the runner role has SELECT only.
- No pack deletion exists in the backend, the UI or git history.

## Part 1: Pack catalog in SQLite (ADR-025)

### Data model

These tables live in the existing `sql-judge.sqlite` file. `app-storage` owns
them and adds them idempotently (`CREATE TABLE IF NOT EXISTS`) inside the same
`BEGIN IMMEDIATE` opener pattern as ADR-024.

- `packs`: `digest TEXT PRIMARY KEY`, `pack_id`, `version`, `title`, `dialect`,
  `schema_sql`, `installed_at`, and `UNIQUE (pack_id, version)`.
- `pack_problems`: `digest` (FK → `packs` ON DELETE CASCADE), `problem_id`,
  `position`, `catalog_json`, `dialect`, `reference_sql`, `output_json`,
  `statement_timeout_ms`, `max_rows`, `max_bytes`, and
  `PRIMARY KEY (digest, problem_id)`.
  - `catalog_json` holds the display entry the UI already receives:
    title, difficulty, topics, statement, schema, sample rows, starter SQL,
    output columns, row order, time limit, fixture count and sample tables.
- `pack_problem_tests`: `digest`, `problem_id` (FK → `pack_problems`
  ON DELETE CASCADE), `test_id`, `position`, `hidden`, `fixture_sql`, and
  `PRIMARY KEY (digest, problem_id, test_id)`.

`app-storage` exposes plain record types (strings, integers, booleans) with no
dependency on `problem-pack` or `judge-core`. `problem-pack` converts a
`ValidatedPack` to those records, and the records back to `ValidatedProblem` and
`PackCatalogEntry`. `src-tauri` orchestrates.

### Import

1. Parse and validate the pack structurally (unchanged).
2. Run the runtime validation in throwaway fixtures (unchanged).
3. Write every row in one SQLite transaction. No pack files are written.
4. Import is idempotent by digest:
   - The same digest returns `already_installed`.
   - The same `(pack_id, version)` with a different digest fails with the
     existing immutable-version error.

### Reads

These read SQLite instead of the file registry:

- `list_installed_packs`
- Run and Submit (`judge_request`)
- environment provisioning (`managed_pack_schema`, `create_managed_environment`)
- `managed_pack_identity` and `inspect_managed_pack`
- backup validation (`validate_backup_pack_registry`)

Rows are written only after full validation. From then on they are trusted
local metadata, like submission history, and are not re-hashed on read. The
digest stays the pack identity.

### Migrating existing installs

When the app opens its state:

1. If `<app data>/packs/sha256/` exists, load and structurally validate each
   directory with the existing loader, check that its digest matches its folder
   name, and insert it if missing. Runtime validation already passed on the
   original import and is not repeated.
2. Remove the old `packs/` directory only after every pack has been inserted.
3. If any pack fails, keep the directory, log the pack digest, and retry on
   the next start. Packs that did migrate stay available.

## Part 2: Deleting a pack

### UI

- Every pack card gets a **Delete** action. It opens a confirmation dialog that
  shows:
  - the pack title, version and digest
  - the number of problems
  - every managed environment of that exact pack that will be removed
  - a note that submission history is kept
- The confirm button stays disabled while the deletion runs. Errors are shown
  in the dialog.
- After deletion, the catalog and environment list refresh.

### Backend: `delete_pack({ packId, version, digest, confirmed })`

1. Reject the request unless `confirmed` is true and the exact identity exists
   in SQLite.
2. Find every managed environment pinned to that exact identity.
3. If any exist, start the runtime. Delete each environment through the
   existing ownership-verified delete path, which writes a recovery intent,
   drops the owned namespace and removes the ledger row. The existing logic
   is refactored into a helper that takes the locked state.
4. If any environment fails to delete, stop and return an error. The pack and
   the environments not yet deleted are kept, so the user can retry.
5. Delete the `packs` row. Problems and tests cascade in the same transaction.

Submission history is not touched. History and Progress already label
problems from uninstalled packs without a link. Re-importing the same digest
links them again.

## Part 3: Pack sandbox environment (ADR-026)

### Identity

There is one sandbox per installed pack version. Both names are derived from
the digest, so they are stable and recognizable:

- `env_id`: `pack-<first 16 hex of digest>`
- namespace: `sj_pack_<first 16 hex of digest>`

The namespace satisfies the managed namespace policy (`sj_` prefix, lowercase,
digits and `_`, at most 63 bytes).

### Creation

- After the import transaction commits, the app tries to create the sandbox
  through the existing provisioning path. That path reserves the ledger row,
  saves a recovery intent, provisions the owned namespace with the pack schema,
  marks it ready and clears the intent.
- A provisioning failure never fails the import. The import result reports the
  sandbox state (`ready` or `pending`).
- `ensure_pack_sandbox` creates a missing sandbox on demand. It runs on the
  first Run, or when an earlier attempt failed or the user deleted the
  sandbox. If the sandbox exists but is not `ready`, Run returns `unavailable`
  with a message that points to the Environments screen. It never
  auto-recovers or resets.

### Run

For each visible test, one transaction:

1. `SET LOCAL search_path` to the sandbox namespace.
2. Load the test's fixture SQL into the sandbox tables, as the namespace owner.
3. Run the reference SQL and the user SQL as the constrained runner, which
   has SELECT only (`validate_read_only_sql` still applies).
4. Compare the results with the existing comparator.
5. `ROLLBACK`.

The committed sandbox state is always the pristine pack schema with empty
tables. Problems never see each other's data, and Run never needs a reset or
another destructive step. The response shape is unchanged: verdict, preview
rows and per-fixture results.

**Submit** is unchanged. It keeps the isolated `sj_fixture_*` schemas,
including hidden tests.

### Solve screen

- `environmentForProblem` prefers the pack's sandbox `env_id`, then falls back
  to any ready environment of that exact identity.
- The chip shows the sandbox state. No manual selection is needed.

The sandbox is an ordinary managed environment. It appears on the Environments
screen, where the user can reset or delete it with confirmation. Deleting it
is safe because the next Run recreates it.

## Security and safety

- Pack content still enters only through full structural and runtime
  validation. Migration re-runs the structural validation and the digest check.
- Sandbox creation and Run touch only the app-owned namespace. Ownership is
  verified by the existing provisioning and session code.
- Run loads fixture data only inside a transaction that is always rolled back.
  The runner keeps SELECT-only grants, and user SQL keeps the read-only policy.
- Deleting a pack requires explicit confirmation in the UI and
  `confirmed: true` in Rust. It re-verifies the exact identity. Each
  environment goes through the existing ownership-verified delete path with a
  recovery intent. Deletion stops on the first error.
- SQLite never becomes a SQL practice backend: it stores pack text, and SQL
  runs only in PostgreSQL.

## Testing

### Part 1

- `app-storage`:
  - migration on a fresh database and on an older database
  - insert and read round trip
  - cascade delete
  - immutable `(pack_id, version)` conflict
  - an invalid record is rejected
- `problem-pack`: a `ValidatedPack` converts to records and back to the same
  `ValidatedProblem` and `PackCatalogEntry`.
- `src-tauri`:
  - import writes SQLite and creates no `packs/` directory
  - `list_installed_packs` and Run/Submit work with no registry on disk
  - a legacy registry migrates, and its directory is removed only on full
    success
  - a corrupt legacy pack keeps the directory

### Part 2

- `src-tauri`:
  - deleting without confirmation fails, and so does deleting an unknown or
    mismatched identity
  - deleting a pack drops its environments' namespaces and removes the
    catalog rows
  - history survives deletion
  - an environment delete failure keeps the pack
- Frontend: dialog state (impact text, pending, error) and the refresh after
  deletion.

### Part 3

- `src-tauri`:
  - import creates a ready sandbox
  - import still succeeds when provisioning fails
  - Run recreates a missing sandbox
  - after Run, the sandbox tables hold 0 rows
  - Run on problem A then problem B gives correct, independent verdicts
  - Submit is unchanged
  - a sandbox that is not ready makes Run return `unavailable`
- Frontend: `environmentForProblem` prefers the sandbox.

## Documentation

- ADR-025 records the pack catalog in SQLite and supersedes file-registry
  reads.
- ADR-026 records the pack sandbox and Run in the sandbox.
- Update `docs/contracts/PROBLEM-PACK.md` (storage) and `tasks/todo.md`.

## Out of scope

- Free-form browsing or querying of a sandbox (an internal Explorer).
- Retention limits for history.
- SQL Server packs. The managed judge is PostgreSQL only.
