# Commerce Pack Runtime Import Repair

## Goal

Make `examples/commerce-analytics.sqljudge.zip` import successfully through the
same managed PostgreSQL runtime-validation path used by the desktop app.

## Scope

- Add a native regression test that starts the managed runtime and imports the
  distributable Commerce Analytics archive.
- Use the initial failed test to identify the exact problem and fixture rejected
  during runtime validation.
- Correct only the defective pack SQL or manifest metadata.
- Rebuild the distributable archive from `examples/packs/commerce-analytics`.

## Non-goals

- Do not weaken structural or runtime validation.
- Do not change importer error handling, publication ordering, or database
  privileges.
- Do not alter unrelated learning packs.

## Data Flow

The regression test calls `import_pack_value` with the Commerce Analytics ZIP.
The importer validates the archive structure, starts the managed PostgreSQL
runtime, executes pack schema and each fixture in isolated validation state,
then executes each reference query. A failure identifies the rejected
`problem/test` pair and prevents publication. After the package correction,
the same test must publish the pack successfully.

## Error Handling And Safety

The test uses only an app-managed temporary runtime and SQLite state. It does
not access an external database. Failed validation must leave no catalog record
for the package, preserving AC-009's atomic-import requirement.

## Verification

- RED: the new Commerce Analytics native import regression fails with the
  current runtime-validation error.
- GREEN: after the package repair, the regression publishes the archive.
- Run structural validation against both the source directory and ZIP archive.
- Run affected Rust formatting, clippy, and native test suites.

## Requirements

- REQ-009, REQ-010
- AC-008, AC-009
- SEC-006
