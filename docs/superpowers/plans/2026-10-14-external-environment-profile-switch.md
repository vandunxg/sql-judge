# External Environment Profile Switching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let the Environments page switch between saved external PostgreSQL profiles, reload only that profile's active environments, and retain local runtime as the default fallback when no PostgreSQL profile exists.

**Architecture:** Keep profile-to-environment ownership in the native SQLite binding already created for external sandboxes. Extend the existing environment-list command with an optional profile filter, then let the React controller select local or a PostgreSQL profile and reload through the same data path. The local runtime remains available as a source option and its existing start behavior is unchanged.

**Tech Stack:** Tauri 2/Rust, React 19 + TypeScript, existing shadcn/Base UI Select, Bun test.

**Spec:** `docs/requirements/PRD.md` (REQ-014, REQ-015, REQ-022); `docs/requirements/ACCEPTANCE.md` (AC-014, AC-015, AC-021).

## Global Constraints

- External connections are never auto-reset or auto-judged; keep destructive controls restricted to app-owned external sandboxes.
- Never expose credentials in frontend state, SQLite metadata, logs, or UI.
- Local runtime remains the fallback source; switching source must not mutate environments or submission history.
- Preserve existing Tauri/shadcn/Bun structure and design-export layout.

## Review Focus

- No PostgreSQL profiles: local source is selected and existing local runtime controls remain usable.
- SQL Server profiles only: they are excluded from the environment-source selector.
- Selected profile: only external sandboxes bound to that profile are returned; managed local and other profiles are excluded.
- Refresh/switch failure: the page shows an actionable error without silently showing stale data as current.
- Profile switch while an environment action is busy: selection is disabled until the operation completes.

---

### Task 1: Filter native environment listing by profile

**Files:**
- Modify: `src-tauri/src/lib.rs` (`list_managed_environments` and command registration)
- Modify: `src/lib/judge-api.ts` (`listManagedEnvironments` wrapper)
- Test: `src-tauri/src/lib.rs` existing native test module

**Interfaces:**
- Consumes: optional `profileId` from the frontend.
- Produces: `listManagedEnvironments(profileId?: string)` returning only `external_sandbox` records whose durable external binding has the requested profile ID; omitted/empty profile returns managed local records.

- [ ] **Step 1: Write failing native tests** for profile filtering, including a record bound to profile A not appearing for profile B, and local records appearing in the local/default view.
- [ ] **Step 2: Run the focused native tests** with `cargo test --manifest-path src-tauri/Cargo.toml <filter>` and observe the expected failure before implementation.
- [ ] **Step 3: Implement the optional command argument and filter** using the existing `EnvironmentStore::get_external_sandbox_target` binding; do not add a second source of truth.
- [ ] **Step 4: Update the TypeScript invoke wrapper** to pass `{ profileId }` only when a profile source is selected, preserving current no-argument callers.
- [ ] **Step 5: Run focused native tests and frontend typecheck** and confirm GREEN.

### Task 2: Add source selection state and reload behavior

**Files:**
- Modify: `src/features/learning/environments-data.ts`
- Modify: `src/features/learning/environments-controller.ts`
- Modify: `src/features/learning/environments-model.ts`
- Test: `src/features/learning/environments-model.test.js` or a focused new state test beside it

**Interfaces:**
- Consumes: PostgreSQL-only `ConnectionProfileSummary[]` and `listManagedEnvironments(profileId?)`.
- Produces: controller state with `selectedProfileId: string | null`, `postgresProfiles`, `selectProfile(profileId: string | null)`, and refreshes environments whenever the source changes.

- [ ] **Step 1: Write failing tests** for default-source selection (local when there are no PostgreSQL profiles, first PostgreSQL profile when profiles exist), SQL Server exclusion, and source-switch selection values.
- [ ] **Step 2: Run the focused Bun tests** with `bun test src/features/learning/environments-model.test.js` and observe the expected failure.
- [ ] **Step 3: Implement the smallest source-selection helpers/state**; keep selected profile in page state only and clear environment rows before/while loading a new source so stale rows are not presented as current.
- [ ] **Step 4: Run focused Bun tests, then `bunx tsc -b`** and confirm GREEN.

### Task 3: Render the profile switch in the existing runtime card

**Files:**
- Modify: `src/features/learning/environments-runtime-card.tsx`
- Modify: `src/features/learning/environments-screen.tsx`
- Modify: `src/features/learning/environments-controller.ts` if final prop wiring requires it

**Interfaces:**
- Consumes: selected source, PostgreSQL profiles, loading/busy state, and local runtime status.
- Produces: accessible Select with “Local runtime” plus PostgreSQL profile options; switching triggers the controller reload; local Start runtime remains available when local is selected and unavailable.

- [ ] **Step 1: Add a UI/state test or assertion-level test** covering the profile option labels, SQL Server absence, and disabled selection while busy.
- [ ] **Step 2: Run the focused UI/state test** and observe the expected failure.
- [ ] **Step 3: Implement the Select using existing `Select`, `SelectTrigger`, `SelectContent`, `SelectItem`, and preserve the design card/guarantees and keyboard labels.
- [ ] **Step 4: Run `bunx tsc -b`, `bunx eslint .`, `bun test`, and `bun run build`.
- [ ] **Step 5: Manually verify the Environments artboard flow** at desktop and responsive width: no profiles → local fallback; profiles → switch profile → loading → filtered empty/list state.

## Verification and Handoff

- Update `tasks/todo.md` only after RED/GREEN evidence is available; link this change to REQ-014/015/022 and AC-014/015/021.
- Run `cargo fmt --all -- --check` and the affected native tests.
- Report the remaining limitation that profile selection is session/page state unless persistence is separately requested.
