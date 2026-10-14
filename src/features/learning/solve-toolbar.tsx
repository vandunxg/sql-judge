import type { ComponentProps } from "react"
import { ChevronLeft, ChevronRight, LoaderCircle, Play, Send, Square } from "lucide-react"

import { Button } from "@/components/ui/button"

import { ProblemListSheet } from "./problem-list-sheet"
import type { JudgeMode } from "./use-judge-run"

type SolveToolbarProps = {
  listProps: ComponentProps<typeof ProblemListSheet>
  previousId: string | null
  nextId: string | null
  activeMode: JudgeMode | null
  canJudge: boolean
  canStop: boolean
  onRun: (mode: JudgeMode) => void
  onStop: () => void
}

export function SolveToolbar({
  listProps,
  previousId,
  nextId,
  activeMode,
  canJudge,
  canStop,
  onRun,
  onStop,
}: SolveToolbarProps) {
  const busy = activeMode !== null
  const blockedReason = canJudge ? undefined : "An installed problem pack is required"

  return (
    <div className="flex min-h-[52px] flex-wrap items-center gap-3 px-3.5 py-2">
      <div className="flex min-w-0 flex-1 items-center gap-1.5">
        <ProblemListSheet {...listProps} />
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Previous problem"
          disabled={busy || !previousId}
          onClick={() => previousId && listProps.onSelectProblem(previousId)}
        >
          <ChevronLeft />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Next problem"
          disabled={busy || !nextId}
          onClick={() => nextId && listProps.onSelectProblem(nextId)}
        >
          <ChevronRight />
        </Button>
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          className="h-[34px] px-3.5 shadow-[0_1px_2px_rgba(16,16,24,0.05)]"
          onClick={() => onRun("run")}
          disabled={busy || !canJudge}
          title={blockedReason ?? "Run on the public sample (Ctrl Enter)"}
        >
          {activeMode === "run" ? <LoaderCircle className="animate-spin" /> : <Play />}
          Run
          <kbd className="font-mono text-[10.5px] opacity-70">Ctrl ↵</kbd>
        </Button>
        <Button
          className="h-[34px] px-3.5"
          onClick={() => onRun("submit")}
          disabled={busy || !canJudge}
          title={blockedReason ?? "Submit for judging (Ctrl Shift Enter)"}
        >
          {activeMode === "submit" ? <LoaderCircle className="animate-spin" /> : <Send />}
          Submit
        </Button>
      </div>
      <div className="flex flex-1 items-center">
        {canStop ? (
          <Button variant="ghost" size="sm" onClick={onStop}>
            <Square />
            Stop
          </Button>
        ) : null}
      </div>
    </div>
  )
}
