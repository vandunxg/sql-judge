# Practice Workspace UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the crowded all-pack Practice layout with a pack-scoped, paginated, editor-first workspace that has stable judge feedback and offline schema-aware Monaco completion.

**Architecture:** Keep installed-pack data and native Run/Submit IPC unchanged. Extract pure pack selection, pagination, delayed-stop, and completion-data functions so they have fast Bun unit coverage; compose those functions in `LearningWorkspace` using the existing catalog and judge-state safeguards. Reuse the existing shadcn `Tabs` and `Sheet` components instead of adding a UI dependency.

**Tech Stack:** React 19, TypeScript, Tailwind/shadcn, `@monaco-editor/react`/Monaco, Bun test, existing Tauri judge API.

---

## File Structure

- Create: `src/features/learning/practice-workspace-state.ts` — pack-scoped filtering, fixed pagination, selection/page synchronization, and delayed Stop visibility.
- Create: `src/features/learning/practice-workspace-state.test.js` — regression coverage for pure workspace state.
- Create: `src/features/learning/sql-completions.ts` — builds dialect keyword/snippet/table/column completion records from trusted installed-pack schema metadata.
- Create: `src/features/learning/sql-completions.test.js` — pure completion-data coverage; never instantiates Monaco or a database.
- Modify: `src/features/learning/learning-workspace.tsx` — compose focused desktop/laptop layouts, existing judge lifecycle, tabs, drawers, and Monaco completion registration.
- Modify: `src/features/learning/catalog.ts` — add one pack-scoped catalog helper only if extraction avoids duplicating pack identity mapping in the workspace.
- Modify: `tasks/todo.md` — record fresh T10/T16 desktop/UI evidence only after all acceptance checks succeed.

## Task 1: Pack-Scoped Paging State

**Files:**
- Create: `src/features/learning/practice-workspace-state.ts`
- Create: `src/features/learning/practice-workspace-state.test.js`

- [x] **Step 1: Write failing tests for pack scoping and page boundaries**

```js
import { describe, expect, test } from "bun:test"

import {
  PROBLEM_PAGE_SIZE,
  pageForProblem,
  paginateProblems,
  problemsForPack,
} from "./practice-workspace-state"

const problems = Array.from({ length: 12 }, (_, index) => ({
  id: `pack-a:${index + 1}`,
  packId: "pack-a",
  title: `Problem ${index + 1}`,
}))

describe("practice workspace state", () => {
  test("scopes the rail to the selected pack", () => {
    expect(
      problemsForPack([...problems, { id: "pack-b:1", packId: "pack-b", title: "Other" }], "pack-a")
    ).toHaveLength(12)
  })

  test("paginates five problems and clamps invalid pages", () => {
    expect(PROBLEM_PAGE_SIZE).toBe(5)
    expect(paginateProblems(problems, 0).items).toHaveLength(5)
    expect(paginateProblems(problems, 99)).toMatchObject({ page: 2, totalPages: 3 })
  })

  test("maps quick-jump selection to its containing page", () => {
    expect(pageForProblem(problems, "pack-a:11")).toBe(2)
    expect(pageForProblem(problems, "missing")).toBe(0)
  })
})
```

- [x] **Step 2: Run the new test and verify RED**

Run: `bun test src/features/learning/practice-workspace-state.test.js`

Expected: import failure because `practice-workspace-state.ts` does not exist.

- [x] **Step 3: Implement the pure state module**

```ts
export const PROBLEM_PAGE_SIZE = 5

export function problemsForPack<T extends { packId: string }>(
  problems: T[],
  packId: string
) {
  return problems.filter((problem) => problem.packId === packId)
}

export function paginateProblems<T>(items: T[], requestedPage: number) {
  const totalPages = Math.max(1, Math.ceil(items.length / PROBLEM_PAGE_SIZE))
  const page = Math.min(Math.max(0, requestedPage), totalPages - 1)
  const start = page * PROBLEM_PAGE_SIZE
  return {
    items: items.slice(start, start + PROBLEM_PAGE_SIZE),
    page,
    totalPages,
    start,
    end: Math.min(start + PROBLEM_PAGE_SIZE, items.length),
  }
}

export function pageForProblem<T extends { id: string }>(
  items: T[],
  problemId: string
) {
  const index = items.findIndex((item) => item.id === problemId)
  return index < 0 ? 0 : Math.floor(index / PROBLEM_PAGE_SIZE)
}
```

- [x] **Step 4: Run the focused state test and verify GREEN**

Run: `bun test src/features/learning/practice-workspace-state.test.js`

Expected: 3 passing tests.

- [x] **Step 5: Commit the isolated state change**

```bash
git add src/features/learning/practice-workspace-state.ts src/features/learning/practice-workspace-state.test.js
git commit -m "feat: add pack-scoped practice pagination"
```

## Task 2: Offline Schema-Aware Completion Data

**Files:**
- Create: `src/features/learning/sql-completions.ts`
- Create: `src/features/learning/sql-completions.test.js`

- [x] **Step 1: Write failing completion-data tests**

```js
import { describe, expect, test } from "bun:test"

import { sqlCompletionItems } from "./sql-completions"

const schema = [
  { name: "tasks", columns: [{ name: "id", type: "integer", nullable: false }] },
]

describe("sqlCompletionItems", () => {
  test("offers PostgreSQL snippets and active-schema identifiers", () => {
    const labels = sqlCompletionItems("postgresql", schema).map((item) => item.label)
    expect(labels).toEqual(expect.arrayContaining(["SELECT", "WITH", "tasks", "id"]))
  })

  test("does not include schema identifiers from another problem", () => {
    expect(sqlCompletionItems("postgresql", []).map((item) => item.label)).not.toContain("tasks")
  })
})
```

- [x] **Step 2: Run the new test and verify RED**

Run: `bun test src/features/learning/sql-completions.test.js`

Expected: import failure because `sql-completions.ts` does not exist.

- [x] **Step 3: Implement serializable completion descriptors**

```ts
import type { CatalogTable, ProblemDialect } from "@/lib/judge-api"

export type SqlCompletionItem = {
  label: string
  detail: string
  insertText: string
  kind: "keyword" | "snippet" | "table" | "column"
}

const KEYWORDS = ["SELECT", "WITH", "JOIN", "GROUP BY", "ORDER BY"]

export function sqlCompletionItems(dialect: ProblemDialect, schema: CatalogTable[]) {
  const keywords = KEYWORDS.map((label) => ({
    label,
    detail: `${dialect} keyword`,
    insertText: label,
    kind: "keyword" as const,
  }))
  const tables = schema.flatMap((table) => [
    { label: table.name, detail: "table", insertText: table.name, kind: "table" as const },
    ...table.columns.map((column) => ({
      label: column.name,
      detail: `${table.name}.${column.name} · ${column.type}`,
      insertText: column.name,
      kind: "column" as const,
    })),
  ])
  return [...keywords, ...tables]
}
```

- [x] **Step 4: Run focused completion tests and verify GREEN**

Run: `bun test src/features/learning/sql-completions.test.js`

Expected: 2 passing tests.

- [x] **Step 5: Commit the isolated completion-data change**

```bash
git add src/features/learning/sql-completions.ts src/features/learning/sql-completions.test.js
git commit -m "feat: add offline SQL completion data"
```

## Task 3: Compose the Pack Workspace and Stable Judge Feedback

**Files:**
- Modify: `src/features/learning/learning-workspace.tsx:61-540`
- Modify: `src/features/learning/catalog.ts:24-40` only if a pack-scoped mapper removes duplication
- Test: `src/features/learning/practice-workspace-state.test.js`

- [x] **Step 1: Extend the state test with delayed Stop behavior**

```js
import { stopVisibleAfter } from "./practice-workspace-state"

test("shows Stop only after the running grace period", () => {
  expect(stopVisibleAfter(1_499)).toBe(false)
  expect(stopVisibleAfter(1_500)).toBe(true)
})
```

- [x] **Step 2: Run the state test and verify RED**

Run: `bun test src/features/learning/practice-workspace-state.test.js`

Expected: missing export `stopVisibleAfter`.

- [x] **Step 3: Add the pure grace-period helper**

```ts
export const STOP_GRACE_PERIOD_MS = 1_500

export function stopVisibleAfter(elapsedMs: number) {
  return elapsedMs >= STOP_GRACE_PERIOD_MS
}
```

- [x] **Step 4: Refactor `LearningWorkspace` around pack-local view state**

> Revised 2026-10-14: implemented against the binding Solve design instead of the persistent rail and context-above-editor layout below. See the revised spec Layout section. Pagination, delayed Stop, and completion rules are unchanged.

Implement these exact rules:

```ts
const [selectedPackId, setSelectedPackId] = useState("")
const [page, setPage] = useState(0)
const [showStop, setShowStop] = useState(false)

const packProblems = problemsForPack(learningProblems, selectedPackId)
const matchingProblems = filterProblems(packProblems, query, "all", difficulty)
const pagedProblems = paginateProblems(matchingProblems, page)

function selectPack(packId: string) {
  setSelectedPackId(packId)
  setQuery("")
  setDifficulty("all")
  setPage(0)
  selectProblem(problemsForPack(learningProblems, packId).at(0)?.id ?? "")
}

function jumpToProblem(problemId: string) {
  setPage(pageForProblem(matchingProblems, problemId))
  selectProblem(problemId)
}
```

Use existing `Tabs` for Statement, Schema, Sample data, and Hints. Keep result markup mounted in a dedicated section below Monaco, with an idle `Ready to run` placeholder. At `min-width: 1280px`, render the paginated problem rail; at `1024px–1279px`, render it in existing `Sheet`; below `1024px`, use a `Sheet` for problems and context. Do not retain the old persistent third context column.

When `run` starts, keep the clicked action in place with `LoaderCircle` and an explicit running label. Start a `window.setTimeout` for `STOP_GRACE_PERIOD_MS`; reveal an inline `Stop` button only after it fires. Clear the timer and hide Stop in every current-request completion, error, cancellation, unmount, and problem change branch. Do not change `runJudge`, `cancelJudge`, request IDs, or native commands.

In `Editor` `onMount`, call `monaco.languages.registerCompletionItemProvider(language, provider)` using `sqlCompletionItems(selectedProblem.dialect, selectedProblem.schema)`. Convert descriptor kinds to Monaco `CompletionItemKind`, set `range: model.getWordUntilPosition(position)`, and return the registration's disposable from a `useEffect` cleanup so a prior problem's schema is never suggested.

- [x] **Step 5: Verify the focused UI state tests**

Run: `bun test src/features/learning/practice-workspace-state.test.js src/features/learning/sql-completions.test.js src/features/learning/catalog.test.js src/features/learning/judge-state.test.js`

Expected: all tests pass.

- [x] **Step 6: Verify type safety and build**

Run: `bun run typecheck && bun run lint && bun run build`

Expected: exit 0; preserve any pre-existing Vite chunk-size warning without suppressing it.

- [ ] **Step 7: Desktop smoke test**

Run: `bun run tauri dev`

Verify at 1400px and 1024px widths:

```text
1. Import or select an installed pack, then select a problem through the quick-jump combobox.
2. Confirm only that pack's problems appear in the rail and pagination shows five per page.
3. Edit SQL and verify table/column/snippet completion is local and problem-scoped.
4. Run sample: action stays in place, displays progress, reveals Stop after 1.5 seconds, and returns to idle/result state.
5. Submit: Run is disabled while submitting; result status is rendered in the fixed result section.
6. At 1024px, open/close problem and context sheets using keyboard, then return focus to their triggers.
```

- [ ] **Step 8: Commit the Practice vertical slice**

```bash
git add src/features/learning/learning-workspace.tsx src/features/learning/catalog.ts src/features/learning/practice-workspace-state.ts src/features/learning/practice-workspace-state.test.js src/features/learning/sql-completions.ts src/features/learning/sql-completions.test.js tasks/todo.md
git commit -m "feat: redesign practice workspace"
```

## Task 4: Record Evidence and Protect Existing Boundaries

**Files:**
- Modify: `tasks/todo.md:23,36`
- Modify: `docs/operations/MVP-DESKTOP-SMOKE-2026-10-13.md`

- [ ] **Step 1: Record actual commands and desktop outcomes**

Add only commands actually run and their observed outcomes. Record pack-scoped pagination, completion scope, stable Run/Submit state, delayed Stop, and both responsive breakpoints. Do not mark T10/T16 complete if any manual check is absent.

- [ ] **Step 2: Run affected suites one final time**

Run: `bun test && bun run typecheck && bun run lint && bun run build`

Expected: all Bun tests pass and static/build commands exit 0.

- [ ] **Step 3: Commit evidence separately**

```bash
git add tasks/todo.md docs/operations/MVP-DESKTOP-SMOKE-2026-10-13.md
git commit -m "docs: record practice workspace smoke evidence"
```

## Spec Coverage Review

- Pack-first workspace, pagination, quick jump, and fixed page size: Task 1 and Task 3.
- Editor-first responsive layout, tabs/drawers, and fixed result region: Task 3.
- Stable Run/Submit state and delayed Stop: Task 3.
- Offline schema-aware completion: Task 2 and Task 3.
- Accessibility and keyboard smoke: Task 3 desktop smoke.
- Secondary-page shared visual system: documented in the approved design; explicitly deferred to a separate implementation plan to keep this vertical slice testable.

## Self-Review

- No placeholder implementation steps remain.
- State helpers defined in Task 1 are used consistently in Task 3.
- `STOP_GRACE_PERIOD_MS` and `stopVisibleAfter` are defined before their usage.
- All new behavior has a RED command, GREEN command, and final verification command.
