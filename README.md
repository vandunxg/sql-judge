# SQL Judge

SQL Judge is an offline-first desktop application for practicing SQL against validated problem packs and isolated local PostgreSQL fixtures. It combines an editor, schema context, bounded result previews, typed verdicts, submission history, managed environments, and read-only external database exploration.

> Status: active development. The app is designed for local learning workflows, not for tamper-proof remote examination or production database administration.

## Product goals

- Practice PostgreSQL SQL with pack-owned schema, public samples, and hidden tests.
- Keep judging deterministic: each submission runs in a clean, constrained fixture context.
- Keep secrets local: credentials do not enter frontend storage, SQLite, logs, backups, or commits.
- Make external database access explorer-only and read-only by default.
- Provide explicit, ownership-verified lifecycle actions for managed environments only.

## Interface preview

The previews below are generated from the approved design artboards in `docs/design/`. They are visual reference material, not shipped runtime markup.

| Home | Problems |
| --- | --- |
| ![Home design](docs/design/previews/home.png) | ![Problems design](docs/design/previews/problems.png) |
| Solve | Managed environments |
| ![Solve design](docs/design/previews/solve.png) | ![Environments design](docs/design/previews/environments.png) |

## Features

- Validated ZIP and directory problem-pack import with bounded metadata and immutable digest identity.
- Monaco SQL editor with local schema-aware completion, Run, Submit, cancellation, result limits, and typed verdicts.
- Disposable PostgreSQL fixture runner with constrained roles, time/row/result-byte limits, rollback cleanup, and safe error mapping.
- Managed environment creation, clone, inspect, reset, delete, ownership markers, durable recovery, and explicit destructive confirmation.
- Submission history, progress, and metadata-only backup/restore with exact pack identity validation.
- PostgreSQL and SQL Server Explorer flows that enforce read-only access and TLS-safe policies.

## Architecture

| Area | Responsibility |
| --- | --- |
| `crates/judge-core` | Pure SQL judging contracts, comparison policy, and verdict aggregation. |
| `crates/judge-postgres` | PostgreSQL runners, fixture isolation, result decoding, and local runtime control. |
| `crates/problem-pack` | Pack validation, immutable registry, and safe import lifecycle. |
| `crates/environment-manager` | Managed-environment ownership and lifecycle policy. |
| `src-tauri` | Thin typed command boundary, authorization, policy enforcement, and error mapping. |
| `src/` | React desktop UI, local state, accessibility, and typed IPC clients. |

Read `docs/architecture/ARCHITECTURE.md` and `docs/security/THREAT-MODEL.md` before changing boundary-crossing behavior.

## Prerequisites

- [Bun](https://bun.sh/) for the frontend toolchain.
- Rust stable toolchain with Cargo.
- Tauri 2 platform prerequisites for your operating system.
- PostgreSQL binaries for local runtime and integration tests.

SQL Server fixture tests are opt-in and require a configured Docker/TLS harness. They are not run by the default test suite.

## Quick start

```bash
bun install --frozen-lockfile
bun run tauri dev
```

The app opens on Home. Import a validated pack from **Packs**, then open **Practice** to run or submit a SQL solution. The local runtime is explicit: never point it at a production database.

## Install the latest Linux release

The official release provides a checksum-verified installer for Linux
`x86_64`. It uses the `.deb` on Debian/Ubuntu-family systems and the
`.AppImage` on other Linux distributions. Review the script before piping it
to a shell in a sensitive environment:

```bash
curl -fsSL https://raw.githubusercontent.com/vandunxg/sql-judge/main/scripts/install-linux.sh | bash
```

The default installs system-wide with `sudo`. To install only for the current
user without `sudo`:

```bash
curl -fsSL https://raw.githubusercontent.com/vandunxg/sql-judge/main/scripts/install-linux.sh | bash -s -- --user
```

The installer supports Linux `x86_64` only. It refuses
to install when the release checksum is absent or invalid. Installing SQL
Judge does not install PostgreSQL; configure the required local PostgreSQL
runtime separately before running judged exercises. SQL Judge finds PostgreSQL
14+ on PATH or in the default Homebrew, Postgres.app, Windows installer
(`C:\Program Files\PostgreSQL\<version>\bin`) and Debian/RHEL locations.

## Development

```bash
# Frontend
bun test
bun run typecheck
bun run lint
bun run build

# Rust workspace and Tauri boundary
cargo test --workspace
cargo test --manifest-path src-tauri/Cargo.toml

# Disposable PostgreSQL harness
bun run test:pg-harness
```

Use `bun run tauri dev` for the desktop shell. Do not claim platform packaging support without running the matching artifact build and install smoke test.

## CI and releases

CI (`.github/workflows/managed-runtime.yml`) runs only on pushes to `main` and `release/**` that touch the runtime, pack, app or example paths, plus manual runs from the Actions tab. Other branches and pull requests do not start CI, so run the affected suites above before merging. See ADR-029.

Release changes are listed in `CHANGELOG.md`. To cut a release:

1. On a `release/vX.Y.Z` branch, set the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and the `tauri-native` entry of `src-tauri/Cargo.lock`.
2. Add a dated `## [X.Y.Z] - YYYY-MM-DD` section at the top of `CHANGELOG.md`.
3. Check both release contracts:

   ```bash
   bash scripts/test_release_version.sh
   bash scripts/test_ci_workflow.sh
   ```

4. Commit, tag `vX.Y.Z` with the changelog section as its message, and push the branch and the tag. The tag starts `.github/workflows/release.yml`, which builds every platform and publishes the release once all assets are uploaded.

## Using problem packs

Problem packs are untrusted input. The importer rejects traversal, links, malformed manifests, oversized archives, unsafe setup, and invalid metadata before publication. Pack-provided setup executes only in trusted ephemeral contexts; user SQL never executes with provisioner credentials.

See `docs/contracts/PROBLEM-PACK.md` for the package contract and `examples/` for reference material.

## Security model

- User SQL executes under constrained runner identities with bounded resources.
- External connections default to read-only Explorer mode. No implicit schema creation, reset, or deletion occurs on external servers.
- Destructive managed lifecycle operations require a verified ownership marker, scoped resource identity, displayed impact, and explicit confirmation in both UI and Rust.
- Secrets stay in the OS credential vault or in-memory session fallback. They must never be added to source control, logs, JSON, SQLite, or frontend storage.

The complete policy is in `docs/security/THREAT-MODEL.md`.

## Contributing

1. Read `AGENTS.md`, `docs/requirements/PRD.md`, `docs/requirements/ACCEPTANCE.md`, and the relevant contract before editing.
2. Check `git status`, current tasks in `tasks/todo.md`, and existing tests. Preserve unrelated work in a dirty tree.
3. Follow Red, Green, Refactor. Add a meaningful failing test before behavior changes; run focused tests before broad verification.
4. Preserve architecture boundaries: core policy remains pure, Tauri handlers remain thin, and database adapters remain capability-specific.
5. Treat frontend artboards under `docs/design/` as visual contracts. Match their stated tokens and layout through React components; do not copy the exported HTML or vendor runtime.
6. Run the affected test suites, formatting/lint, typecheck, and build. For database work, include integration and negative/security coverage.
7. Document task evidence and limitations in `tasks/todo.md`. Never mark visual fidelity complete without a rendered comparison to the artboard.

See `docs/agents/WORKFLOW.md` for task handoff and commit expectations.

## Documentation map

- Requirements and acceptance: `docs/requirements/`
- Architecture and ADRs: `docs/architecture/`
- Protocol and pack contracts: `docs/contracts/`
- Security model: `docs/security/`
- Testing rules: `docs/testing/`
- Design system and screen artboards: `docs/design/`
- Operations and release guidance: `docs/operations/`
- Release history: `CHANGELOG.md`

## License

No license has been declared for this repository.
