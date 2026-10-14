import { cn } from "@/lib/utils"

import type { ProblemDifficulty } from "./catalog"
import { historyCardClass } from "./history-ui"
import type { ProgressSummary } from "./progress-state"

const CIRCUMFERENCE = 2 * Math.PI * 15.5
const SEGMENT_GAP = 1.2

export const difficultyText: Record<ProblemDifficulty, string> = {
  easy: "text-ok",
  medium: "text-warn",
  hard: "text-err",
}
const difficultyStroke: Record<ProblemDifficulty, string> = {
  easy: "stroke-ok",
  medium: "stroke-warn",
  hard: "stroke-err",
}
const difficultyFill: Record<ProblemDifficulty, string> = {
  easy: "bg-ok",
  medium: "bg-warn",
  hard: "bg-err",
}
const difficulties: ProblemDifficulty[] = ["easy", "medium", "hard"]

export const progressTitleClass = "text-[15px] font-semibold"

export function SolvedCard({ summary }: { summary: ProgressSummary }) {
  const lengths = difficulties
    .map((difficulty) => ({
      difficulty,
      length: summary.total === 0 ? 0 : (summary.byDifficulty[difficulty].solved / summary.total) * CIRCUMFERENCE,
    }))
    .filter((segment) => segment.length > 0)
  const segments = lengths.map((segment, index) => ({
    ...segment,
    offset: lengths.slice(0, index).reduce((sum, previous) => sum + previous.length + SEGMENT_GAP, 0),
  }))

  return (
    <section aria-labelledby="solved-h" className={cn(historyCardClass, "flex flex-[1_1_420px] flex-col gap-4 px-6 py-5")}>
      <h2 id="solved-h" className={progressTitleClass}>Solved</h2>
      <div className="flex flex-wrap items-center gap-7">
        <div className="relative size-[120px] shrink-0">
          <svg viewBox="0 0 36 36" className="size-[120px] -rotate-90" role="img" aria-label={`${summary.solved} of ${summary.total} problems solved`}>
            <circle cx="18" cy="18" r="15.5" fill="none" strokeWidth="2.6" className="stroke-muted" />
            {segments.map((segment) => (
              <circle
                key={segment.difficulty}
                cx="18"
                cy="18"
                r="15.5"
                fill="none"
                strokeWidth="2.6"
                strokeLinecap="round"
                strokeDasharray={`${segment.length} ${CIRCUMFERENCE}`}
                strokeDashoffset={-segment.offset}
                className={difficultyStroke[segment.difficulty]}
              />
            ))}
          </svg>
          <div className="absolute inset-0 flex flex-col items-center justify-center leading-[1.1]" aria-hidden="true">
            <span className="text-[28px] font-semibold tracking-[-0.02em]">{summary.solved}</span>
            <span className="text-xs text-muted-foreground">of {summary.total} solved</span>
          </div>
        </div>
        <div className="flex flex-[1_1_200px] flex-col gap-3.5 text-[13px]">
          {difficulties.map((difficulty) => {
            const { solved, total } = summary.byDifficulty[difficulty]
            return (
              <div key={difficulty} className="flex flex-col gap-[5px]">
                <div className="flex justify-between">
                  <span className={cn("font-medium capitalize", difficultyText[difficulty])}>{difficulty}</span>
                  <span>
                    <b className="font-semibold">{solved}</b>
                    <span className="text-muted-foreground"> / {total}</span>
                  </span>
                </div>
                <ProgressBar value={solved} max={total} className={difficultyFill[difficulty]} />
              </div>
            )
          })}
        </div>
      </div>
    </section>
  )
}

export function ProgressBar({ value, max, className, minPercent = 0 }: { value: number; max: number; className: string; minPercent?: number }) {
  const percent = max === 0 ? 0 : Math.max(minPercent, Math.round((value / max) * 100))
  return (
    <div className="h-1.5 overflow-hidden rounded-full bg-muted">
      <span className={cn("block h-full rounded-full", className)} style={{ width: `${percent}%` }} />
    </div>
  )
}

type Stat = { label: string; value: string | number; caption: string; className: string }

export function TotalsCard({ stats }: { stats: Stat[] }) {
  return (
    <section aria-label="Totals" className={cn(historyCardClass, "grid flex-[1_1_420px] overflow-hidden sm:grid-cols-2")}>
      {stats.map((stat) => (
        <div key={stat.label} className={cn("flex flex-col gap-1 px-5 py-[18px]", stat.className)}>
          <span className="text-[13px] text-muted-foreground">{stat.label}</span>
          <b className="text-[26px] leading-[1.15] font-semibold tracking-[-0.02em]">{stat.value}</b>
          <span className="text-[12.5px] text-muted-foreground">{stat.caption}</span>
        </div>
      ))}
    </section>
  )
}
