import { useEffect, useRef, useState } from "react"
import { LoaderCircle, RefreshCw, Search, Shuffle } from "lucide-react"

import { parseHash, problemHref } from "@/app-route-state"
import { Button } from "@/components/ui/button"
import {
  listInstalledPacks,
  listSubmissionHistory,
  type InstalledPack,
  type SubmissionHistorySummary,
} from "@/lib/judge-api"
import { cn } from "@/lib/utils"

import { catalogProblems, type ProblemDifficulty } from "./catalog"
import { learningLoadErrorMessage } from "./load-state"
import { paginateProblems } from "./practice-workspace-state"
import { difficultyProgress, problemStatuses } from "./problem-status"
import { ProblemsAside, problemsCardClass } from "./problems-aside"
import { filterCatalog, topicCounts, type CatalogFilters, type StatusFilter } from "./problems-filter"
import { ProblemsPagination } from "./problems-pagination"
import { ProblemsTable } from "./problems-table"

const PAGE_SIZE = 10
const selectClass =
  "h-9 max-w-40 rounded-lg border bg-card px-3 text-sm font-medium shadow-[0_1px_2px_rgba(16,16,24,0.05)] outline-none hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50"
const chipClass =
  "inline-flex h-[30px] items-center gap-1.5 rounded-full border bg-card px-3 text-[13px] font-medium hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground"

export function ProblemsScreen() {
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [history, setHistory] = useState<SubmissionHistorySummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)
  const [page, setPage] = useState(0)
  const [filters, setFilters] = useState<CatalogFilters>(() => ({
    query: "",
    topic: "all",
    packId: parseHash(window.location.hash).params.get("pack") ?? "all",
    difficulty: "all",
    status: "all",
  }))
  const searchRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    let active = true
    void Promise.all([listInstalledPacks(), listSubmissionHistory().catch(() => [])])
      .then(([nextPacks, nextHistory]) => {
        if (active) {
          setPacks(nextPacks)
          setHistory(nextHistory)
        }
      })
      .catch(() => active && setError(learningLoadErrorMessage("catalog")))
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [reloadToken])

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      const target = event.target as HTMLElement
      if (event.key === "/" && !target.closest("input, textarea, select, [contenteditable='true']")) {
        event.preventDefault()
        searchRef.current?.focus()
      }
    }
    window.addEventListener("keydown", focusSearch)
    return () => window.removeEventListener("keydown", focusSearch)
  }, [])

  const problems = catalogProblems(packs)
  const statuses = problemStatuses(history)
  const filtered = filterCatalog(problems, filters, statuses)
  const paged = paginateProblems(filtered, page, PAGE_SIZE)
  const packTitles = new Map(packs.map((pack) => [pack.packId, pack.title]))

  function updateFilters(patch: Partial<CatalogFilters>) {
    setFilters((current) => ({ ...current, ...patch }))
    setPage(0)
  }

  return (
    <main className="min-h-0 flex-1 overflow-auto px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1200px] flex-wrap items-start gap-7">
        <div className="flex min-w-0 flex-[3_1_640px] flex-col gap-5">
          <header className="flex flex-col gap-1.5">
            <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">Problems</h1>
            <p className="text-[15px] text-muted-foreground">
              {problems.length} problems from {packs.length} installed {packs.length === 1 ? "pack" : "packs"}. Open one to start writing SQL.
            </p>
          </header>
          <div role="group" aria-label="Filter by topic" className="flex flex-wrap gap-2">
            <button type="button" className={chipClass} aria-pressed={filters.topic === "all"} onClick={() => updateFilters({ topic: "all" })}>
              All topics
            </button>
            {topicCounts(problems).map(({ topic, count }) => (
              <button key={topic} type="button" className={cn(chipClass, "group")} aria-pressed={filters.topic === topic} onClick={() => updateFilters({ topic })}>
                {topic} <span className="font-normal text-muted-foreground group-aria-pressed:text-primary-foreground/70">{count}</span>
              </button>
            ))}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <label className="flex h-9 min-w-0 flex-[1_1_260px] items-center gap-2 rounded-lg border border-input bg-card px-3 shadow-[0_1px_2px_rgba(16,16,24,0.05)] focus-within:ring-3 focus-within:ring-ring/50">
              <Search className="size-4 shrink-0 text-muted-foreground" />
              <input
                ref={searchRef}
                type="search"
                aria-label="Search problems"
                placeholder="Search by title or topic"
                className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
                value={filters.query}
                onChange={(event) => updateFilters({ query: event.target.value })}
              />
              <kbd className="flex h-5 items-center rounded-[5px] border px-1.5 font-mono text-[11px] text-muted-foreground">/</kbd>
            </label>
            <select aria-label="Filter by pack" className={selectClass} value={filters.packId} onChange={(event) => updateFilters({ packId: event.target.value })}>
              <option value="all">Pack</option>
              {packs.map((pack) => <option key={`${pack.packId}@${pack.version}`} value={pack.packId}>{pack.title}</option>)}
            </select>
            <select aria-label="Filter by difficulty" className={selectClass} value={filters.difficulty} onChange={(event) => updateFilters({ difficulty: event.target.value as ProblemDifficulty | "all" })}>
              <option value="all">Difficulty</option>
              <option value="easy">Easy</option>
              <option value="medium">Medium</option>
              <option value="hard">Hard</option>
            </select>
            <select aria-label="Filter by status" className={selectClass} value={filters.status} onChange={(event) => updateFilters({ status: event.target.value as StatusFilter })}>
              <option value="all">Status</option>
              <option value="solved">Solved</option>
              <option value="attempted">Attempted</option>
              <option value="todo">Not started</option>
            </select>
            <Button variant="outline" size="icon-lg" aria-label="Open a random problem" title="Open a random problem" disabled={filtered.length === 0} onClick={() => openRandomProblem(filtered)}>
              <Shuffle />
            </Button>
          </div>
          <div className={cn(problemsCardClass, "overflow-hidden")}>
            {loading ? (
              <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
                <LoaderCircle className="size-4 animate-spin" /> Loading installed problem packs...
              </p>
            ) : error ? (
              <div className="flex flex-col items-start gap-3 p-6 text-sm" role="alert">
                <p className="text-destructive">{error}</p>
                <Button variant="outline" size="sm" onClick={() => { setError(null); setLoading(true); setReloadToken((token) => token + 1) }}>
                  <RefreshCw /> Retry
                </Button>
              </div>
            ) : problems.length === 0 ? (
              <div className="flex flex-col items-start gap-3 p-6 text-sm text-muted-foreground">
                <p>No problem packs are installed yet. Import a validated pack to start practicing.</p>
                <Button variant="outline" size="sm" nativeButton={false} render={<a href="#packs" />}>Open packs</Button>
              </div>
            ) : filtered.length === 0 ? (
              <p className="p-6 text-sm text-muted-foreground">No problems match these filters.</p>
            ) : (
              <>
                <ProblemsTable problems={paged.items} packTitles={packTitles} statuses={statuses} />
                <ProblemsPagination paged={paged} total={filtered.length} onPage={setPage} />
              </>
            )}
          </div>
        </div>
        <ProblemsAside
          progress={difficultyProgress(problems, statuses)}
          packs={packs}
          totalProblems={problems.length}
          packId={filters.packId}
          onSelectPack={(packId) => updateFilters({ packId })}
        />
      </div>
    </main>
  )
}

function openRandomProblem(problems: Array<{ id: string }>) {
  const pick = problems[Math.floor(Math.random() * problems.length)]
  if (pick) {
    window.location.assign(problemHref(pick.id))
  }
}
