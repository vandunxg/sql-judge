# External Sandbox Environments Design

## Status

Proposed on 2026-10-14. This design changes the existing external-connection contract and requires updates to the requirements, acceptance criteria, threat model, and an ADR before implementation.

## Goal

Allow a user to select a saved PostgreSQL connection when creating an environment, while limiting all writes, resets, and deletes to a schema created and owned by SQL Judge.

## Non-goals

- Adopt, reset, delete, or modify an existing external schema or database.
- Allow SQL Judge to operate against a connection that has not passed the writable-sandbox capability check.
- Grant arbitrary lifecycle access to profiles used only by the read-only Explorer.
- Change SQL Server external behavior in this scope.

## User Experience

The New environment dialog will use the project Select component for a required Database connection field. It lists saved PostgreSQL profiles with a stored credential. The dialog also retains the exact installed pack selector and namespace input.

After the user selects a profile, the app probes it for writable-sandbox capability. The dialog reports loading, success, or an actionable error. A profile that fails the probe cannot be used to create an environment.

The impact summary states the selected connection, target database, generated schema, and pack identity. Creation requires an explicit in-app confirmation. Environment rows show whether the environment uses the managed local runtime or an external connection and show the selected database.

The connection-profile delete action will replace `window.confirm` with the existing Base UI AlertDialog shell. Browser-native prompts must not be used for application actions; all confirmations use project dialog components with focus management and keyboard support.

## External Sandbox Lifecycle

1. The native command loads the exact saved profile and retrieves its credential from the vault.
2. The PostgreSQL adapter validates TLS and checks that the server permits a scoped sandbox. It does not execute user SQL.
3. The command generates a schema name from the environment ID and creates only that schema.
4. Trusted pack schema/setup runs only in the new schema using an explicitly constrained provisioner identity.
5. SQL Judge writes a protected ownership marker containing the environment ID, connection profile ID, server/database identity, schema name, and nonce.
6. The SQLite environment record persists the same pinned connection identity and marker data; it never stores a password.
7. Reset and delete reload the saved profile, revalidate TLS and database identity, and verify both the marker and durable record before touching the schema.

Any failed creation rolls back or removes only the schema created by this request after re-verifying its marker. No fallback may drop an unmarked schema, another schema, or a database.

## Authorization And Safety

External sandbox operations are distinct from Explorer operations. A profile may be read-only for Explorer and unavailable for environment creation. A writable profile is not automatically trusted: it must explicitly opt into sandbox creation and pass the server-side capability and identity checks.

The UI cannot select an arbitrary host, database, or schema. It sends a saved profile ID, exact pack identity, generated environment ID, and namespace. Rust revalidates all of them and treats frontend input as untrusted.

External profile edits invalidate external environment lifecycle access until the stored server/database identity is re-verified. A mismatched host, database, TLS policy, profile ID, marker, nonce, or environment record fails closed.

## Error Handling

Errors are structured and secret-free. The UI distinguishes unavailable credentials, TLS failure, authentication failure, offline server, denied sandbox privileges, identity mismatch, and ownership mismatch. An error never reveals a password, connection string, raw administrative SQL, or an unowned schema name.

## Verification

- Unit tests cover profile eligibility, generated schema validation, identity matching, and all marker/record mismatch cases.
- Disposable PostgreSQL integration tests prove creation, reset, and deletion affect only a marked sandbox schema.
- Negative tests prove that forged IDs, changed profile targets, missing credentials, denied schema privileges, existing unmarked schemas, and forged ownership data are rejected without modifying the target database.
- Frontend tests cover connection selection, probe states, disabled submission, impact summary, modal focus behavior, and replacement of native confirmation APIs.
- Manual desktop verification covers keyboard-only creation, confirmation, reset, deletion, and the Environments artboard at desktop and responsive widths.

## Requirements Impact

This intentionally expands REQ-022, which currently defers external writable sandboxes. Before implementation, update the requirement, AC-021, the external-connection ADR set, and the threat model with the bounded ownership model above. REQ-015 remains in force: external credentials stay in the vault, TLS is verified, and arbitrary external databases are never reset or judged.
