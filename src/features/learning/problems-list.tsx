import { useEffect, useMemo, useRef, useState } from "react"
import {
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Plus,
  RefreshCw,
  Search,
  Shuffle,
} from "lucide-react"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { parseHash } from "@/app-route-state"
import { cn } from "@/lib/utils"
import type { InstalledPack, SubmissionHistorySummary } from "@/lib/judge-api"
import { catalogProblemKey, type LearningProblem, type ProblemDifficulty } from "./catalog"
import {
  matchesStatus,
  problemProgress,
  progressTotals,
  relativeTime,
  verdictShortLabel,
  type StatusFilter,
} from "./problem-progress"
import {
  filterCatalogProblems,
  pageWindow,
  paginateList,
  topicCounts,
  topicLabel,
  visibleTopics,
} from "./problems-list-state"

const difficultyLabels: Record<ProblemDifficulty, string> = {
  easy: "Easy",
  medium: "Medium",
  hard: "Hard",
}

const difficultyColors: Record<ProblemDifficulty, string> = {
  easy: "text-ok",
  medium: "text-warn",
  hard: "text-err",
}

const difficultyBars: Record<ProblemDifficulty, string> = {
  easy: "bg-ok",
  medium: "bg-warn",
  hard: "bg-err",
}

const TOPIC_LIMIT = 6
// Circumference of the r=15.5 progress ring in the 36x36 viewBox.
const RING_LENGTH = 2 * Math.PI * 15.5

const statusOptions: Array<{ value: StatusFilter; label: string }> = [
  { value: "all", label: "All statuses" },
  { value: "solved", label: "Solved" },
  { value: "attempted", label: "Attempted" },
  { value: "unattempted", label: "Not attempted" },
]

function progressKey(packId: string, version: string, digest: string, problemId: string) {
  return catalogProblemKey({ packId, version, digest }, problemId)
}

const designCard = "rounded-xl border bg-card shadow-xs-design"
const outlineButton =
  "inline-flex h-9 items-center justify-center gap-2 rounded-lg border bg-card px-3.5 text-sm leading-none font-medium whitespace-nowrap shadow-xs-design outline-none hover:bg-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-45"
const smallIconButton =
  "inline-flex size-8 items-center justify-center rounded-lg border border-transparent text-[13px] font-medium outline-none hover:bg-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-45"
const focusRing =
  "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"

function solveHref(problemId: string) {
  return `#solve/${encodeURIComponent(problemId)}`
}

export function ProblemsList({
  packs,
  problems,
  history,
  loading,
  error,
  onRetry,
}: {
  packs: InstalledPack[]
  problems: LearningProblem[]
  history: SubmissionHistorySummary[]
  loading: boolean
  error: string | null
  onRetry: () => void
}) {
  const [query, setQuery] = useState("")
  const [topic, setTopic] = useState("all")
  // Packs links here as `#problems?pack=<id>` to open the list pre-filtered.
  const [packId, setPackId] = useState(() => parseHash(window.location.hash).params.get("pack") ?? "all")
  const [difficulty, setDifficulty] = useState<ProblemDifficulty | "all">("all")
  const [status, setStatus] = useState<StatusFilter>("all")
  const [topicsExpanded, setTopicsExpanded] = useState(false)
  const [page, setPage] = useState(0)
  const [now] = useState(() => Date.now())
  const searchRef = useRef<HTMLInputElement>(null)

  const topics = useMemo(() => topicCounts(problems), [problems])
  const progress = useMemo(() => problemProgress(history, progressKey), [history])
  const totals = useMemo(() => progressTotals(problems, progress), [problems, progress])
  const filtered = useMemo(
    () =>
      filterCatalogProblems(problems, { query, topic, packId, difficulty }).filter((problem) =>
        matchesStatus(progress.get(problem.id), status)
      ),
    [difficulty, packId, problems, progress, query, status, topic]
  )
  const topicChips = visibleTopics(topics, {
    expanded: topicsExpanded,
    selected: topic,
    limit: TOPIC_LIMIT,
  })
  const visible = paginateList(filtered, page)
  const packTitles = new Map(packs.map((pack) => [pack.packId, pack.title]))
  const problemNumbers = useMemo(
    () => new Map(problems.map((problem, index) => [problem.id, index + 1])),
    [problems]
  )

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null
      if (
        event.key !== "/" ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        target?.closest("input, textarea, select, [contenteditable='true']")
      ) {
        return
      }
      event.preventDefault()
      searchRef.current?.focus()
    }
    window.addEventListener("keydown", focusSearch)
    return () => window.removeEventListener("keydown", focusSearch)
  }, [])

  function updateFilter(apply: () => void) {
    apply()
    setPage(0)
  }

  function openRandomProblem() {
    const problem = filtered[Math.floor(Math.random() * filtered.length)]
    if (problem) {
      window.location.hash = solveHref(problem.id)
    }
  }

  return (
    <main className="flex-1 px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1200px] flex-wrap items-start gap-7">
        <section className="flex min-w-0 flex-[3_1_640px] flex-col gap-5">
          <div className="flex flex-col gap-1.5">
            <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">
              Problems
            </h1>
            <p className="text-[15px] text-muted-foreground">
              {problems.length} problem{problems.length === 1 ? "" : "s"} from{" "}
              {packs.length} installed pack{packs.length === 1 ? "" : "s"}.
              Open one to start writing SQL.
            </p>
          </div>

          {topics.length > 0 ? (
            <div
              role="group"
              aria-label="Filter by topic"
              className="flex flex-wrap gap-2"
            >
              <TopicChip
                label="All topics"
                pressed={topic === "all"}
                onClick={() => updateFilter(() => setTopic("all"))}
              />
              {topicChips.shown.map((entry) => (
                <TopicChip
                  key={entry.topic}
                  label={topicLabel(entry.topic)}
                  count={entry.count}
                  pressed={topic === entry.topic}
                  onClick={() => updateFilter(() => setTopic(entry.topic))}
                />
              ))}
              {topicChips.hiddenCount > 0 || topicsExpanded ? (
                <button
                  type="button"
                  aria-expanded={topicsExpanded}
                  onClick={() => setTopicsExpanded((current) => !current)}
                  className={cn(
                    "inline-flex h-[30px] items-center gap-1 rounded-full px-2.5 text-[13px] font-medium text-muted-foreground hover:bg-hover hover:text-foreground",
                    focusRing
                  )}
                >
                  {topicsExpanded ? "Show less" : `+${topicChips.hiddenCount} more`}
                  {topicsExpanded ? (
                    <ChevronUp className="size-3.5" aria-hidden="true" />
                  ) : (
                    <ChevronDown className="size-3.5" aria-hidden="true" />
                  )}
                </button>
              ) : null}
            </div>
          ) : null}

          <div className="flex flex-wrap items-center gap-2">
            <label className="flex h-9 min-w-0 flex-[1_1_260px] items-center gap-2 rounded-lg border border-input bg-card px-3 text-sm shadow-xs-design focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-ring">
              <Search className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
              <input
                ref={searchRef}
                type="search"
                value={query}
                onChange={(event) =>
                  updateFilter(() => setQuery(event.target.value))
                }
                placeholder="Search by title or topic"
                aria-label="Search problems"
                aria-keyshortcuts="/"
                className="min-w-0 flex-1 border-0 bg-transparent outline-none placeholder:text-muted-foreground"
              />
              <Kbd>/</Kbd>
            </label>
            <FilterMenu
              label="Pack"
              value={packId}
              options={[
                { value: "all", label: "All packs" },
                ...packs.map((pack) => ({ value: pack.packId, label: pack.title })),
              ]}
              onChange={(value) => updateFilter(() => setPackId(value))}
            />
            <FilterMenu
              label="Difficulty"
              value={difficulty}
              options={[
                { value: "all", label: "All difficulties" },
                { value: "easy", label: "Easy" },
                { value: "medium", label: "Medium" },
                { value: "hard", label: "Hard" },
              ]}
              onChange={(value) =>
                updateFilter(() =>
                  setDifficulty(value as ProblemDifficulty | "all")
                )
              }
            />
            <FilterMenu
              label="Status"
              value={status}
              options={statusOptions}
              onChange={(value) => updateFilter(() => setStatus(value as StatusFilter))}
            />
            <button
              type="button"
              className={cn(outlineButton, "w-9 px-0")}
              aria-label="Open a random problem"
              title="Random problem"
              disabled={filtered.length === 0}
              onClick={openRandomProblem}
            >
              <Shuffle className="size-4" aria-hidden="true" />
            </button>
          </div>

          <div className={cn(designCard, "overflow-hidden")}>
            {loading ? (
              <p className="p-6 text-sm text-muted-foreground" role="status">
                Loading installed problem packs...
              </p>
            ) : error ? (
              <div className="flex flex-col items-start gap-3 p-6" role="alert">
                <p className="text-sm text-err">{error}</p>
                <button type="button" className={outlineButton} onClick={onRetry}>
                  <RefreshCw className="size-4" aria-hidden="true" />
                  Retry
                </button>
              </div>
            ) : problems.length === 0 ? (
              <div className="flex flex-col items-start gap-3 p-6">
                <p className="text-sm text-muted-foreground">
                  No problems are installed yet. Import a validated pack to
                  begin practicing.
                </p>
                <a className={outlineButton} href="#packs">
                  <Plus className="size-4" aria-hidden="true" />
                  Import a pack
                </a>
              </div>
            ) : (
              <>
                <div className="overflow-x-auto">
                  <table className="w-full table-fixed border-collapse text-sm">
                    <colgroup>
                      <col className="w-16" />
                      <col />
                      <col className="w-[120px]" />
                      <col className="w-[160px]" />
                    </colgroup>
                    <thead>
                      <tr className="border-b text-left text-[13px] whitespace-nowrap text-muted-foreground">
                        <th className="py-3 pr-2 pl-5 font-medium">Status</th>
                        <th className="px-3 py-3 font-medium">Problem</th>
                        <th className="px-3 py-3 font-medium">Difficulty</th>
                        <th className="py-3 pr-5 pl-3 font-medium">Last attempt</th>
                      </tr>
                    </thead>
                    <tbody>
                      {visible.items.length === 0 ? (
                        <tr>
                          <td colSpan={4} className="h-28 px-5 text-center text-sm text-muted-foreground">
                            No problems match these filters.
                          </td>
                        </tr>
                      ) : (
                        visible.items.map((problem) => {
                          const entry = progress.get(problem.id)
                          const topicsText = problem.topics.map(topicLabel).join(", ")
                          return (
                            <tr key={problem.id} className="border-b transition-colors last:border-b-0 hover:bg-hover">
                              <td className="py-3.5 pr-2 pl-5 align-middle">
                                <StatusIcon status={entry?.status} />
                              </td>
                              <td className="min-w-0 px-3 py-3.5 align-middle">
                                <a
                                  className={cn("block truncate font-medium underline-offset-3 hover:underline", focusRing)}
                                  href={solveHref(problem.id)}
                                  title={problem.title}
                                >
                                  {problemNumbers.get(problem.id)}. {problem.title}
                                </a>
                                <div className="mt-1 truncate text-[13px] text-muted-foreground" title={topicsText}>
                                  {packTitles.get(problem.packId) ?? problem.packId}
                                  {topicsText ? <span className="text-muted-foreground/70"> · {topicsText}</span> : null}
                                </div>
                              </td>
                              <td className="px-3 py-3.5 align-middle">
                                <span className={cn("font-medium", difficultyColors[problem.difficulty])}>
                                  {difficultyLabels[problem.difficulty]}
                                </span>
                              </td>
                              <td className="py-3.5 pr-5 pl-3 align-middle text-[13px]">
                                {entry ? (
                                  <>
                                    <div className="truncate">{verdictShortLabel(entry.last.verdict)}</div>
                                    <div className="mt-1 truncate text-muted-foreground">{relativeTime(entry.last.at, now)}</div>
                                  </>
                                ) : (
                                  <span className="text-muted-foreground">—</span>
                                )}
                              </td>
                            </tr>
                          )
                        })
                      )}
                    </tbody>
                  </table>
                </div>
                <div className="flex flex-wrap items-center justify-between gap-3 border-t px-5 py-3">
                  <span className="text-[13px] text-muted-foreground">
                    {filtered.length === 0
                      ? "Showing 0 of 0"
                      : `Showing ${visible.start + 1}–${visible.end} of ${filtered.length}`}
                  </span>
                  <nav aria-label="Pagination" className="flex gap-1">
                    <button
                      type="button"
                      className={smallIconButton}
                      aria-label="Previous page"
                      disabled={visible.page === 0}
                      onClick={() => setPage(visible.page - 1)}
                    >
                      <ChevronLeft className="size-4" aria-hidden="true" />
                    </button>
                    {pageWindow(visible.page, visible.totalPages).map((pageIndex) => (
                      <button
                        key={pageIndex}
                        type="button"
                        className={cn(
                          smallIconButton,
                          pageIndex === visible.page &&
                            "border-border bg-card shadow-xs-design"
                        )}
                        aria-label={`Page ${pageIndex + 1}`}
                        aria-current={pageIndex === visible.page ? "page" : undefined}
                        onClick={() => setPage(pageIndex)}
                      >
                        {pageIndex + 1}
                      </button>
                    ))}
                    <button
                      type="button"
                      className={smallIconButton}
                      aria-label="Next page"
                      disabled={visible.page >= visible.totalPages - 1}
                      onClick={() => setPage(visible.page + 1)}
                    >
                      <ChevronRight className="size-4" aria-hidden="true" />
                    </button>
                  </nav>
                </div>
              </>
            )}
          </div>
        </section>

        <aside
          aria-label="Your progress"
          className="flex min-w-0 flex-[1_1_280px] flex-col gap-5 pt-1"
        >
          <section className={cn(designCard, "flex flex-col gap-[18px] p-5")}>
            <h2 className="text-[15px] font-semibold">Your progress</h2>
            <div className="flex items-center gap-5">
              <div className="relative size-24 shrink-0">
                <svg
                  viewBox="0 0 36 36"
                  className="size-24 -rotate-90"
                  role="img"
                  aria-label={`${totals.solved} of ${totals.total} solved`}
                >
                  <circle cx="18" cy="18" r="15.5" fill="none" stroke="var(--muted)" strokeWidth="3" />
                  {totals.solved > 0 ? (
                    <circle
                      cx="18"
                      cy="18"
                      r="15.5"
                      fill="none"
                      stroke="var(--ok)"
                      strokeWidth="3"
                      strokeLinecap="round"
                      strokeDasharray={`${(totals.solved / totals.total) * RING_LENGTH} ${RING_LENGTH}`}
                    />
                  ) : null}
                </svg>
                <div className="absolute inset-0 flex flex-col items-center justify-center leading-[1.1]">
                  <span className="text-[22px] font-semibold">{totals.solved}</span>
                  <span className="text-xs text-muted-foreground">of {totals.total}</span>
                </div>
              </div>
              <div className="flex flex-1 flex-col gap-2.5 text-[13px]">
                {(["easy", "medium", "hard"] as const).map((level) => (
                  <div key={level} className="flex flex-col gap-1">
                    <div className="flex justify-between">
                      <span className={cn("font-medium", difficultyColors[level])}>
                        {difficultyLabels[level]}
                      </span>
                      <span className="text-muted-foreground tabular-nums">
                        {totals[level].solved} / {totals[level].total}
                      </span>
                    </div>
                    <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                      <span
                        className={cn("block h-full rounded-full", difficultyBars[level])}
                        style={{ width: `${totals[level].total ? (totals[level].solved / totals[level].total) * 100 : 0}%` }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </section>

          <section className={cn(designCard, "flex flex-col p-2")}>
            <h2 className="px-3 pt-3 pb-2 text-[15px] font-semibold">Packs</h2>
            <PackButton
              label="All packs"
              count={problems.length}
              pressed={packId === "all"}
              onClick={() => updateFilter(() => setPackId("all"))}
            />
            {packs.map((pack) => (
              <PackButton
                key={pack.packId}
                label={pack.title}
                count={pack.problems.length}
                pressed={packId === pack.packId}
                onClick={() => updateFilter(() => setPackId(pack.packId))}
              />
            ))}
            <a
              href="#packs"
              className={cn(
                "mt-1 flex h-[34px] items-center gap-2.5 rounded-lg px-2.5 text-[13px] text-muted-foreground hover:bg-hover",
                focusRing
              )}
            >
              <Plus className="size-4" aria-hidden="true" />
              Import another pack
            </a>
          </section>
        </aside>
      </div>
    </main>
  )
}

function Kbd({ children }: { children: string }) {
  return (
    <kbd className="inline-flex h-5 items-center rounded-[5px] border bg-card px-[5px] font-mono text-[11px] text-muted-foreground">
      {children}
    </kbd>
  )
}

function TopicChip({
  label,
  count,
  pressed,
  onClick,
}: {
  label: string
  count?: number
  pressed: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      className={cn(
        "inline-flex h-[30px] items-center gap-1.5 rounded-full border bg-card px-3 text-[13px] font-medium",
        focusRing,
        pressed
          ? "border-primary bg-primary text-primary-foreground"
          : "hover:bg-hover"
      )}
    >
      {label}
      {count === undefined ? null : (
        <span
          className={cn(
            "font-normal",
            pressed ? "text-primary-foreground opacity-70" : "text-muted-foreground"
          )}
        >
          {count}
        </span>
      )}
    </button>
  )
}

function FilterMenu({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: string
  options: Array<{ value: string; label: string }>
  onChange: (value: string) => void
}) {
  const selected = value === "all" ? null : options.find((option) => option.value === value)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        className={outlineButton}
        aria-label={selected ? `${label}: ${selected.label}` : `${label}: all`}
      >
        {selected ? selected.label : label}
        <ChevronDown className="size-4 text-muted-foreground" aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-auto min-w-44">
        <DropdownMenuRadioGroup value={value} onValueChange={(next) => onChange(String(next))}>
          {options.map((option) => (
            <DropdownMenuRadioItem key={option.value} value={option.value}>
              {option.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function PackButton({
  label,
  count,
  pressed,
  onClick,
}: {
  label: string
  count: number
  pressed: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      title={label}
      className={cn(
        "flex min-h-[38px] items-center gap-4 rounded-lg px-3 py-2 text-left text-sm",
        focusRing,
        pressed ? "bg-hover font-medium" : "hover:bg-hover"
      )}
    >
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="shrink-0 text-[13px] font-normal text-muted-foreground tabular-nums">{count}</span>
    </button>
  )
}

function StatusIcon({ status }: { status?: "solved" | "attempted" }) {
  if (status === "solved") {
    return <Check className="size-4 text-ok" role="img" aria-label="Solved" />
  }
  if (status === "attempted") {
    return (
      <svg viewBox="0 0 24 24" className="size-4 text-warn" fill="none" stroke="currentColor" strokeWidth="2" role="img" aria-label="Attempted">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor" stroke="none" />
      </svg>
    )
  }
  return <span className="sr-only">Not attempted</span>
}
