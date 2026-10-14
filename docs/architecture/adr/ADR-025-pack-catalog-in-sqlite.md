# ADR-025: Installed Pack Catalog in SQLite

- Status: Accepted
- Date: 2026-10-14
- Amends: ADR-015 (pack loader publication), ADR-020 (pinned managed pack identity)
- Design: `docs/superpowers/specs/2026-10-14-pack-catalog-and-sandbox-design.md`

## Context

Imported packs were published as content-addressed directories under
`<app data>/packs/sha256/<digest>`. Each of these re-read every file, re-parsed
its JSON and re-hashed the whole pack:

- listing installed packs
- Run and Submit
- environment provisioning
- pack identity lookups
- backup validation

Each step slowed down as more packs were installed. User data (history,
connection profiles, environments) already lived in `sql-judge.sqlite`.

## Decision

- `app-storage` owns three tables in `sql-judge.sqlite`:
  - `packs`: one row per digest, with `UNIQUE (pack_id, version)`
  - `pack_problems`: display catalog JSON, reference SQL, output policy JSON
    and limits
  - `pack_problem_tests`: test ID, hidden flag and fixture SQL

  The tables are created idempotently in an `IMMEDIATE` transaction. Problems
  and tests cascade when their pack is deleted. `app-storage` stores plain
  records with no dependency on `problem-pack` or `judge-core`.
- `problem-pack` converts a `ValidatedPack` into those records, and records
  back into `ValidatedProblem` and `PackCatalogEntry`. Every problem uses the
  pack schema, as before.
- Import still runs full structural validation and runtime validation in
  disposable fixtures. All rows are then written in one transaction, and no
  pack files are written.
  - A pack whose digest is already stored returns `already_installed` and
    skips runtime validation.
  - The same `(pack_id, version)` with a different digest fails with "Another
    pack is already installed with this ID and version."
- These now read SQLite:
  - listing installed packs
  - Run and Submit
  - environment creation and provisioning
  - managed pack identity, where the newest version is chosen by numeric
    version order
  - backup validation

  Stored rows are trusted local metadata, like history, and are not re-hashed
  on read. The digest remains the pack identity.
- Migration: when state opens, each directory in a legacy
  `packs/sha256/<digest>` registry is loaded, structurally validated and
  checked against its folder name. Runtime validation already passed on the
  original import and is not repeated. Each valid pack is inserted. The
  registry is removed only when every pack migrated. Otherwise it is kept
  (with a logged digest) and retried on the next start.

## Consequences

- Listing, judging and provisioning no longer touch the filesystem or re-hash
  packs.
- Pack content is only as tamper-evident as the local SQLite file, the same
  trust level as history and environment metadata.
- `PackImporter::import`, `list_catalog` and `load_validated*` remain for the
  legacy registry and its tests. The app no longer publishes registry files.
