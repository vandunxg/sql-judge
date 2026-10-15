# ADR-013: Managed PostgreSQL Process Runtime

- Status: Accepted
- Date: 2026-10-12

## Decision

The managed PostgreSQL prototype starts an app-owned cluster with `initdb` and
`pg_ctl` using argument arrays, stores it under a caller-provided app data
directory, and exposes only a Unix-domain socket connection. TCP listening is
disabled. The runtime initializes an admin-owned instance marker and a
`NOLOGIN` constrained runner role before returning its connection string.

The runtime fails closed when the data path is not absolute, already exists,
required binaries are unavailable, the process cannot start, or marker setup
fails. Dropping the runtime stops only the process it started; data-directory
retention and platform packaging remain explicit lifecycle decisions.

## Rationale

This supplies a disposable, app-owned target for pack provisioning and judging
without credentials or accidental access to an external database. The marker
is then required by `ManagedPostgresSession`, so a loopback connection alone
cannot authorize pack import.

## Consequences

- Clean-machine packaging of `initdb`/`pg_ctl` is still T24 work.
- The runtime currently targets Unix-socket PostgreSQL environments; Windows
  process/service packaging needs a separate verified implementation.
- Tauri command wiring, crash reconciliation, and persistent lifecycle state
  remain T10/T13/T14 work.

## Amendment 2026-10-15: macOS and Windows desktop runtime

Pack import failed on macOS and Windows because the runtime only worked from a
Linux shell. The runtime now:

- **Finds the server tools without PATH.** GUI apps launched from Finder or the
  Start menu do not inherit the shell PATH, and keg-only Homebrew, Postgres.app,
  EDB and Debian installs never add `initdb`/`pg_ctl` to it. PATH is searched
  first, then the fixed install locations of those installers (newest major
  first, relative PATH entries ignored). An existing cluster only uses tools whose
  `pg_ctl --version` major matches its `PG_VERSION`, so installing a newer
  Homebrew/EDB major beside the old one does not break it. These are the same
  trust level as the user's PATH; nothing is downloaded or bundled.
- **Initialises UTF-8 with the `C` locale.** Finder-launched apps have no `LANG`,
  which made `initdb` choose `SQL_ASCII` (where `length()` counts bytes). Existing
  clusters keep their encoding.
- **Runs the tools with `LC_ALL=C` on macOS.** With no locale variables the
  postmaster aborts with "postmaster became multithreaded during startup"; the
  macOS runner reproduced it under a Finder-like environment.
- **Quotes every connection value.** macOS app data lives under
  `Application Support`; unquoted socket paths were split at the space. The
  loopback policy check now parses with the driver's own `Config` parser, so it
  checks exactly what will be connected.
- **Moves an over-long socket directory.** `sockaddr_un` holds 103 path bytes on
  macOS. When `<root>/socket` would exceed it, the socket lives in
  `$TMPDIR/sqljudge-<FNV-1a of root>` instead (Linux allows 107 bytes). That
  directory is created `0700`; on start and on adoption an existing one must be a
  real directory (not a symlink) owned by the runtime root's owner.
- **Treats an empty root as fresh.** A start that failed before `initdb` ran left
  an empty directory that permanently failed policy checks. A missing root is
  still claimed with `create_dir`; a failed `initdb` is not followed by
  `remove_dir_all` (initdb removes what it wrote), and a failed first start only
  removes the root when no server is running in it, so a concurrent first launch
  cannot delete the winner's cluster.
- **Joins pack-internal paths with `/` (problem-pack).** Problem reference and
  fixture paths were built with `Path::join`, which inserts `\` on Windows, so
  every pack failed structural validation there. Found by the Windows runner.
- **Uses loopback TCP on Windows.** rust-postgres has no Unix sockets on Windows
  and peer authentication does not exist there. The cluster is initialised with
  `scram-sha-256`, listens on `127.0.0.1` only, and uses a port chosen free on
  every start (never a fixed 5432, which the EDB service usually holds); adoption
  reads it back from `postmaster.pid`. The random superuser password (two
  UUIDv4, 244 random bits) is stored only in the Windows Credential Manager
  under `sql-judge-managed-runtime`. It is built
  directly from `windows-native-keyring-store` so the app-wide keyring default is
  untouched. It is created once (an existing entry is reused) and saved before
  `initdb` runs, so neither a crash nor a concurrent first launch can leave a
  cluster nobody can log in to. The password touches disk once, as an
  `initdb --pwfile` inside the user-private app data directory, deleted immediately after `initdb` exits.
  Tool processes run with `CREATE_NO_WINDOW`.

- **Imports off the main thread.** `import_pack` is an async command that runs
  the import on its own OS thread: a plain Tauri command runs on the main thread
  and froze the window through initdb and validation, and the sync postgres
  client cannot run inside Tauri's async runtime (dropping its Tokio runtime
  there panics, which aborts the release build).

The instance marker and `NOLOGIN` runner role are unchanged on every platform.
`.github/workflows/managed-runtime.yml` (on pushes to `main` and `release/**`,
see ADR-029) runs the runtime and native import tests on `macos-latest` and `windows-latest` with PostgreSQL left off PATH (and again
on macOS under launchd's GUI environment), since these platforms cannot be
exercised on a Linux workstation. It also uploads the NSIS installer and both
DMGs as workflow artifacts and installs `examples/hr-analytics.sqljudge.zip`
through the packaged app's own Packs screen: Accessibility scripting on macOS
(Apple Silicon and a native Intel runner), UI Automation on Windows after a
silent NSIS install, and WebKitWebDriver on Linux. A lost Credential
Manager entry leaves a Windows cluster unreachable; recovery is a manual
deletion of `postgres-runtime`.
