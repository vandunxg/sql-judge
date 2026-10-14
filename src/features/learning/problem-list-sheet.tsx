import { useState } from "react"
import { ChevronLeft, ChevronRight, Menu } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import type { InstalledPack } from "@/lib/judge-api"
import { cn } from "@/lib/utils"

import {
  filterProblems,
  type LearningProblem,
  type ProblemDifficulty,
} from "./catalog"
import { DifficultyBadge } from "./difficulty-badge"
import { pageForProblem, paginateProblems } from "./practice-workspace-state"

const selectClass =
  "h-8 rounded-lg border border-input bg-background px-2 text-sm text-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/50"

type ProblemListSheetProps = {
  packs: InstalledPack[]
  selectedPackId: string
  problems: LearningProblem[]
  selectedId: string
  disabled: boolean
  onSelectPack: (packId: string) => void
  onSelectProblem: (problemId: string) => void
}

export function ProblemListSheet({
  packs,
  selectedPackId,
  disabled,
  onSelectPack,
  ...listProps
}: ProblemListSheetProps) {
  const [open, setOpen] = useState(false)

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger
        render={<Button variant="ghost" size="sm" disabled={disabled} />}
      >
        <Menu />
        Problem list
      </SheetTrigger>
      <SheetContent side="left" className="gap-0">
        <SheetHeader className="gap-3 border-b">
          <SheetTitle>Problems</SheetTitle>
          <SheetDescription>Pick a problem from one installed pack.</SheetDescription>
          <label className="flex flex-col gap-1 text-xs font-medium text-muted-foreground">
            Pack
            <select
              className={selectClass}
              value={selectedPackId}
              onChange={(event) => onSelectPack(event.target.value)}
            >
              {packs.map((pack) => (
                <option key={`${pack.packId}-${pack.version}`} value={pack.packId}>
                  {pack.title} · {pack.version}
                </option>
              ))}
            </select>
          </label>
        </SheetHeader>
        <PackProblemList
          key={selectedPackId}
          {...listProps}
          onSelectProblem={(problemId) => {
            listProps.onSelectProblem(problemId)
            setOpen(false)
          }}
        />
      </SheetContent>
    </Sheet>
  )
}

function PackProblemList({
  problems,
  selectedId,
  onSelectProblem,
}: Pick<ProblemListSheetProps, "problems" | "selectedId" | "onSelectProblem">) {
  const [query, setQuery] = useState("")
  const [difficulty, setDifficulty] = useState<ProblemDifficulty | "all">("all")
  const filtered = filterProblems(problems, query, "all", difficulty)
  const [page, setPage] = useState(() => pageForProblem(filtered, selectedId))
  const paged = paginateProblems(filtered, page)

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
      <div className="flex gap-2">
        <Input
          aria-label="Search problems"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setPage(0)
          }}
          placeholder="Search by title or topic"
        />
        <select
          aria-label="Filter by difficulty"
          className={selectClass}
          value={difficulty}
          onChange={(event) => {
            setDifficulty(event.target.value as ProblemDifficulty | "all")
            setPage(0)
          }}
        >
          <option value="all">All</option>
          <option value="easy">Easy</option>
          <option value="medium">Medium</option>
          <option value="hard">Hard</option>
        </select>
      </div>
      {filtered.length === 0 ? (
        <p className="rounded-lg border border-dashed p-4 text-center text-xs text-muted-foreground">
          No problems match these filters.
        </p>
      ) : (
        <div className="grid gap-1" role="listbox" aria-label="SQL problems">
          {paged.items.map((problem) => (
            <button
              key={problem.id}
              type="button"
              role="option"
              aria-selected={problem.id === selectedId}
              onClick={() => onSelectProblem(problem.id)}
              className={cn(
                "flex items-start justify-between gap-2 rounded-lg px-3 py-2.5 text-left hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50",
                problem.id === selectedId && "bg-accent"
              )}
            >
              <span className="min-w-0">
                <span className="block font-medium">{problem.title}</span>
                <span className="block truncate text-xs text-muted-foreground">
                  {problem.topics.join(" · ") || problem.dialect}
                </span>
              </span>
              <DifficultyBadge difficulty={problem.difficulty} />
            </button>
          ))}
        </div>
      )}
      {filtered.length > 0 ? (
        <div className="mt-auto flex items-center justify-between border-t pt-3 text-xs text-muted-foreground">
          <span>
            Showing {paged.start + 1}–{paged.end} of {filtered.length}
          </span>
          <div className="flex gap-1">
            <Button
              variant="outline"
              size="icon-sm"
              aria-label="Previous page"
              disabled={paged.page === 0}
              onClick={() => setPage(paged.page - 1)}
            >
              <ChevronLeft />
            </Button>
            <Button
              variant="outline"
              size="icon-sm"
              aria-label="Next page"
              disabled={paged.page >= paged.totalPages - 1}
              onClick={() => setPage(paged.page + 1)}
            >
              <ChevronRight />
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  )
}
