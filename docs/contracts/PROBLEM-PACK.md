# Problem Pack Contract — `sqljudge.pack/v1`

## Packaging
Root of package contains exactly `pack.json`, `schema/*.sql`, `problems/<problem-id>/problem.json`, `reference.sql`, and `tests/<id>.sql`, optional `README.md` and `LICENSE`. Pack archives should use `.sqljudge.zip`; root entries at ZIP root, not nested arbitrary directory. Use UTF-8 text. Allowlisted paths only. No scripts/commands executed by importer. Manifest is JSON (rather than YAML) to avoid parser ambiguity; human-readable README may be Markdown.

## Machine schema
`schemas/pack.schema.json` defines root manifest; `schemas/problem.schema.json` defines problem. Importer must use strict JSON Schema validation with `additionalProperties: false`. Paths are **relative, POSIX style**, resolved within staging root and forbidden from escaping it. `problem.json` has one `reference` path and `tests` listing complete seed SQL files, each applied after pack schema. `engine` is `postgresql` or `sqlserver`, but MVP managed judging accepts PostgreSQL only. Future SQL Server pack import may browse metadata while `judge` capability is disabled.

## Pack example
```
hr-analytics/
├── pack.json
├── schema/001_tables.sql
└── problems/second-highest-salary/
    ├── problem.json
    ├── reference.sql
    └── tests/
        ├── basic.sql
        ├── salary-ties.sql
        ├── null-salaries.sql
        └── empty-department.sql
```
The reference pack is `examples/packs/hr-analytics` (eight problems); its distributable archive is `examples/hr-analytics.sqljudge.zip`. Test fixtures insert data into freshly created pack schema; no fixture should depend on SQL from other test fixtures.

## Idempotence and versioning
- Identity: `(pack.id, pack.version, content_sha256)`. Distinct hash with same ID+version is rejected (immutable version); unchanged hash may be a no-op.
- SemVer pack.version, immutable installed content keyed by digest, never silently mutate installed pack version. Installed content lives in the local SQLite catalog (`packs`, `pack_problems`, `pack_problem_tests`), written in one transaction after structural and runtime validation (ADR-025). Deleting a pack requires explicit confirmation, deletes its managed environments through the ownership-verified path, and keeps submission history.
- Each installed pack version gets an app-managed sandbox environment (`sj_pack_<digest prefix>`) provisioned with the pack schema. Run loads each public fixture into it inside a rolled-back, read-only transaction; Submit uses disposable fixture schemas (ADR-026).
- Each submission stores pack ID, version, hash, PostgreSQL server version and policy version.
- Import: stage -> validate paths + JSON contract -> dialect check -> create ephemeral env -> apply schema and fixtures per test -> check reference execution + output shape -> persist files/metadata atomically. On failure preserve previously installed versions and previous active env.

## Contract details
- Each problem declares `statement`, `difficulty`, `topics`, `dialect`, `reference`, `tests`, `output` and `limits`.
- `output.row_order`: `ordered` or `unordered`; `strict_column_names` and `strict_column_types` are required booleans to prevent ambiguous grading.
- `output.columns`: ordered list with `name`, `type`, so users know exact expected shape. SQL type matching maps only through a documented dialect-specific canonicalizer.
- `tests[*].hidden` controls UI visibility only, not true secrecy on offline desktop; always include one public and at least two hidden adversarial cases.
- Pack-level `schema` SQL files are ordered. Each fixture is self-contained and applied to *empty schema*; not appended on previous fixture.
- Optional `catalog` is display-only metadata exposed to the UI and never used for judging: `schema` (table/column descriptions), `sample_rows` (output of the public fixture), `starter_sql` (may be multi-line: `\n`, `\r\n` and `\t` are the only allowed control characters; should be a starting point, not an accepted answer) and `sample_tables` (example input: at most 8 `{ name, rows }` tables, names unique and declared in `catalog.schema` when it is non-empty, at most 20 flat scalar rows each). Keep sample data identical to the public fixture. See ADR-022.
- The native catalog also exposes `outputColumns`, `rowOrder`, `timeLimitMs` and `fixtureCount` derived from the manifest; hidden fixture contents never cross the IPC boundary.
- Global fixtures/datasets can be added by **new spec version**, not ad-hoc undocumented fields.
- Reject ambiguous order-sensitive tasks without deterministic tie-break keys. State output grain explicitly in `statement`.

## Validation report (target API)
```json
{
  "pack_id": "hr-analytics",
  "version": "1.2.0",
  "structural_status": "valid",
  "database_status": "not_run",
  "diagnostics": []
}
```
`structural_status=valid` never means references/fixtures have executed. Production import must pass both stages before status `installed`.

## Migration rules
Spec changes require ADR + tests + version bump (`v2`) + existing pack migration or explicit unsupported version error. Never silently accept unknown properties, unrecognized SQL dialects, missing references, or unspecified comparator options.
