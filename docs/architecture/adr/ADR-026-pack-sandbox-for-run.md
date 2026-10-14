# ADR-026: Pack Sandbox Environment for Run

- Status: Accepted
- Date: 2026-10-14
- Amends: ADR-009 (ownership gate), ADR-012 (managed schema provisioning)
- Design: `docs/superpowers/specs/2026-10-14-pack-catalog-and-sandbox-design.md`

## Context

The user asked for importing a pack to create a dedicated environment with the
pack schema, and for every problem to use that environment automatically.
Judging never used managed environments. Every case built a disposable
`sj_fixture_*` schema and rolled it back.

The pack schema is shared by all of its problems. Data comes only from each
test's fixture, so different problems need different rows in the same tables.

## Decision

- Every installed pack version gets one managed sandbox, named from its
  digest:
  - `env_id`: `pack-<first 16 hex>`
  - namespace: `sj_pack_<first 16 hex>`

  It is provisioned through the existing ownership-marked path, which runs the
  pack schema as the constrained runner.
- Import provisions the sandbox after the catalog transaction commits. A
  failure never fails the import. The result reports `sandbox: "ready"` or
  `"pending"`, and the first Run creates a missing sandbox.
- Run judges each visible test inside the sandbox with
  `PostgresSession::with_namespace_fixture`, in one transaction per test:
  1. Verify the ownership marker and that the runner is constrained.
  2. `SET LOCAL search_path` to the sandbox and `SET LOCAL ROLE` to the runner.
  3. Load the fixture, which is validated by the setup SQL policy for that
     namespace.
  4. Switch to `transaction_read_only = on`.
  5. Run the reference SQL and the user SQL. The user SQL still passes
     `validate_read_only_sql`.
  6. Always `ROLLBACK`.

  The sandbox's committed state is always the pristine pack schema with empty
  tables. Problems never see each other's rows, and Run never needs a reset.
- A sandbox that exists but is not `ready` makes Run return `unavailable`. Run
  never recovers or resets it automatically, so destructive actions still
  require the user's confirmation on the Environments screen.
- Submit is unchanged. It keeps disposable fixture schemas, including hidden
  tests.
- The Solve screen prefers the sandbox when it shows the environment chip.
  Deleting a pack deletes its sandbox like any other environment of that pack.

## Consequences

- Run reuses a provisioned schema instead of creating a role, a schema and
  the tables for each fixture.
- The sandbox appears on the Environments screen as an ordinary managed
  environment. Deleting it is safe because the next Run recreates it.
- Backups list the sandbox like any other managed environment.
