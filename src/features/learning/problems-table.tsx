import { Check } from "lucide-react"

import { problemHref } from "@/app-route-state"
import { useAppSettings } from "@/lib/app-settings"
import { cn } from "@/lib/utils"

import type { LearningProblem, ProblemDifficulty } from "./catalog"
import { relativeTime, type ProblemStatus } from "./problem-status"
import { isJudgeVerdict, verdictLabel } from "./verdict-display"

const difficultyText: Record<ProblemDifficulty, string> = {
  easy: "text-ok",
  medium: "text-warn",
  hard: "text-err",
}

type ProblemsTableProps = {
  problems: LearningProblem[]
  packTitles: Map<string, string>
  statuses: Map<string, ProblemStatus>
}

export function ProblemsTable({ problems, packTitles, statuses }: ProblemsTableProps) {
  const { compactTables } = useAppSettings()

  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr className="text-left text-[13px] text-muted-foreground [&_th]:border-b [&_th]:px-4 [&_th]:py-2.5 [&_th]:font-medium [&_th]:whitespace-nowrap">
            <th className="w-14">Status</th>
            <th>Title</th>
            <th>Topics</th>
            <th className="w-[100px]">Difficulty</th>
            <th className="w-[150px]">Last attempt</th>
          </tr>
        </thead>
        <tbody className={cn("[&_td]:border-b [&_td]:px-4 [&_td]:align-middle [&_tr:last-child_td]:border-b-0 [&_tr:nth-child(even)_td]:bg-muted/40 [&_tr:hover_td]:bg-accent", compactTables ? "[&_td]:h-9" : "[&_td]:h-12")}>
          {problems.map((problem) => {
            const status = statuses.get(problem.id)
            return (
              <tr key={problem.id}>
                <td><StatusIcon status={status?.status} /></td>
                <td className="min-w-[260px] py-1.5">
                  <a href={problemHref(problem.id)} className="font-medium hover:underline hover:underline-offset-[3px]">
                    {problem.title}
                  </a>
                  <div className="text-xs text-muted-foreground">{packTitles.get(problem.packId) ?? problem.packId}</div>
                </td>
                <td className="text-[13px] text-muted-foreground">{problem.topics.join(" · ")}</td>
                <td className={cn("font-medium capitalize", difficultyText[problem.difficulty])}>{problem.difficulty}</td>
                <td className="text-[13px] text-muted-foreground">
                  {status ? `${lastVerdictLabel(status.lastVerdict)} · ${relativeTime(status.lastAt)}` : "—"}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function lastVerdictLabel(verdict: string) {
  return isJudgeVerdict(verdict)
    ? verdictLabel({ status: "completed", verdict })
    : "Attempted"
}

function StatusIcon({ status }: { status?: ProblemStatus["status"] }) {
  if (status === "solved") {
    return <Check className="size-4 text-ok" role="img" aria-label="Solved" />
  }
  if (status === "attempted") {
    return (
      <svg viewBox="0 0 24 24" className="size-4 text-warn" role="img" aria-label="Attempted">
        <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2" />
        <path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor" />
      </svg>
    )
  }
  return <span className="sr-only">Not started</span>
}
