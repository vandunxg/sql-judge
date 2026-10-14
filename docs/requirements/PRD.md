# PRD — SQL Judge Desktop (v1.0)

## 1. Objective and target users
Standalone desktop learning environment for developers studying PostgreSQL / T-SQL: author SQL in Monaco, explore schemas, run samples, submit to result-based judge, learn from reproducible results, import AI-generated versioned problem packs, provision isolated local exercises, and optionally explore databases provided by the user.

**Primary user:** SQL learner / backend developer on Linux; target distribution selected by existing tauri-ui scaffold: Linux AppImage/DEB, Windows NSIS/Portable, macOS Apple Silicon/Intel DMG. No cloud service/account needed. External database use depends on connectivity and permissions.

## 2. Scope — ordered capability map
| Module ID | Owns | Depends on | Phase |
|---|---|---|---|
| `pack-contract` | Versioned manifest, validator, import lifecycle | none | P0 |
| `judge-core` | Typed results, multiset/ordered compare, verdict policy | none | P1 |
| `db-postgres` | PostgreSQL metadata/query adapter + isolation | judge-core | P1 |
| `env-manager` | Managed local env create/rebuild/reset/clone/delete | db-postgres, pack-contract | P1–P2 |
| `desktop-shell` | Design-led desktop shell, Home, problem workspace and thin commands | contracts and service adapters | P1–P3 |
| `app-storage` | SQLite submission, progress, connection metadata | none | P2 |
| `connections` | External PG/MSSQL provider configuration/probe/explorer | adapters, secrets | P3 |
| `db-sqlserver` | SQL Server metadata/runner, dialect support | judge-core, connections | P3–P4 |
| `pack-authoring` | AI agent generation docs and export/import feedback | pack-contract | P0–P4 |
| `release-runtime` | Managed DB runtime distribution/lifecycle | env-manager, desktop-shell | P4 |

## 3. User journeys
**J0 (home):** open app -> Home is the default route -> inspect local runtime/setup state and next available problem -> navigate to Practice, Packs, Environments, History, Progress, Connections or Explorer.

**J1 (learn):** open app -> select problem -> view statement/table schema/sample fixture -> type query -> Run on sample -> Submit -> result/verdict/diff -> next problem -> progress persisted.

**J2 (import AI content):** AI generates one `.sqljudge.zip` containing `pack.json`, references and fixtures -> app shows validation report -> tests references in disposable managed PG database -> atomically installs immutable pack version -> optional provision of named workspace -> problems available without app restart.

**J3 (workspaces):** create E-Commerce and HR pack environments -> inspect schema -> switch, reset only scoped owned env, clone env -> no cross-contamination or loss of submission history.

**J4 (external explorer):** connect user PostgreSQL or Microsoft SQL Server by host/port/database, authentication + encrypted connection -> test metadata/read-only -> browse metadata/sample SELECT with conservative limits -> no create/reset/drop on arbitrary external server.

**J5 (opt-in external practice; deferred):** advanced user designates a disposable, isolated namespace with scoped credentials, ownership proof and destructive confirmation; external SQL Server has its own runner/DDL semantics. Never auto-provision from plain connection profile.

## 4. Functional requirements
| ID | Priority | Description |
|---|---|---|
| REQ-001 | MUST | Preserve existing agmmnn/tauri-ui scaffold, features, Vite React TypeScript/Bun/shadcn preset b0. |
| REQ-002 | MUST | Problem browser search/filter by dialect, tags, difficulty; show schema/statement/sample. |
| REQ-003 | MUST | Monaco SQL editor with per-problem draft and Run vs Submit operations. |
| REQ-004 | MUST | Sample Run returns bounded typed rows, column metadata, execution status and duration. |
| REQ-005 | MUST | Judge reference and user SQL under equivalent clean fixture states across >= 3 tests/problem. |
| REQ-006 | MUST | Ordered/unordered multiset comparison including duplicate counts and NULL; column schema policy explicit. |
| REQ-007 | MUST | Typed verdicts incl. Accepted/WrongAnswer/SyntaxError/RuntimeError/Timeout/PolicyViolation/JudgeError/ResultLimit. |
| REQ-008 | MUST | Engine never reports Accepted from text similarity alone. |
| REQ-009 | MUST | Versioned problem pack with declarative manifest, hash, schema setup, test fixtures, reference query; validated import with detailed errors. |
| REQ-010 | MUST | AI agent authoring RULE and machine-verifiable schemas with example pack; no implicit optional fields. |
| REQ-011 | MUST | Managed PostgreSQL per-pack workspace create/inspect/rebuild/reset/clone/delete with ownership proofs. |
| REQ-012 | MUST | Per-submission ephemeral fixture isolation; no user SQL runs under management credentials. |
| REQ-013 | MUST | SQLite submission and result history survive app restart; progress computed from accepted submissions. |
| REQ-014 | MUST | External PostgreSQL and Microsoft SQL Server connection profiles and read-only browsing. |
| REQ-015 | MUST | External connections are never auto-reset or auto-judged; protected credentials, TLS verification and clear errors. |
| REQ-016 | MUST | App works without cloud; if managed PG runtime is not bundled in a given phase/platform, startup shows install requirement. |
| REQ-017 | MUST | Manage cancel, timeout, cleanup, crash recovery and connection failures with no false WrongAnswer. |
| REQ-018 | MUST | Structured logs devoid of secrets, minimal telemetry (off by default); friendly diagnostics. |
| REQ-019 | MUST | Implement the application shell and every screen from the approved visual contract in `docs/design/SQL Judge — UI redesign.html` and its matching artboard. Home is the default route; shell navigation groups are Learn, Library and Data; desktop sidebar is 248px and header is 56px. Preserve keyboard access, dark/light modes through the existing provider, responsive behavior and all safety states. |
| REQ-020 | SHOULD | Import/export backup of packs, settings (no secrets) and history; collision/version semantics. |
| REQ-021 | SHOULD | Distinct dialect support and version metadata, `EXPLAIN` for local read-only practice. |
| REQ-022 | MUST | External PostgreSQL writable sandbox is available only through explicit advanced opt-in, saved-profile capability verification, a generated app-owned schema, durable target binding, ownership-marker verification, displayed impact summary, and explicit confirmation. It must never adopt or alter an existing external schema or database. |
| REQ-023 | LATER | Bundled managed PostgreSQL per OS release asset with startup/shutdown/update mechanics. |
| REQ-024 | LATER | SQL Server judge for disposable managed SQL Server test environments. |
| REQ-025 | MUST | UI implementation must reproduce the matching artboard's declared tokens, layout, typography, spacing, radii, borders, shadows, interaction states and empty/loading/error states through the existing React component system. Exported HTML and vendor runtime are reference material only. |

## 5. Nonfunctional requirements / target budgets (validate on target machines)
| ID | Objective | Verification |
|---|---|---|
| NFR-001 | Offline app launch and content browsing without Internet | Block outbound network and launch app |
| NFR-002 | Default per-query statement deadline 2s; wall-clock watchdog 5s; configurable per pack within admin cap | Timeout integration test |
| NFR-003 | Cap query fetch at 10,000 rows and <=16MiB result payload; oversized results cannot be marked Accepted | Boundary integration tests |
| NFR-004 | Start with concurrency 1 local judge; never allow overlapping reset and judge of same owned env | Race-condition test |
| NFR-005 | No secrets stored in plaintext in app DB/logs, external TLS validated | Static scan + integration test |
| NFR-006 | User can cancel running judge and environment is eventually cleaned | Kill/cancel integration test |
| NFR-007 | Every successful import is atomic and traceable by pack ID/version/content hash | Import rollback tests |
| NFR-008 | Frontend passes typecheck; Rust fmt/clippy/tests pass; UI manual keyboard review | CI & smoke testing |
| NFR-009 | User DB never modified by read-only explorer or managed reset | External decoy DB test |
| NFR-010 | Release builds documented and verified per selected platform; no assumed bundled PG | Matrix evidence |
| NFR-011 | Screen fidelity is validated at each artboard viewport and supported responsive widths; no completion claim without rendered comparison evidence | Visual review artifact per route |

## 6. Explicitly excluded from initial MVP
Full SQL syntax equivalence proof, cloud multi-user judge, tamper-proof hidden tests, arbitrary writable production databases, DB migration tool for user data, MySQL driver, DML/DDL problems, auto-generated AI hints requiring an Internet API, sync accounts, PL/pgSQL security proofs, full DB sidecar on day 1.

## 7. Assumptions / decisions needing proof
- `sql-judge` generated scaffold exists locally, but is not provided with these docs; agents **must inspect it** before changes. `src/` + `src-tauri/` are likely, not guaranteed.
- Bun governs generated JavaScript toolchain. Do not create pnpm workspace or replace existing root package layout unprompted.
- SQL Server means **Microsoft SQL Server (T-SQL)**, not generic SQL host only.
- Managed DB server may be preinstalled/local container in development; bundled-runtime distribution is a separate high-risk cross-platform deliverable.
- AI-created package references are *candidate oracles*: import validation must exercise adversarial fixtures and compare independent expectations when available. No finite tests guarantee semantic equivalence.
