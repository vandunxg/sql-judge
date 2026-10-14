# Database Connection & Environment Contracts (v1)

## Terminology — never conflate
- **Connection**: user-specified endpoint/auth to PostgreSQL or Microsoft SQL Server.
- **Explorer connection**: by default read-only, can browse metadata/run bounded SELECT, *not* used for judge provisioning/reset.
- **Managed instance**: local PostgreSQL service controlled by SQL Judge in a dedicated OS/container account, with no access to user databases.
- **Pack environment**: named, versioned dataset/schema associated with installed pack; app owns lifecycle.
- **Execution environment**: short-lived clean state for one test; references and user SQL see equivalent snapshots.
- **Practice external namespace**: an explicit PostgreSQL opt-in that creates a generated SQL Judge-owned schema on a saved connection; it is never inferred from an Explorer connection test.

## Connection profile (metadata only)
```json
{
  "id": "conn_local_pg",
  "name": "Local PG Explorer",
  "engine": "postgresql",
  "host": "127.0.0.1",
  "port": 5432,
  "database": "learning",
  "username": "sql_reader",
  "tls_mode": "verify-full",
  "mode": "explorer",
  "secret_ref": "os-vault-reference-only"
}
```
Do not log/serialize actual password. Support on-session secret fallback if desktop secret store unavailable. Fail TLS host/certificate checks rather than default to `trust_server_certificate=true`. Local development exception requires visible per-profile warning and explicit opt-in.

## Capability matrix
| Operation | Managed PG | External PG Explorer | External PG Sandbox | External MSSQL Explorer |
|---|---|---|---|
| Test connection | yes | yes | capability probe | yes |
| Metadata inspect | yes | least-privilege | least-privilege | least-privilege |
| Read sample SELECT | yes | yes with limits | no | yes with limits |
| Submit judge | yes | **no** | **no** | **no** |
| Create isolated pack env | yes | **no** | generated marker-owned schema only | **no** |
| Reset/rebuild/drop | owned resources only | **no** | exact profile/target/marker only | **no** |
| Clone env | yes | **no** | same verified target only | **no** |

## Managed environment ownership and destructive gate
Each environment ledger includes `env_id`, `instance_id`, `pack_id`, `pack_version`, `namespace`, `ownership_nonce`, schema fingerprint, initial seed hash, state. Managed PG must have a separate server identity, never alias a user external connection. Ownership marker stored both in app SQLite ledger and in app-managed DB namespace; both must match along with connection identity before destructive operation.

Reset/rebuild requires: `mode=managed`, `ownership=verified`, env lock, scoped impact summary, explicit UI confirmation, Rust revalidation, new environment created+tested first, pointer swap atomically, old env deleted only after swap. No arbitrary `DROP SCHEMA public CASCADE`/blanket table deletion. If verification fails: abort and preserve existing data. User cannot edit ownership through frontend DTO.

An external sandbox additionally records the profile ID and immutable server/database identity. It may only create a generated schema after a dedicated writable-sandbox capability probe and explicit confirmation. Every reset/delete reconnects through the saved profile and requires the profile, target identity, schema name, nonce, and database marker to match. Existing external schemas are never adopted.

## Provider interface strategy
Split `MetadataReader`, `ReadQueryExecutor`, `ManagedEnvironmentProvisioner`, and `JudgeQueryExecutor` into capabilities. Do not place `drop_all_tables()` in a generic DatabaseProvider. MSSQL and PostgreSQL have different types, schema qualifiers, catalog queries, T-SQL vs SQL syntax, timeout and permissions. Distinct adapter tests required.

## Startup and degradation
Local managed runtime: detect status, version, initialized owned data directory and required permissions. When absent, allow read-only UI/history/pack browsing; show actionable runtime installation guidance. If future bundled runtime is shipped, platform-specific binary provenance, native licenses, installation path safety, OS sandboxing, upgrades, uninstaller data retention and signature validation require verified implementation and artifacts. External network unavailability is nonfatal to other workspaces.

## Environment lifecycle states
`absent -> provisioning -> ready -> running -> ready`, `ready -> rebuilding -> ready`, `ready -> deleting -> deleted`, errors -> `needs_recovery`. Restart reconciler only inspects app-owned marker resources. Idempotent retries + cancellation; preserve last-known-good on failed rebuild. Retain submission history independently from environment lifecycle.

## Offline decision
Offline-first means **the desktop application, bundled/problem content, editor, local SQLite history and managed local SQL evaluation can operate without Internet** if the selected platform includes or already has a local PostgreSQL runtime. Remote PostgreSQL/SQL Server connections inherently need network reachability and aren't offline.
