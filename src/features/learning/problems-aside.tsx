import { Plus } from "lucide-react"

import type { InstalledPack } from "@/lib/judge-api"
import { cn } from "@/lib/utils"

import type { ProblemDifficulty } from "./catalog"
import type { difficultyProgress } from "./problem-status"

const RING_CIRCUMFERENCE = 2 * Math.PI * 15.5

export const problemsCardClass =
  "rounded-xl border bg-card shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

const difficultyRows: Array<{ key: ProblemDifficulty; label: string; text: string; bar: string }> = [
  { key: "easy", label: "Easy", text: "text-ok", bar: "bg-ok" },
  { key: "medium", label: "Medium", text: "text-warn", bar: "bg-warn" },
  { key: "hard", label: "Hard", text: "text-err", bar: "bg-err" },
]

type ProblemsAsideProps = {
  progress: ReturnType<typeof difficultyProgress>
  packs: InstalledPack[]
  totalProblems: number
  packId: string
  onSelectPack: (packId: string) => void
}

export function ProblemsAside({ progress, packs, totalProblems, packId, onSelectPack }: ProblemsAsideProps) {
  const solved = progress.easy.solved + progress.medium.solved + progress.hard.solved
  const ratio = totalProblems > 0 ? solved / totalProblems : 0

  return (
    <aside aria-label="Your progress" className="flex min-w-0 flex-[1_1_280px] flex-col gap-5 pt-1">
      <section className={cn(problemsCardClass, "flex flex-col gap-[18px] p-5")}>
        <h2 className="text-[15px] font-semibold">Your progress</h2>
        <div className="flex items-center gap-5">
          <div className="relative size-24 shrink-0">
            <svg viewBox="0 0 36 36" className="size-24 -rotate-90" role="img" aria-label={`${solved} of ${totalProblems} solved`}>
              <circle cx="18" cy="18" r="15.5" fill="none" className="stroke-muted" strokeWidth="3" />
              <circle
                cx="18"
                cy="18"
                r="15.5"
                fill="none"
                className="stroke-ok"
                strokeWidth="3"
                strokeLinecap="round"
                strokeDasharray={`${ratio * RING_CIRCUMFERENCE} ${RING_CIRCUMFERENCE}`}
              />
            </svg>
            <div className="absolute inset-0 flex flex-col items-center justify-center leading-[1.1]">
              <span className="text-[22px] font-semibold">{solved}</span>
              <span className="text-xs text-muted-foreground">of {totalProblems}</span>
            </div>
          </div>
          <div className="flex flex-1 flex-col gap-2.5 text-[13px]">
            {difficultyRows.map((row) => {
              const { solved: done, total } = progress[row.key]
              return (
                <div key={row.key} className="flex flex-col gap-1">
                  <div className="flex justify-between">
                    <span className={cn("font-medium", row.text)}>{row.label}</span>
                    <span className="text-muted-foreground">{done} / {total}</span>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                    <span
                      className={cn("block h-full rounded-full", row.bar)}
                      style={{ width: `${total > 0 ? (done / total) * 100 : 0}%` }}
                    />
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      </section>
      <section className={cn(problemsCardClass, "flex flex-col p-2")}>
        <h2 className="px-3 pt-3 pb-2 text-[15px] font-semibold">Packs</h2>
        <PackFilterButton label="All packs" count={totalProblems} active={packId === "all"} onClick={() => onSelectPack("all")} />
        {packs.map((pack) => (
          <PackFilterButton
            key={`${pack.packId}@${pack.version}`}
            label={pack.title}
            count={pack.problems.length}
            active={packId === pack.packId}
            onClick={() => onSelectPack(pack.packId)}
          />
        ))}
        <a href="#packs" className="mt-1 flex h-[34px] items-center gap-2.5 rounded-lg px-2.5 text-[13px] hover:bg-accent">
          <Plus className="size-4 text-muted-foreground" /> Import another pack
        </a>
      </section>
    </aside>
  )
}

function PackFilterButton({ label, count, active, onClick }: { label: string; count: number; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex h-[34px] items-center justify-between rounded-lg px-2.5 text-sm hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50",
        active && "bg-accent font-medium"
      )}
    >
      <span className="truncate">{label}</span>
      <span className="text-muted-foreground">{count}</span>
    </button>
  )
}
