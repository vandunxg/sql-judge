# ADR-018: External PostgreSQL Explorer Capability

- Status: Accepted
- Date: 2026-10-13
- Requirements: REQ-014, REQ-015
- Acceptance: AC-014, AC-015
- Security: SEC-002, SEC-004, SEC-005, SEC-007

## Context

SQL Judge connection profiles need a safe way to inspect user-provided
PostgreSQL databases. The managed PostgreSQL runner is an owned disposable
target with lifecycle and judging permissions; an external database is not
owned and must never be reset, provisioned, or judged.

The existing PostgreSQL session is intentionally loopback-only and supports
managed fixture/provisioning operations. Reusing it for external connections
would blur the security boundary and make it easy to apply managed operations
to an arbitrary server.

## Decision

Implement a separate `ExternalPostgresExplorer` capability. It exposes only
connection test, catalog metadata inspection, and bounded read-only SELECT.
It accepts profile metadata plus a secret only in native memory and never
returns or logs a connection string or credential.

Use the PostgreSQL TLS adapter with system certificate roots. `verify_full`
requires certificate and hostname validation. `verify_ca` retains certificate
chain validation and explicitly relaxes only hostname matching. `require`
remains encrypted and certificate-verified. `disable` is rejected unless a
future explicit local-development opt-in contract is added.

Each operation opens a fresh connection, applies server-side statement and
result limits, and executes SELECT inside a read-only transaction. SQL parsing
is defense-in-depth only; the database role and transaction mode remain the
primary write-protection boundary.

## Alternatives Rejected

- Reusing `ManagedPostgresSession`: mixes owned-resource lifecycle operations
  with arbitrary external targets.
- UI-only read-only enforcement: forged IPC or a malicious client could still
  issue writes.
- Parser-only protection: PostgreSQL functions and side effects can bypass a
  simplistic read-only parser.
- `NoTls` by default: violates the external TLS verification contract.

## Consequences

- External network failure is isolated from local learning and managed runtime.
- The adapter needs disposable external PostgreSQL integration fixtures and
  typed error mapping for auth, TLS, offline, policy, timeout, and limits.
- SQL Server remains a distinct adapter in T21; no PostgreSQL assumptions are
  exported as a generic database provider.

## Amendment (2026-10-14): TLS fallback and default TEMPORARY privilege

- Every TLS mode sets the driver's `sslmode=require`. The driver default,
  `prefer`, silently continued in plaintext when a server declined TLS, so
  `verify_full` could connect unencrypted. A server without TLS now fails with
  `tls_failed`. `disable` is still rejected for external profiles.
- The read-only check no longer treats the database `TEMPORARY` privilege as a
  write capability. PostgreSQL grants it to `PUBLIC` by default, so every
  SELECT-only role failed the connection test. Temporary objects are
  session-local and cannot be created inside the Explorer's `READ ONLY`
  transactions. Database `CREATE` and every other write privilege still fail
  the check.
