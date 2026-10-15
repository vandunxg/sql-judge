# Changelog

Notable changes to SQL Judge, newest first. Each version matches the app
version in `src-tauri/tauri.conf.json` and is released from the tag `vX.Y.Z`.

## [0.1.1] - 2026-10-15

### Fixed

- Pack import works on macOS. Connection values are quoted, so the
  `Application Support` data directory no longer breaks the socket path.
  PostgreSQL tools are found outside the shell `PATH` (Homebrew including
  keg-only formulae, Postgres.app, the EDB installer and Debian/RHEL
  locations) and run with `LC_ALL=C`, so an app launched from Finder no longer
  aborts with "postmaster became multithreaded during startup". Socket paths
  that are too long move to a private temporary directory, and a runtime
  directory left empty by a failed start is set up again.
- Pack import works on Windows. The managed PostgreSQL runtime listens on
  loopback TCP with `scram-sha-256`, a free port per start and a random
  password kept in Windows Credential Manager. Pack file paths use forward
  slashes, so imports no longer fail with "Pack failed structural validation."
- Packs zipped or copied on macOS or Windows import cleanly: `.DS_Store`,
  `__MACOSX/`, `._*`, `Thumbs.db` and `desktop.ini` are skipped. Symlinks are
  still rejected.
- New managed PostgreSQL clusters use UTF-8 with the C locale instead of
  `SQL_ASCII`.
- The window stays responsive while a pack imports; the import runs on its
  own thread instead of the main thread.
- Cancelling a judge run, or hitting its time limit, now stops the query: the
  cancel request is resent every 100 ms until the query ends instead of being
  sent once.
- Windows release builds compile on stable Rust (hard-link checks use a stable
  Windows API) and bundle SQLite instead of needing `sqlite3.lib`.
- The release workflow uploads to an existing release without replacing its
  notes, keeps a new release as a draft until every platform has uploaded,
  and names Linux assets `x86_64`.

### Changed

- CI runs only on pushes to `main` and `release/**`, and on manual runs.
  Other branches and pull requests no longer start CI.
- CI runs the runtime and pack-import tests on macOS and Windows with
  PostgreSQL off `PATH`, builds the Windows NSIS installer and both macOS
  DMGs, and installs an example pack through the packaged app's UI on macOS
  (Apple Silicon and Intel), Windows and Linux.

### Known limitations

- SQL Judge does not bundle PostgreSQL. Install PostgreSQL 14 or later
  before judging.
- The one-command Linux installer supports `x86_64` only.

## [0.1.0] - 2026-10-14

First release: validated problem-pack import, a Monaco SQL editor with Run,
Submit and typed verdicts, managed PostgreSQL environments, submission
history with backup and restore, read-only PostgreSQL and SQL Server
Explorer, and a checksum-verified Linux installer.

[0.1.1]: https://github.com/vandunxg/sql-judge/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/vandunxg/sql-judge/releases/tag/v0.1.0
