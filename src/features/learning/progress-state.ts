import { problemHref } from "@/app-route-state"
import type { InstalledPack, SubmissionHistorySummary } from "@/lib/judge-api"

import { catalogProblems } from "./catalog"
import type { HistoryEntry } from "./history-state"
import { difficultyProgress, historyProblemKey, problemStatuses } from "./problem-status"

const RECENT_LIMIT = 5

export type TopicProgress = { name: string; solved: number; total: number }

export function progressSummary(
  records: SubmissionHistorySummary[],
  entries: HistoryEntry[],
  packs: InstalledPack[]
) {
  const problems = catalogProblems(packs)
  const statuses = problemStatuses(records)
  const isSolved = (id: string) => statuses.get(id)?.status === "solved"
  const byDifficulty = difficultyProgress(problems, statuses)
  const submits = entries.filter((entry) => entry.record.mode === "submit")
  const acceptedSubmits = submits.filter((entry) => entry.verdict === "Accepted")

  const topicMap = new Map<string, TopicProgress>()
  for (const problem of problems) {
    for (const name of problem.topics) {
      const topic = topicMap.get(name) ?? { name, solved: 0, total: 0 }
      topic.total += 1
      topic.solved += isSolved(problem.id) ? 1 : 0
      topicMap.set(name, topic)
    }
  }
  const topics = [...topicMap.values()].sort((a, b) => b.total - a.total)
  const weakest = topics
    .filter((topic) => topic.solved < topic.total)
    .reduce<TopicProgress | null>(
      (best, topic) =>
        !best || topic.solved / topic.total < best.solved / best.total ? topic : best,
      null
    )
  const next = weakest
    ? problems.find((problem) => problem.topics.includes(weakest.name) && !isSolved(problem.id))
    : undefined

  const seen = new Set<string>()
  const recentAccepted = acceptedSubmits.filter((entry) => {
    const key = historyProblemKey(entry.record) ?? entry.id
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })

  return {
    solved: problems.filter((problem) => isSolved(problem.id)).length,
    total: problems.length,
    byDifficulty,
    submissions: submits.length,
    acceptedSubmissions: acceptedSubmits.length,
    acceptanceRate:
      submits.length === 0 ? 0 : Math.round((acceptedSubmits.length / submits.length) * 100),
    sampleRuns: entries.length - submits.length,
    topics,
    suggestion:
      weakest && next ? { topic: weakest.name, title: next.title, href: problemHref(next.id) } : null,
    recentAccepted: recentAccepted.slice(0, RECENT_LIMIT),
  }
}

export type ProgressSummary = ReturnType<typeof progressSummary>
