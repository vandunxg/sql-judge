import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEventHandler,
  type PointerEventHandler,
  type ReactNode,
} from "react"
import Editor, { type Monaco } from "@monaco-editor/react"
import {
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  CircleX,
  Code,
  FileText,
  History,
  LoaderCircle,
  Maximize2,
  Menu,
  Minimize2,
  Moon,
  Play,
  RotateCcw,
  Send,
  Sun,
  Table2,
  TriangleAlert,
  XCircle,
} from "lucide-react"

import { useResolvedTheme } from "@/hooks/use-resolved-theme"
import { cn } from "@/lib/utils"
import type { JudgeRunResult, SubmissionHistorySummary } from "@/lib/judge-api"
import type { LearningProblem, ProblemDifficulty } from "./catalog"
import { topicLabel } from "./problems-list-state"
import { catalogProblemKey } from "./catalog"
import { problemProgress, relativeTime, submittedAtMs, verdictShortLabel } from "./problem-progress"
import {
  fixtureName,
  fixtureSummary,
  initialFixture,
  limitLabel,
  orderedColumns,
  resultSummary,
  rowOrderNote,
  verdictPresentation,
  type VerdictTone,
} from "./solve-state"
import { resizeResultSplit, resizeSplit } from "./solve-layout"
import { statementBlocks } from "./statement-format"
import type { sqlCompletionItems } from "./sql-completions"

const MAX_RENDERED_ROWS = 100
const MAX_RENDERED_CELL_LENGTH = 512
const MIN_PROBLEM_WIDTH = 25
const MAX_PROBLEM_WIDTH = 60
const MIN_RESULT_HEIGHT = 25
const MAX_RESULT_HEIGHT = 60

const difficultyLabels: Record<ProblemDifficulty, string> = {
  easy: "Easy",
  medium: "Medium",
  hard: "Hard",
}

const difficultyBadges: Record<ProblemDifficulty, string> = {
  easy: "bg-[var(--ok-bg)] text-ok",
  medium: "bg-[var(--warn-bg)] text-warn",
  hard: "bg-[var(--err-bg)] text-err",
}

const dialectLabels = { postgresql: "PostgreSQL", sqlserver: "SQL Server" }

const toneText: Record<VerdictTone, string> = {
  ok: "text-ok",
  err: "text-err",
  warn: "text-warn",
  muted: "text-muted-foreground",
}

const focusRing =
  "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
const baseButton = cn(
  "inline-flex items-center justify-center gap-2 rounded-lg border border-transparent text-sm leading-none font-medium whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-45",
  focusRing
)
const ghostSm = cn(baseButton, "h-[30px] gap-1.5 px-2.5 text-[13px] hover:bg-hover")
const ghostIcon = cn(baseButton, "size-[30px] hover:bg-hover")
const outlineMd = cn(
  baseButton,
  "h-9 border-[var(--solve-border)] bg-card px-3.5 shadow-xs-design hover:bg-hover"
)
const primaryMd = cn(baseButton, "h-9 bg-primary px-3.5 text-primary-foreground hover:opacity-88")
const pane =
  "flex min-h-0 min-w-0 flex-col overflow-hidden rounded-[10px] border border-[var(--solve-border)] bg-card"
const paneHeader =
  "flex h-[42px] shrink-0 items-center gap-0.5 border-b border-[var(--solve-border)] px-1.5"
const miniTable =
  "min-w-[220px] border-separate border-spacing-0 overflow-hidden rounded-lg border border-[var(--solve-border)] text-[13px]"
const miniTh =
  "border-b border-[var(--solve-border)] bg-muted px-3 py-[7px] text-left font-medium whitespace-nowrap"
const miniTd =
  "border-b border-[var(--solve-border)] px-3 py-[7px] font-mono whitespace-nowrap [tr:last-child_&]:border-b-0"

type CompletionItems = ReturnType<typeof sqlCompletionItems>
type ProblemTab = "description" | "schema" | "submissions"
type ResultTab = "run" | "submit"

export type SolveViewProps = {
  problem: LearningProblem
  packTitle: string
  environment?: { namespace: string; ready: boolean }
  history: SubmissionHistorySummary[]
  position: number
  total: number
  previousHref?: string
  nextHref?: string
  loading: boolean
  error: string | null
  onRetry: () => void
  draft: string
  onDraftChange: (value: string | undefined) => void
  onResetDraft: () => void
  completionItems: CompletionItems
  canJudge: boolean
  busy: boolean
  activeMode: ResultTab | null
  canStop: boolean
  result: JudgeRunResult | null
  resultMode: ResultTab | null
  onRun: (mode: ResultTab) => void
  onStop: () => void
}

function defineSolveThemes(monaco: Monaco) {
  const rules = (dark: boolean) => [
    { token: "keyword", foreground: dark ? "c4b5fd" : "7c3aed" },
    { token: "string", foreground: dark ? "86efac" : "047857" },
    { token: "number", foreground: dark ? "fcd34d" : "b45309" },
    { token: "comment", foreground: dark ? "7c7c86" : "8a8a93" },
    { token: "predefined.sql", foreground: dark ? "93c5fd" : "2563eb" },
    { token: "operator.sql", foreground: dark ? "c4b5fd" : "7c3aed" },
    { token: "delimiter", foreground: dark ? "f4f4f5" : "0a0a0c" },
    { token: "delimiter.parenthesis.sql", foreground: dark ? "f4f4f5" : "0a0a0c" },
  ]
  monaco.editor.defineTheme("sj-light", {
    base: "vs",
    inherit: true,
    rules: rules(false),
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#0a0a0c",
      "editor.lineHighlightBackground": "#f6f6f8",
      "editor.lineHighlightBorder": "#00000000",
      "editorLineNumber.foreground": "#a1a1aa",
      "editorLineNumber.activeForeground": "#0a0a0c",
      "editorCursor.foreground": "#0a0a0c",
      "editorBracketHighlight.foreground1": "#0a0a0c",
      "editorBracketHighlight.foreground2": "#0a0a0c",
      "editorBracketHighlight.foreground3": "#0a0a0c",
    },
  })
  monaco.editor.defineTheme("sj-dark", {
    base: "vs-dark",
    inherit: true,
    rules: rules(true),
    colors: {
      "editor.background": "#111114",
      "editor.foreground": "#f4f4f5",
      "editor.lineHighlightBackground": "#18181c",
      "editor.lineHighlightBorder": "#00000000",
      "editorLineNumber.foreground": "#55555e",
      "editorLineNumber.activeForeground": "#f4f4f5",
      "editorCursor.foreground": "#f4f4f5",
      "editorBracketHighlight.foreground1": "#f4f4f5",
      "editorBracketHighlight.foreground2": "#f4f4f5",
      "editorBracketHighlight.foreground3": "#f4f4f5",
    },
  })
}

export function SolveView(props: SolveViewProps) {
  const { problem, busy, activeMode, result, resultMode } = props
  const [problemTab, setProblemTab] = useState<ProblemTab>("description")
  const [resultTab, setResultTab] = useState<ResultTab>("run")
  const [maximized, setMaximized] = useState(false)
  const [problemWidth, setProblemWidth] = useState(42)
  const [resultHeight, setResultHeight] = useState(38)
  const [now] = useState(() => Date.now())
  const [cursor, setCursor] = useState({ line: 1, column: 1 })
  const { dark, toggle } = useResolvedTheme()
  const completionItemsRef = useRef(props.completionItems)
  const runRef = useRef(props.onRun)
  const canJudgeRef = useRef(false)
  const completionProviderRef = useRef<{ dispose(): void } | null>(null)

  const judgeDisabled = busy || props.loading || props.error !== null || !props.canJudge

  useEffect(() => {
    completionItemsRef.current = props.completionItems
    runRef.current = props.onRun
    canJudgeRef.current = !judgeDisabled
  })

  useEffect(() => () => completionProviderRef.current?.dispose(), [])

  const [followedMode, setFollowedMode] = useState(activeMode)
  if (activeMode !== followedMode) {
    setFollowedMode(activeMode)
    if (activeMode) {
      setResultTab(activeMode)
    }
  }

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if (!(event.ctrlKey || event.metaKey) || event.key !== "Enter") {
        return
      }
      event.preventDefault()
      if (canJudgeRef.current) {
        runRef.current(event.shiftKey ? "submit" : "run")
      }
    }
    window.addEventListener("keydown", handleShortcut)
    return () => window.removeEventListener("keydown", handleShortcut)
  }, [])

  const submissions = props.history
    .filter(
      (entry) =>
        entry.problemId !== null &&
        catalogProblemKey({ packId: entry.packId, version: entry.packVersion, digest: entry.packHash }, entry.problemId) === problem.id
    )
    .map((entry) => ({ ...entry, at: submittedAtMs(entry.createdAt) ?? 0 }))
    .sort((left, right) => right.at - left.at)
  const status = problemProgress(submissions, (packId, version, digest, problemId) =>
    catalogProblemKey({ packId, version, digest }, problemId)
  ).get(problem.id)?.status
  const outputColumns = orderedColumns(
    problem.sampleRows,
    problem.outputColumns.map((column) => column.name)
  )
  const sampleTables = problem.sampleTables.map((table) => ({
    ...table,
    columns: orderedColumns(
      table.rows,
      problem.schema.find((schemaTable) => schemaTable.name === table.name)?.columns.map((column) => column.name) ?? []
    ),
  }))

  function resizeProblemPanel(event: Parameters<PointerEventHandler<HTMLDivElement>>[0]) {
    const container = event.currentTarget.parentElement?.getBoundingClientRect()
    if (!container) {
      return
    }
    const startX = event.clientX
    const startWidth = problemWidth
    const onMove = (moveEvent: PointerEvent) => {
      setProblemWidth(
        resizeSplit(
          startWidth,
          ((moveEvent.clientX - startX) / container.width) * 100,
          MIN_PROBLEM_WIDTH,
          MAX_PROBLEM_WIDTH
        )
      )
    }
    const onUp = () => {
      window.removeEventListener("pointermove", onMove)
      window.removeEventListener("pointerup", onUp)
    }
    window.addEventListener("pointermove", onMove)
    window.addEventListener("pointerup", onUp)
  }

  function resizeResultPanel(event: Parameters<PointerEventHandler<HTMLDivElement>>[0]) {
    const container = event.currentTarget.parentElement?.getBoundingClientRect()
    if (!container) {
      return
    }
    const startY = event.clientY
    const startHeight = resultHeight
    const onMove = (moveEvent: PointerEvent) => {
      setResultHeight(
        resizeResultSplit(
          startHeight,
          ((moveEvent.clientY - startY) / container.height) * 100,
          MIN_RESULT_HEIGHT,
          MAX_RESULT_HEIGHT
        )
      )
    }
    const onUp = () => {
      window.removeEventListener("pointermove", onMove)
      window.removeEventListener("pointerup", onUp)
    }
    window.addEventListener("pointermove", onMove)
    window.addEventListener("pointerup", onUp)
  }

  function keyboardResizeProblem(direction: -1 | 1) {
    setProblemWidth((current) => resizeSplit(current, direction * 2, MIN_PROBLEM_WIDTH, MAX_PROBLEM_WIDTH))
  }

  function keyboardResizeResult(direction: -1 | 1) {
    setResultHeight((current) => resizeResultSplit(current, direction * 2, MIN_RESULT_HEIGHT, MAX_RESULT_HEIGHT))
  }

  return (
    <div className="solve-surface flex min-h-dvh flex-col bg-[var(--solve-bg)] text-sm leading-normal text-foreground">
      <header className="flex min-h-[52px] flex-wrap items-center gap-3 px-3.5 py-2">
        <div className="flex min-w-[260px] flex-1 items-center gap-1.5">
          <a
            href="#home"
            aria-label="SQL Judge home"
            className={cn(
              "mr-1.5 flex size-[30px] items-center justify-center rounded-lg bg-primary font-mono text-xs font-medium text-primary-foreground",
              focusRing
            )}
          >
            SJ
          </a>
          <a className={ghostSm} href="#problems">
            <Menu className="size-4" aria-hidden="true" />
            Problem list
          </a>
          <NavArrow href={props.previousHref} label="Previous problem">
            <ChevronLeft className="size-4" aria-hidden="true" />
          </NavArrow>
          <NavArrow href={props.nextHref} label="Next problem">
            <ChevronRight className="size-4" aria-hidden="true" />
          </NavArrow>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className={outlineMd}
            onClick={() => props.onRun("run")}
            disabled={judgeDisabled}
            title="Run on the public sample (Ctrl Enter)"
            aria-keyshortcuts="Control+Enter"
          >
            {busy && activeMode === "run" ? (
              <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
            ) : (
              <Play className="size-3.5" aria-hidden="true" />
            )}
            {busy && activeMode === "run" ? "Running…" : "Run"}
            <Kbd>Ctrl ↵</Kbd>
          </button>
          <button
            type="button"
            className={primaryMd}
            onClick={() => props.onRun("submit")}
            disabled={judgeDisabled}
            title="Submit for judging (Ctrl Shift Enter)"
            aria-keyshortcuts="Control+Shift+Enter"
          >
            {busy && activeMode === "submit" ? (
              <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
            ) : (
              <Send className="size-3.5" aria-hidden="true" />
            )}
            {busy && activeMode === "submit" ? "Submitting…" : "Submit"}
          </button>
          {props.canStop ? (
            <button type="button" className={cn(ghostSm, "text-err")} onClick={props.onStop}>
              <XCircle className="size-4" aria-hidden="true" />
              Stop
            </button>
          ) : null}
        </div>
        <div className="flex min-w-[200px] flex-1 items-center justify-end gap-1.5">
          {props.environment ? (
            <a
              href="#environments"
              className={cn("inline-flex items-center gap-1.5 rounded-md text-[13px] text-muted-foreground hover:text-foreground", focusRing)}
              title={props.environment.ready ? "Managed environment ready" : "Managed environment is not ready"}
            >
              <span
                className={cn("size-[7px] rounded-full", props.environment.ready ? "bg-[#16a34a]" : "bg-err")}
                aria-hidden="true"
              />
              {props.environment.namespace}
            </a>
          ) : null}
          <button type="button" className={ghostIcon} aria-label="Switch theme" onClick={toggle}>
            {dark ? <Sun className="size-4" aria-hidden="true" /> : <Moon className="size-4" aria-hidden="true" />}
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col gap-2 px-2.5 pb-2.5 lg:flex-row">
        {maximized ? null : (
          <section
            aria-label="Problem"
            className={cn(pane, "min-h-[600px] w-full flex-none lg:w-[var(--problem-width)]")}
            style={{ "--problem-width": `${problemWidth}%` } as CSSProperties}
          >
            <div className={paneHeader} role="tablist" aria-label="Problem sections">
              <PaneTab selected={problemTab === "description"} onSelect={() => setProblemTab("description")} icon={<FileText className="size-3.5" aria-hidden="true" />}>
                Description
              </PaneTab>
              <PaneTab selected={problemTab === "schema"} onSelect={() => setProblemTab("schema")} icon={<Table2 className="size-3.5" aria-hidden="true" />}>
                Schema
              </PaneTab>
              <PaneTab selected={problemTab === "submissions"} onSelect={() => setProblemTab("submissions")} icon={<History className="size-3.5" aria-hidden="true" />}>
                Submissions
                {submissions.length > 0 ? (
                  <span className="inline-flex h-[18px] items-center rounded-full bg-muted px-1.5 text-[11px] font-medium text-muted-foreground">
                    {submissions.length}
                  </span>
                ) : null}
              </PaneTab>
            </div>
            <div role="tabpanel" className="flex flex-1 flex-col gap-[22px] overflow-auto px-6 pt-[22px] pb-7">
              {props.loading ? (
                <p className="text-sm text-muted-foreground" role="status">Loading installed problem packs...</p>
              ) : props.error ? (
                <div className="flex flex-col items-start gap-3" role="alert">
                  <p className="text-sm text-err">{props.error}</p>
                  <button type="button" className={outlineMd} onClick={props.onRetry}>Retry</button>
                </div>
              ) : !problem.id ? (
                <div className="flex flex-col items-start gap-3">
                  <h1 className="text-[21px] leading-[1.3] font-semibold tracking-[-0.015em]">Problem not found</h1>
                  <p className="text-sm text-muted-foreground">This problem is not in any installed pack. Pick one from the problem list.</p>
                  <a className={outlineMd} href="#problems">Open problem list</a>
                </div>
              ) : problemTab === "description" ? (
                <>
                  <div className="flex flex-col gap-3">
                    <h1 className="min-w-0 line-clamp-2 text-[21px] leading-[1.3] font-semibold tracking-[-0.015em]" title={problem.title}>
                      {props.position}. {problem.title}
                    </h1>
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className={cn("inline-flex h-[22px] items-center rounded-full px-2 text-xs font-medium", difficultyBadges[problem.difficulty])}>
                        {difficultyLabels[problem.difficulty]}
                      </span>
                      {problem.topics.map((topic) => (
                        <span key={topic} className="inline-flex h-[22px] items-center rounded-full border border-[var(--solve-border)] bg-card px-2 text-xs font-medium">
                          {topicLabel(topic)}
                        </span>
                      ))}
                      {status ? (
                        <span className="ml-1.5 inline-flex items-center gap-[5px] text-[13px] text-muted-foreground">
                          {status === "solved" ? (
                            <CircleCheck className="size-3.5 text-ok" aria-hidden="true" />
                          ) : (
                            <svg viewBox="0 0 24 24" className="size-3.5 text-warn" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                              <circle cx="12" cy="12" r="9" />
                              <path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor" stroke="none" />
                            </svg>
                          )}
                          {status === "solved" ? "Solved" : "Attempted"}
                        </span>
                      ) : null}
                    </div>
                  </div>
                  <div className="flex max-w-[64ch] flex-col gap-3 text-[14.5px] leading-[1.65]">
                    {statementBlocks(problem.statement).map((paragraph, index) => (
                      <p key={index} className="whitespace-pre-line">
                        {paragraph.map((segment, segmentIndex) =>
                          segment.kind === "strong" ? (
                            <strong key={segmentIndex} className="font-semibold">{segment.text}</strong>
                          ) : segment.kind === "code" ? (
                            <code key={segmentIndex} className="rounded-[4px] bg-muted px-[5px] py-px font-mono text-[13px]">{segment.text}</code>
                          ) : (
                            segment.text
                          )
                        )}
                      </p>
                    ))}
                  </div>
                  {problem.outputColumns.length > 0 ? (
                    <div className="flex flex-col gap-2.5">
                      <h3 className="text-sm font-semibold">Return columns</h3>
                      <div className="flex flex-wrap gap-2">
                        {problem.outputColumns.map((column) => (
                          <span key={column.name} className="inline-flex h-[22px] items-center gap-[5px] rounded-full border border-[var(--solve-border)] bg-card px-2 font-mono text-xs">
                            {column.name} <span className="text-muted-foreground">{column.type}</span>
                          </span>
                        ))}
                      </div>
                      <p className="text-[13px] text-muted-foreground">{rowOrderNote(problem.rowOrder)}</p>
                    </div>
                  ) : null}
                  <div className="flex flex-col gap-2.5">
                    <h3 className="text-sm font-semibold">Example</h3>
                    {sampleTables.length > 0 ? (
                      <div className="flex flex-wrap items-start gap-3">
                        {sampleTables.map((table) => (
                          <div key={table.name} className="flex max-w-full flex-col gap-1.5">
                            <span className="font-mono text-xs text-muted-foreground">{table.name}</span>
                            <div className="max-w-full overflow-x-auto">
                              <table className={cn(miniTable, table.columns.length <= 2 && "min-w-[160px]")}>
                                <thead>
                                  <tr>{table.columns.map((column) => <th key={column} className={miniTh}>{column}</th>)}</tr>
                                </thead>
                                <tbody>
                                  {table.rows.map((row, index) => (
                                    <tr key={index}>
                                      {table.columns.map((column) => <ValueCell key={column} value={row[column]} />)}
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : null}
                    <div className="flex flex-col gap-1.5">
                      <span className="text-xs font-medium text-muted-foreground">Output</span>
                      {outputColumns.length > 0 ? (
                        <div className="max-w-full self-start overflow-x-auto">
                          <table className={miniTable}>
                            <thead>
                              <tr>{outputColumns.map((column) => <th key={column} className={miniTh}>{column}</th>)}</tr>
                            </thead>
                            <tbody>
                              {problem.sampleRows.map((row, index) => (
                                <tr key={index}>
                                  {outputColumns.map((column) => <ValueCell key={column} value={row[column]} />)}
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      ) : (
                        <p className="text-[13px] text-muted-foreground">Public sample output is unavailable for this problem.</p>
                      )}
                    </div>
                  </div>
                  <div className="mt-auto flex flex-wrap gap-x-4 gap-y-1.5 border-t border-[var(--solve-border)] pt-3.5 text-[12.5px] text-muted-foreground">
                    <span>{props.packTitle} {problem.packVersion}</span>
                    <span>{dialectLabels[problem.dialect]}</span>
                    {problem.fixtureCount > 0 ? (
                      <span>Judged on {problem.fixtureCount} fixture{problem.fixtureCount === 1 ? "" : "s"}</span>
                    ) : null}
                    {problem.timeLimitMs > 0 ? <span>{limitLabel(problem.timeLimitMs)}</span> : null}
                  </div>
                </>
              ) : problemTab === "schema" ? (
                problem.schema.length === 0 ? (
                  <p className="text-[13px] text-muted-foreground">Schema metadata is unavailable for this problem.</p>
                ) : (
                  problem.schema.map((table) => (
                    <div key={table.name} className="flex flex-col gap-1.5">
                      <span className="font-mono text-xs text-muted-foreground">{table.name}</span>
                      <table className={cn(miniTable, "self-start")}>
                        <thead>
                          <tr><th className={miniTh}>column</th><th className={miniTh}>type</th><th className={miniTh}>nullable</th></tr>
                        </thead>
                        <tbody>
                          {table.columns.map((column) => (
                            <tr key={column.name}>
                              <td className={miniTd}>{column.name}</td>
                              <td className={cn(miniTd, "text-muted-foreground")}>{column.type}</td>
                              <td className={cn(miniTd, "text-muted-foreground")}>{column.nullable ? "yes" : "no"}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ))
                )
              ) : (
                submissions.length === 0 ? (
                  <div className="flex flex-col items-start gap-2">
                    <p className="text-sm font-medium">No submissions yet</p>
                    <p className="text-[13px] text-muted-foreground">Submit your query to judge it against every fixture. Each attempt appears here.</p>
                  </div>
                ) : (
                  <div className="flex flex-col gap-3">
                    <ul className="flex flex-col overflow-hidden rounded-lg border border-[var(--solve-border)]">
                      {submissions.map((entry) => {
                        const tone = verdictPresentation({ status: "completed", verdict: entry.verdict as never }).tone
                        return (
                          <li key={entry.submissionId} className="flex items-center justify-between gap-4 border-b border-[var(--solve-border)] px-3.5 py-3 text-[13px] last:border-b-0">
                            <span className={cn("inline-flex items-center gap-2 font-medium", toneText[tone])}>
                              {tone === "ok" ? <CircleCheck className="size-4" aria-hidden="true" /> : tone === "err" ? <CircleX className="size-4" aria-hidden="true" /> : <TriangleAlert className="size-4" aria-hidden="true" />}
                              {verdictShortLabel(entry.verdict)}
                            </span>
                            <span className="text-muted-foreground">{entry.at ? relativeTime(entry.at, now) : entry.createdAt}</span>
                          </li>
                        )
                      })}
                    </ul>
                    <a className={cn(ghostSm, "self-start px-0 text-muted-foreground hover:bg-transparent hover:text-foreground")} href="#history">Open full history</a>
                  </div>
                )
              )}
            </div>
          </section>
        )}
        {maximized ? null : (
          <ResizeHandle
            orientation="vertical"
            label="Resize problem panel"
            value={problemWidth}
            minimum={MIN_PROBLEM_WIDTH}
            maximum={MAX_PROBLEM_WIDTH}
            onPointerDown={resizeProblemPanel}
            onKeyboardResize={keyboardResizeProblem}
          />
        )}

        <div className="flex min-h-0 min-w-0 w-full flex-1 flex-col gap-2 lg:w-0">
          <section aria-label="SQL editor" className={cn(pane, "min-h-[360px] flex-1")}>
            <div className={cn(paneHeader, "justify-between pr-1.5 pl-3")}>
              <div className="flex items-center gap-2.5">
                <span className="inline-flex items-center gap-1.5 text-[13px] font-medium">
                  <Code className="size-3.5 text-[var(--fn)]" aria-hidden="true" />
                  Code
                </span>
                <span className="inline-flex h-[26px] items-center px-2 text-[13px] text-muted-foreground">{dialectLabels[problem.dialect]}</span>
              </div>
              <div className="flex items-center gap-0.5">
                <span className="mr-2 text-xs text-muted-foreground">Draft kept</span>
                <button type="button" className={ghostIcon} aria-label="Reset to starter code" title="Reset" onClick={props.onResetDraft} disabled={!problem.id}>
                  <RotateCcw className="size-4" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={ghostIcon}
                  aria-label={maximized ? "Restore editor" : "Maximize editor"}
                  aria-pressed={maximized}
                  title={maximized ? "Restore" : "Maximize"}
                  onClick={() => setMaximized((current) => !current)}
                >
                  {maximized ? <Minimize2 className="size-4" aria-hidden="true" /> : <Maximize2 className="size-4" aria-hidden="true" />}
                </button>
              </div>
            </div>
            <div className="min-h-0 flex-1 py-2.5">
              <Editor
                height="100%"
                language="sql"
                theme={dark ? "sj-dark" : "sj-light"}
                beforeMount={defineSolveThemes}
                value={props.draft}
                onChange={props.onDraftChange}
                onMount={(editor, monaco) => {
                  editor.onDidChangeCursorPosition((event) =>
                    setCursor({ line: event.position.lineNumber, column: event.position.column })
                  )
                  completionProviderRef.current?.dispose()
                  completionProviderRef.current = monaco.languages.registerCompletionItemProvider("sql", {
                    provideCompletionItems: () => ({
                      suggestions: completionItemsRef.current.map((item) => ({
                        label: item.label,
                        detail: item.detail,
                        insertText: item.insertText,
                        insertTextRules:
                          item.kind === "snippet"
                            ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
                            : undefined,
                        kind:
                          item.kind === "table"
                            ? monaco.languages.CompletionItemKind.Struct
                            : item.kind === "column"
                              ? monaco.languages.CompletionItemKind.Field
                              : item.kind === "snippet"
                                ? monaco.languages.CompletionItemKind.Snippet
                                : monaco.languages.CompletionItemKind.Keyword,
                      })),
                    }),
                  })
                }}
                options={{
                  ariaLabel: `SQL solution for ${problem.title}`,
                  automaticLayout: true,
                  fontFamily: "'Geist Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
                  fontSize: 13.5,
                  lineHeight: 23,
                  lineNumbers: "on",
                  lineNumbersMinChars: 4,
                  lineDecorationsWidth: 16,
                  glyphMargin: false,
                  folding: false,
                  minimap: { enabled: false },
                  guides: { indentation: false },
                  bracketPairColorization: { enabled: false },
                  matchBrackets: "never",
                  overviewRulerBorder: false,
                  overviewRulerLanes: 0,
                  hideCursorInOverviewRuler: true,
                  renderLineHighlight: "line",
                  scrollBeyondLastLine: false,
                  tabSize: 2,
                  wordWrap: "on",
                  readOnly: !problem.id,
                }}
              />
            </div>
            <div className="flex justify-between gap-3 border-t border-[var(--solve-border)] px-3.5 py-1.5 text-xs text-muted-foreground">
              <span>Read-only · one statement</span>
              <span>
                Ln {cursor.line}, Col {cursor.column} · {props.draft.length.toLocaleString()} chars
              </span>
            </div>
          </section>

          {maximized ? null : (
            <>
              <ResizeHandle
                orientation="horizontal"
                label="Resize result panel"
                value={resultHeight}
                minimum={MIN_RESULT_HEIGHT}
                maximum={MAX_RESULT_HEIGHT}
                onPointerDown={resizeResultPanel}
                onKeyboardResize={keyboardResizeResult}
              />
              <section
                aria-label="Results"
                className={cn(pane, "min-h-[220px] flex-none")}
                style={{ flexBasis: `${resultHeight}%` }}
              >
              <div className={paneHeader} role="tablist" aria-label="Result sections">
                <PaneTab selected={resultTab === "run"} onSelect={() => setResultTab("run")} icon={<Play className="size-3.5" aria-hidden="true" />}>
                  Run result
                </PaneTab>
                <PaneTab selected={resultTab === "submit"} onSelect={() => setResultTab("submit")} icon={<Send className="size-3.5" aria-hidden="true" />}>
                  Submission
                </PaneTab>
              </div>
              <div role="tabpanel" className="flex flex-col gap-4 overflow-auto px-5 pt-4 pb-5" aria-live="polite">
                <ResultPanel
                  tab={resultTab}
                  busy={busy && activeMode === resultTab}
                  result={resultMode === resultTab ? result : null}
                />
              </div>
              </section>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

function ResultPanel({ tab, busy, result }: { tab: ResultTab; busy: boolean; result: JudgeRunResult | null }) {
  const [selection, setSelection] = useState<{ result: JudgeRunResult; index: number } | null>(null)
  if (busy) {
    return (
      <p className="inline-flex items-center gap-2 text-sm text-muted-foreground" role="status">
        <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
        {tab === "submit" ? "Judging against every fixture…" : "Running on the public sample…"}
      </p>
    )
  }
  if (!result) {
    return (
      <div className="flex flex-col gap-1.5">
        <h2 className="text-[17px] font-semibold">{tab === "submit" ? "No submission yet" : "Not run yet"}</h2>
        <p className="text-[13px] text-muted-foreground">
          {tab === "submit"
            ? "Submit to judge your query against every fixture in this problem."
            : "Run your query on the public sample to preview its rows before submitting."}
        </p>
      </div>
    )
  }

  const tests = result.tests ?? []
  const selectedIndex = selection?.result === result ? selection.index : initialFixture(tests)
  const selected = tests[selectedIndex]
  const summary = [fixtureSummary(tests), resultSummary(result)].filter(Boolean).join(" · ")
  const verdict = verdictPresentation(result)
  const VerdictIcon = verdict.tone === "ok" ? CircleCheck : verdict.tone === "err" ? CircleX : TriangleAlert

  return (
    <>
      <div className="flex flex-wrap items-baseline gap-x-3.5 gap-y-1.5">
        {tab === "run" ? (
          <h2 className="text-[17px] font-semibold">Ran on sample</h2>
        ) : (
          <h2 className={cn("inline-flex items-center gap-2 text-[19px] font-semibold", toneText[verdict.tone])}>
            <VerdictIcon className="size-5 self-center" aria-hidden="true" />
            {verdict.label}
          </h2>
        )}
        <span className="text-[13px] text-muted-foreground">{summary}</span>
      </div>
      {tab === "submit" && tests.length > 0 ? (
        <div role="tablist" aria-label="Fixtures" className="flex flex-wrap gap-1.5">
          {tests.map((test, index) => {
            const passed = test.verdict === "Accepted"
            const isSelected = index === selectedIndex
            return (
              <button
                key={test.id}
                type="button"
                role="tab"
                aria-selected={isSelected}
                onClick={() => setSelection({ result, index })}
                className={cn(
                  "inline-flex h-[30px] items-center gap-1.5 rounded-lg border px-2.5 text-[13px] font-medium",
                  focusRing,
                  isSelected
                    ? cn("bg-card shadow-xs-design", passed ? "border-ok" : "border-err")
                    : "border-transparent bg-muted hover:bg-hover"
                )}
              >
                <span className={cn("size-1.5 rounded-full", passed ? "bg-ok" : "bg-err")} aria-hidden="true" />
                Fixture {index + 1}
                {isSelected ? <span className="font-normal text-muted-foreground">· {fixtureName(test.id)}</span> : null}
                <span className="sr-only">{passed ? "passed" : verdictShortLabel(test.verdict)}</span>
              </button>
            )
          })}
        </div>
      ) : null}
      <div className="flex flex-wrap items-start gap-4">
        <ResultTable result={result} />
        <div className="flex flex-[1_1_240px] flex-col gap-2 rounded-lg bg-muted px-3.5 py-3 text-[13px]">
          {tab === "run" ? (
            <>
              <span className="font-medium">
                Sample check: <span className={toneText[verdict.tone]}>{verdict.label}</span>
              </span>
              <span className="text-muted-foreground">{result.message}</span>
              <span className="mt-1 flex gap-1.5">
                <Kbd>Ctrl Shift ↵</Kbd>
                <span className="self-center text-muted-foreground">to submit against every fixture</span>
              </span>
            </>
          ) : (
            <>
              {selected ? (
                <>
                  <span className="font-medium">
                    Fixture {selectedIndex + 1} · {fixtureName(selected.id)}:{" "}
                    <span className={toneText[verdictPresentation({ status: "completed", verdict: selected.verdict }).tone]}>
                      {verdictShortLabel(selected.verdict)}
                    </span>
                  </span>
                  <span className="text-muted-foreground">
                    {selected.hidden
                      ? "Hidden fixture — its data and expected rows stay private. Check ties, NULLs and empty groups."
                      : "Public fixture — uses the example data shown in the description."}{" "}
                    Ran in {selected.elapsedMs} ms.
                  </span>
                </>
              ) : (
                <span className="font-medium">Judge message</span>
              )}
              <span className="text-muted-foreground">{result.message}</span>
            </>
          )}
        </div>
      </div>
    </>
  )
}

function ResultTable({ result }: { result: JudgeRunResult }) {
  if (result.status !== "completed" || !result.columns?.length) {
    return null
  }
  const columns = result.columns
  const rows = result.rows ?? []
  const visibleRows = rows.slice(0, MAX_RENDERED_ROWS)
  return (
    <div className="flex max-w-full flex-col gap-1.5">
      <span className="text-xs font-medium text-muted-foreground">
        {rows.length > visibleRows.length
          ? `Your output · showing ${visibleRows.length} of ${rows.length} rows`
          : `Your output · ${rows.length} row${rows.length === 1 ? "" : "s"}`}
      </span>
      <div className="max-h-72 overflow-auto">
        <table className={miniTable}>
          <thead>
            <tr>
              {columns.map((column, index) => (
                <th key={`${column.name}-${index}`} className={miniTh}>
                  {column.name}
                  <small className="block font-mono text-[11px] font-normal text-muted-foreground">{column.type}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {visibleRows.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className={cn(miniTd, "font-sans text-muted-foreground")}>Query returned no rows.</td>
              </tr>
            ) : (
              visibleRows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {columns.map((column, columnIndex) => (
                    <ValueCell key={`${column.name}-${columnIndex}`} value={row[columnIndex]} />
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function ValueCell({ value }: { value: string | number | boolean | null | undefined }) {
  if (value === null || value === undefined) {
    return <td className={cn(miniTd, "text-muted-foreground italic")}>NULL</td>
  }
  const text = typeof value === "boolean" ? (value ? "true" : "false") : String(value)
  const shown = text.slice(0, MAX_RENDERED_CELL_LENGTH)
  return (
    <td className={cn(miniTd, "max-w-64 truncate")} title={text}>
      {shown}
      {text.length > shown.length ? "…" : ""}
    </td>
  )
}

function PaneTab({
  selected,
  onSelect,
  icon,
  children,
}: {
  selected: boolean
  onSelect: () => void
  icon: ReactNode
  children: ReactNode
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      onClick={onSelect}
      className={cn(
        "inline-flex h-[30px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium hover:bg-hover",
        focusRing,
        selected ? "bg-hover text-foreground" : "text-muted-foreground"
      )}
    >
      {icon}
      {children}
    </button>
  )
}

function ResizeHandle({
  orientation,
  label,
  value,
  minimum,
  maximum,
  onPointerDown,
  onKeyboardResize,
}: {
  orientation: "horizontal" | "vertical"
  label: string
  value: number
  minimum: number
  maximum: number
  onPointerDown: PointerEventHandler<HTMLDivElement>
  onKeyboardResize: (direction: -1 | 1) => void
}) {
  const horizontal = orientation === "horizontal"
  const onKeyDown: KeyboardEventHandler<HTMLDivElement> = (event) => {
    const direction = horizontal
      ? event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowUp"
          ? -1
          : 0
      : event.key === "ArrowRight"
        ? 1
        : event.key === "ArrowLeft"
          ? -1
          : 0
    if (direction === 0) {
      return
    }
    event.preventDefault()
    onKeyboardResize(direction)
  }

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={minimum}
      aria-valuemax={maximum}
      aria-valuenow={Math.round(value)}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      className={cn(
        "group relative z-10 shrink-0 rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring",
        horizontal
          ? "flex h-2 w-full cursor-row-resize items-center justify-center"
          : "hidden w-2 cursor-col-resize items-center justify-center lg:flex"
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "rounded-full bg-border transition-colors group-hover:bg-ring group-focus-visible:bg-ring",
          horizontal ? "h-0.5 w-10" : "h-10 w-0.5"
        )}
      />
    </div>
  )
}

function NavArrow({ href, label, children }: { href?: string; label: string; children: ReactNode }) {
  return href ? (
    <a className={ghostIcon} href={href} aria-label={label}>
      {children}
    </a>
  ) : (
    <button type="button" className={ghostIcon} aria-label={label} disabled>
      {children}
    </button>
  )
}

function Kbd({ children }: { children: string }) {
  return (
    <kbd className="inline-flex h-5 items-center rounded-[5px] border border-[var(--solve-border)] bg-card px-[5px] font-mono text-[11px] font-normal text-muted-foreground">
      {children}
    </kbd>
  )
}
