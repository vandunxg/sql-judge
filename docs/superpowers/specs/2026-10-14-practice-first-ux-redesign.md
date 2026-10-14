# Practice-First UX Redesign

## Status

Approved design direction: Practice-first with a shared page system. This
specification preserves the existing Tauri UI scaffold, shadcn components, and
native safety boundaries.

## Goals

- Make solving a SQL exercise the dominant, uncluttered workflow.
- Scope problem discovery to one selected pack at a time.
- Preserve action-bar geometry while a judge request runs.
- Provide practical, offline schema-aware completion in Monaco.
- Make desktop and laptop layouts usable without squeezing three persistent
  columns into the viewport.
- Apply consistent hierarchy and async feedback to every operational page.

## Non-Goals

- No new database permissions, Tauri commands, or pack format fields.
- No remote AI completion, telemetry, or transmission of editor SQL.
- No replacement of shadcn, theme provider, sidebar framework, debug panel, or
  generated release infrastructure.
- No full dashboard/IA rewrite in this increment.

## Shared Page System

Every feature screen uses the same primitives and state rules:

- A compact page header: eyebrow, title, one-sentence purpose, and only
  page-level actions.
- A toolbar for filters, search, and scoped selection rather than filters
  embedded in long content lists.
- Loading uses skeleton/inline status in the affected region. Empty states
  state the next safe action. Recoverable errors remain local and expose Retry.
- Mutating actions retain their placement and width. Their own label changes to
  a spinner and progress text; unrelated actions are disabled with an
  explanation rather than removed.
- Destructive actions remain explicit and use the existing ownership/impact
  review flow. UI changes do not weaken native authorization.
- Desktop uses a max-width content surface. Laptop collapses secondary content
  into tabs/drawers. Narrow widths use a modal sheet, not compressed columns.

## Practice Workspace

### Information Architecture

Revised 2026-10-14: the binding design export
`docs/design/Solve — submit verdict-html/Solve.dc.html` supersedes the earlier
persistent-rail layout. The separate table-style Problems page
(`docs/design/Problems-html/`) is a later task.

- The current pack is the pack of the selected problem. The initial selection
  is the first problem of the first installed pack.
- A `Problem list` toolbar button opens a left sheet with a pack selector,
  search, a difficulty filter, and a fixed page of five problems from that
  pack with Previous/Next page controls and a `Showing x–y of n` range. The
  sheet opens on the page containing the selected problem.
- Previous/Next problem buttons beside `Problem list` step through the current
  pack in catalog order and disable at the boundaries.
- Changing pack selects that pack's first problem, resets the sheet filters,
  retains per-problem drafts by immutable pack/problem key, and clears any
  previous result.

### Layout

- A toolbar row holds `Problem list` and Previous/Next on the left, and
  `Run` (secondary, `Ctrl ↵`) and `Submit` (primary) in the centre.
- Two panes follow, as in the design: the problem pane (Description and
  Schema tabs) and the work column (editor pane over a results pane with Run
  result and Submission tabs). Panes use the design's 10px radius, 42px tab
  header, 30px tabs, and 8px gaps.
- The panes are flex items (basis 440px and 560px) that wrap, so below about
  1024px of content width the problem pane stacks above the editor without
  horizontal scrolling.
- The results pane has a fixed minimum height below the editor. A result
  replaces the idle copy inside it, so it never shifts the action bar or
  collapses the editor.
- Not built because no data exists for them, and adding them would need new
  IPC: the Submissions tab, per-fixture tabs, expected output, the "What
  differs" explanation, example input tables, editor formatting, and maximize.

### Run, Submit, and Stop

- Idle: `Run sample` is secondary and `Submit` is primary. Tooltips explain
  the difference and disabled state.
- On start: the triggered button keeps its place, label, and width and swaps
  its icon for a spinner; the other primary action is disabled.
- The results pane switches to the matching tab and shows `Running sample...`
  or `Submitting...` immediately. After 1.5 seconds, a small `Stop` action
  appears to the right of the action group. It invokes
  the existing request-ID cancellation path.
- Editor input and problem/pack switching are disabled during an active run to
  avoid ambiguous draft/request state. Context tabs remain readable.
- Completion replaces inline progress with typed result banner/table. Cancel,
  policy, syntax, timeout, and infrastructure errors use distinct copy and do
  not expose SQL, credentials, or driver internals.

### Monaco Completion

- Register an offline completion provider for SQL before editor mount, scoped
  to the active problem and its PostgreSQL or SQL Server dialect.
- Offer dialect keywords plus snippets for `SELECT`, `WITH`, `JOIN`,
  `GROUP BY`, and `ORDER BY`.
- Offer table names and columns declared by the active problem schema. Column
  suggestions include table detail and insert only the identifier.
- Refresh/dispose schema-specific completion data on problem change. Never
  parse, execute, or transmit the draft to derive completions in this phase.

## Secondary Pages

### Packs

- Split the screen into a compact import card and installed-pack catalog.
- Installed packs become concise rows/cards with problem count, dialect,
  version, and `Open in Practice`; detailed problem previews move to an
  expandable detail or pack detail route instead of extending the page.
- Import keeps current native validation, but presents stages in a stable
  status area: source validation, reference validation, publication, complete.

### Environments

- Runtime health is a compact status strip, not a competing primary card.
- Create environment becomes a focused toolbar (pack, namespace, Create).
- Managed environments use a responsive table/list with state, pack, and
  concise row actions. Reset/Delete keep the existing impact review.

### Connections

- Saved profiles become a selectable list with clear engine and TLS chips.
- The details form is a focused edit surface; explorer actions are grouped in
  a separate, gated panel that becomes available only after connection test.
- No secret, URL, or database error detail is newly persisted or revealed.

### History and Progress

- History starts with filterable, compact submission rows. Selecting a row
  opens verdict/test detail in a side sheet or detail region.
- Progress leads with a small summary and pack-scoped progress; detailed rows
  are collapsible. Backup actions remain visible but secondary.

## Accessibility and Keyboard Rules

- Comboboxes, pagination, tabs, drawers, and result regions have accessible
  names, visible focus, Escape dismissal, and correct disabled semantics.
- Run/Submit can use documented shortcuts only when Monaco focus and
  accessibility behavior permit them; no shortcut overrides typing.
- Status uses `aria-live=polite`; errors use `role=alert`; verdict never relies
  only on color.
- The same workflow works from 1024px desktop width through large displays.

## Data and Safety

- UI state derives from existing installed-pack and problem metadata only.
- No page sends new data over IPC or relaxes backend pack, runner, ownership,
  cancellation, credential, or result-limit policy.
- Existing immutable pack identity remains on Run/Submit requests.

## Test Plan

- Unit test pack-scoped problem selection, pagination boundaries, quick jump,
  and selected-problem reset behavior.
- Unit test async action state: fixed labels/placement, delayed Stop visibility,
  disabled competing actions, and completion/cancellation reset.
- Unit test completion-data construction for dialect keywords, schema tables,
  and columns without editor/DB mocking.
- Component tests for keyboard combobox, tabs, pagination, and responsive
  drawer behavior if the existing test harness supports them.
- Native regression tests remain required for cancellation and exact pack
  identity. Desktop smoke verifies import, pack selection, Run, Submit, Stop,
  and a narrow-layout breakpoint.

## Traceability

- Requirements: `REQ-003`, `REQ-004`, `REQ-008`, `REQ-013`, `REQ-015`,
  `REQ-017`, `REQ-019`.
- Acceptance: `AC-002`, `AC-003`, `AC-004`, `AC-007`, `AC-008`, `AC-013`,
  `AC-015`, `AC-017`, `AC-019`.
- Security: `SEC-001`, `SEC-003`, `SEC-007`, `SEC-010`.
