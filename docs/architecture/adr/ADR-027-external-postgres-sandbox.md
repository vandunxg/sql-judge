# ADR-027: External PostgreSQL Sandbox Environments

- Status: Accepted
- Date: 2026-10-14
- Requirements: REQ-022, REQ-015
- Acceptance: AC-021
- Security: SEC-002, SEC-004, SEC-005, SEC-008

## Decision

An external PostgreSQL sandbox is an explicitly confirmed environment on a saved PostgreSQL profile. It creates only a generated `sj_external_*` schema. The schema contains an ownership marker bound to the environment ID, profile ID, nonce, database name/OID, and PostgreSQL system identifier.

Creation, reset, and deletion reconnect through the stored vault-backed profile and fail closed unless the durable binding, connected target identity, and marker all match. Existing schemas are never adopted, and cleanup never drops a database or an unmarked schema. Profile deletion is rejected while an external sandbox references it.

## Consequences

Explorer remains read-only. External sandbox provisioning is separate from Explorer and requires a writable capability probe, TLS verification, a displayed impact summary, and explicit confirmation.

The first implementation provisions and manages only the empty, marker-owned schema. It does not execute imported pack schema, fixtures, reference SQL, or user SQL against an external database. Those operations remain managed-runtime-only until a separate trusted setup/provisioner design is accepted and verified with a disposable TLS PostgreSQL fixture.
