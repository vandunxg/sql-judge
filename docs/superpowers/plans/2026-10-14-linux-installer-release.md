# Linux Installer and Release Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a safe one-command Linux installer for SQL Judge, publish verifiable Linux release metadata, and align requirements, README, release operations, task tracking, and agent instructions.

**Architecture:** The installer is a POSIX shell script fetched over HTTPS. It queries the fixed GitHub repository's latest release, selects a matching DEB for Debian-family systems or AppImage for other supported Linux systems, verifies a SHA-256 entry published by CI, and uses `sudo` only for system installation. Release CI creates and uploads the checksum manifest alongside Linux artifacts.

**Tech Stack:** POSIX shell, curl, sha256sum/shasum/openssl fallback, GitHub Actions, Tauri 2 AppImage/DEB artifacts, Markdown documentation.

**Spec:** `docs/requirements/PRD.md` and `docs/requirements/ACCEPTANCE.md`, with release procedure in `docs/operations/BUILD-RELEASE.md`.

## Global Constraints

- Preserve the existing Bun/Tauri structure and release matrix; do not commit `dist/` or `src-tauri/target/` artifacts.
- Treat release metadata and downloaded artifacts as untrusted until HTTPS download and checksum verification succeed.
- Never run the whole installer as root; require explicit `sudo` only for system paths and package installation.
- Do not claim runtime self-containment: the app may still require a separately installed local PostgreSQL runtime.
- Installer supports Linux `x86_64` first; unsupported architectures fail with an actionable message instead of selecting the wrong binary.
- Existing `v0.1.0` Tauri version is the release tag used for the first published release.

## Review Focus

- GitHub API has no matching release/asset: installer exits before changing the system.
- Checksum is missing or mismatched: installer deletes the temporary download and exits non-zero.
- Debian-family package installation fails: no fallback silently installs an unverified artifact.
- Non-Debian distro lacks desktop integration or AppImage runtime support: installer reports the limitation and leaves a verified executable only if installation completed.
- `sudo` is unavailable or declined: user installation remains possible only through an explicit documented mode; the default system installer fails closed.

---

### Task 1: Installer behavior tests and Linux installer script

**Files:**
- Create: `scripts/install-linux.sh`
- Create: `scripts/test_install_linux.sh`
- Modify: `.gitignore` only if the test needs a temporary fixture path (prefer `mktemp` and no ignore change)

**Interfaces:**
- Consumes: GitHub Releases API and release asset URLs from `vandunxg/sql-judge`.
- Produces: `scripts/install-linux.sh [--user|--system]`, defaulting to `--system`; exit code non-zero on unsupported platform, unavailable release, failed checksum, or failed install.

- [ ] **Step 1: Write failing shell tests** for option parsing, unsupported architecture, checksum failure, and rejecting an empty/missing release response. The test harness must stub `curl`, `sudo`, and platform commands through a temporary `PATH`; it must not contact GitHub or modify `/usr`.
- [ ] **Step 2: Run the focused test** with `bash scripts/test_install_linux.sh`; expected RED because `scripts/install-linux.sh` does not exist.
- [ ] **Step 3: Implement the minimum installer** with `set -eu`, fixed repository constants, temporary directory cleanup trap, API asset selection, architecture guard, checksum verification using available system checksum tools, and explicit `--user`/`--system` paths. Use `dpkg -i` plus `apt-get -f install` only for a verified DEB; use a verified AppImage plus desktop entry/symlink for fallback.
- [ ] **Step 4: Run the focused tests** again; expected GREEN, including proof that no `sudo` call occurs in `--user` mode and no installation occurs after checksum failure.
- [ ] **Step 5: Manually shell-parse the script** with `bash -n scripts/install-linux.sh scripts/test_install_linux.sh`.

### Task 2: Publish Linux checksums from release CI

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `docs/operations/BUILD-RELEASE.md`

**Interfaces:**
- Consumes: Linux artifacts produced by the existing Tauri action.
- Produces: A stable checksum asset named `sql-judge-linux-x86_64.SHA256SUMS` uploaded to the same GitHub release, covering the Linux AppImage and DEB assets.

- [ ] **Step 1: Add a CI assertion/documentation test** that checks the workflow contains the Linux-only checksum step, fixed checksum filename, and `gh release upload` command.
- [ ] **Step 2: Run the assertion before the workflow edit**; expected RED because the checksum publication does not exist.
- [ ] **Step 3: Add the Linux-only CI step** after Tauri packaging: locate the generated AppImage/DEB, generate SHA-256 lines with stable basenames, and upload the manifest using the release tag from the Tauri action output. Fail if either artifact is absent.
- [ ] **Step 4: Run the assertion and YAML/format checks**; expected GREEN. Do not claim a real release artifact until GitHub Actions has run.
- [ ] **Step 5: Update the operations runbook** with the exact manual build commands, artifact paths, checksum requirement, `curl` installer command, `--user` and `--system` behavior, and PostgreSQL runtime caveat.

### Task 3: Requirements, README, task queue, and repository instructions

**Files:**
- Modify: `docs/requirements/PRD.md`
- Modify: `docs/requirements/ACCEPTANCE.md`
- Modify: `README.md`
- Modify: `tasks/todo.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: The tested installer interface and CI checksum contract from Tasks 1–2.
- Produces: Traceable release requirements and user-facing installation instructions without claiming unverified distro coverage.

- [ ] **Step 1: Add requirement/acceptance text** for one-command Linux installation, checksum verification, architecture failure, and system/user installation boundaries; link it to a new release task.
- [ ] **Step 2: Update README** with the recommended `curl` command, explicit trust/security warning, supported artifact selection, manual fallback, uninstall locations, and missing PostgreSQL runtime note.
- [ ] **Step 3: Update `AGENTS.md`** with the installer safety contract and release verification rule so future agents do not bypass checksum or publish unsupported platform claims.
- [ ] **Step 4: Add a `tasks/todo.md` entry** with the new REQ/AC IDs and a pending status until a real GitHub Actions release and install smoke test are observed.
- [ ] **Step 5: Run Markdown link/path checks** for referenced script, workflow, and operations files; expected all referenced repository paths exist.

### Task 4: Verification, commit, tag, and push

**Files:**
- Modify: only files from Tasks 1–3; no generated artifacts.

**Interfaces:**
- Consumes: Green focused tests and clean documentation checks.
- Produces: Git commit on a named branch, annotated `v0.1.0` tag, and pushed branch/tag if the remote accepts the operation.

- [ ] **Step 1: Run focused checks:** `bash scripts/test_install_linux.sh`, `bash -n scripts/install-linux.sh scripts/test_install_linux.sh`, workflow assertion, and `git diff --check`.
- [ ] **Step 2: Run affected project checks:** `bun run typecheck`, `bun run build`, Rust tests/checks appropriate to the existing checkout, and record any pre-existing failures without masking them.
- [ ] **Step 3: Review `git diff` and `git status`** to confirm only intended source/docs/workflow files changed and no secrets or build artifacts are present.
- [ ] **Step 4: Commit** with `feat(release): add verified linux installer`.
- [ ] **Step 5: Create annotated tag** `v0.1.0` only if that tag does not already exist locally/remotely; otherwise stop and report the conflict rather than moving it.
- [ ] **Step 6: Push the commit and tag** to `origin`, then report the GitHub Actions workflow URL and the fact that artifact/install smoke remains pending until CI completes.

## Self-Review Coverage

- Installer path selection, checksum failure, architecture rejection, and root-boundary behavior are covered by Task 1 tests.
- CI checksum publication and artifact presence are covered structurally by Task 2; actual packaging remains a GitHub runner responsibility.
- User-facing instructions, requirements, and future-agent constraints are covered by Task 3.
- Tag immutability and absence of generated artifacts are checked before Task 4 push.
