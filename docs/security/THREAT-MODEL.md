# Threat Model — SQL Judge Desktop

## Assets and trust boundaries

**Assets:** user's personal/production DBs and credentials, app SQLite history, local files/processes, trusted installed pack registry, sandbox integrity.

**Boundaries:** editor SQL -> native Tauri -> DB runner; ZIP/agent content -> package importer -> SQL fixture execution; external endpoint -> connection manager; UI delete button -> Rust environment-admin gate; release workflow -> signed distributed binary.

## Security invariants

| SEC ID  | Required property                                                                                                                                                                                                                                                     | Evidence/test                                                                                          |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| SEC-001 | User SQL never executed with admin/setup credentials                                                                                                                                                                                                                  | Integration test using forbidden write query                                                           |
| SEC-002 | Unmanaged/external databases are never reset/dropped                                                                                                                                                                                                                  | Decoy external instance fixture + attempted forged command                                             |
| SEC-003 | SQL policy checks are defense-in-depth; real isolation is dedicated PG process/OS permissions and DB least privilege                                                                                                                                                  | Security design review + functional exploit tests                                                      |
| SEC-004 | Password/token encrypted via native vault or memory-only, no plaintext in SQLite, logs, backups                                                                                                                                                                       | Secret canary test and search                                                                          |
| SEC-005 | Remote TLS validation defaults secure; opt-out explicit and marked unsafe                                                                                                                                                                                             | Invalid cert test                                                                                      |
| SEC-006 | Package validation prevents symlinks, hardlinks, path traversal, absolute path, duplicate entries, decompression bombs, cumulative compressed/expanded-byte exhaustion, excessive relative path length/depth, excessive filesystem/ZIP entries, unknown schema fields | Rust/Python directory hardlink and ZIP link-like metadata negatives plus bounded structural validation |
| SEC-007 | Resource ceilings cap runaway recursion, delay functions, huge results, too many active jobs                                                                                                                                                                          | Timeouts, count, payload and cancellation tests                                                        |
| SEC-008 | Destructive operations require server-side ownership verification and explicit intent; external sandboxes must additionally bind the saved profile and verified target identity to a generated marker-owned schema | Rust command invoked directly with forged UI args                                                      |
| SEC-009 | Crash/restart cleanup can only touch verified app-managed resources                                                                                                                                                                                                   | Forced termination integration test                                                                    |
| SEC-010 | No unintended network exposure: managed PG listens on unix socket only (Unix) or `127.0.0.1` with a Credential Manager scram password (Windows), dedicated credentials and restricted access                                                                                                                                          | Port/permissions review                                                                                |
| SEC-011 | SQL Server runner uses server-granted least privileges, not merely a UI read-only toggle                                                                                                                                                                              | Login with SELECT-only permissions rejects write                                                       |
| SEC-012 | Packaging verifies binary provenance, signatures/checksums, no secret-bearing build env leaks                                                                                                                                                                         | Release pipeline audit                                                                                 |

## Attack stories

- **Malicious SELECT:** PostgreSQL function or SQL Server procedure called from otherwise read-looking query can cause side effects or sleep. Mitigate with restrictive DB privileges, dedicated instance, statement/wall timeouts, OS-level process isolation, function/extensions permissions and blocked unsafe capabilities. A regex/read-only transaction alone is insufficient.
- **Wrong-target reset:** external server might contain business data. Even an advanced user may click wrong connection. Enforce explicit `managed` identity, dual marker/ledger checks and reject ambiguous targets; no `DROP ALL` API.
- **Pack poisoning:** AI-generated `schema.sql` has malicious DDL or path refers outside extraction root. Validate archive boundaries and execute fixtures only in disposable managed instance with file/network privileges restricted; warn user that offline imported packages are still untrusted.
- **Connection leakage:** `postgres://user:password@...`, JDBC URLs, or ADO `Pwd=`/`Password=` values appear in telemetry, logs or exception traces. Use secret references + scrub log fields and typed errors.
- **Runaway query:** large cross join, recursive CTE, output with unbounded row width. Limits at DB, runner and process; cancellation plus orphan cleanup.
- **Hidden tests:** all offline pack bytes can be inspected/modified. Must not market offline tests as secure against a determined attacker.

## Import safety minimum

Limit compressed bytes, total expanded bytes, entries, file size, nested depth; reject symlinks/hardlinks (including ZIP UNIX attributes), duplicate paths and casefold collisions; normalize and resolve each target inside staging. Skip OS metadata names (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `._*`, `__MACOSX/`) unread and outside the digest, after the symlink check. Reject `..`, absolute paths, NUL, unlisted files, unsupported schema versions; hash all unpacked files; atomic publish only after trusted test validation.

## Destructive action decision table

| Target                             | Requested action          | Behavior                                                  |
| ---------------------------------- | ------------------------- | --------------------------------------------------------- |
| Managed verified environment       | reset/rebuild/delete      | impact preview + explicit confirmation + locks + rollback |
| Unknown or external connection     | reset/rebuild/drop        | denied unconditionally                                    |
| Verified external sandbox schema   | reset/rebuild/delete      | exact profile/target/marker verification + confirmation    |
| Managed but marker/ledger mismatch | any destructive operation | denied, error + manual recovery guide                     |
| Running environment                | reset/delete              | blocked until cancel/completion                           |

## Validation is not authorization

Tauri frontend confirmation, SQL AST parse, filesystem path validation and connection test are **not** proof of privilege or ownership. Enforce in Rust and in OS/database identities. See [PostgreSQL docs](https://www.postgresql.org/docs/current/) and [Microsoft least privilege](https://learn.microsoft.com/en-us/sql/relational-databases/security/authentication-access/getting-started-with-database-engine-permissions).
