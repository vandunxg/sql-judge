import type { SubmissionHistorySummary } from "@/lib/judge-api"

import { catalogProblemKey, type ProblemDifficulty } from "./catalog"

export type ProblemStatus = {
  status: "solved" | "attempted"
  lastVerdict: string
  lastAt: number
}

export function historyProblemKey(
  record: Pick<SubmissionHistorySummary, "packId" | "packVersion" | "packHash" | "problemId">
) {
  return record.problemId
    ? catalogProblemKey(
        { packId: record.packId, version: record.packVersion, digest: record.packHash },
        record.problemId
      )
    : null
}

export function problemStatuses(records: SubmissionHistorySummary[]) {
  const statuses = new Map<string, ProblemStatus>()
  for (const record of records) {
    const key = historyProblemKey(record)
    if (!key || record.mode !== "submit") {
      continue
    }
    const at = Number(record.createdAt)
    const previous = statuses.get(key)
    const solved = previous?.status === "solved" || record.verdict === "Accepted"
    const newer = !previous || at >= previous.lastAt
    statuses.set(key, {
      status: solved ? "solved" : "attempted",
      lastVerdict: newer ? record.verdict : previous.lastVerdict,
      lastAt: newer ? at : previous.lastAt,
    })
  }
  return statuses
}

export function difficultyProgress(
  problems: Array<{ id: string; difficulty: ProblemDifficulty }>,
  statuses: Map<string, Pick<ProblemStatus, "status">>
) {
  const progress = {
    easy: { solved: 0, total: 0 },
    medium: { solved: 0, total: 0 },
    hard: { solved: 0, total: 0 },
  }
  for (const problem of problems) {
    progress[problem.difficulty].total += 1
    if (statuses.get(problem.id)?.status === "solved") {
      progress[problem.difficulty].solved += 1
    }
  }
  return progress
}

const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

export function relativeTime(seconds: number, now = Date.now() / 1000) {
  const elapsed = Math.max(0, now - seconds)
  if (elapsed < MINUTE) return "just now"
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)} min ago`
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)} h ago`
  if (elapsed < 2 * DAY) return "yesterday"
  return `${Math.floor(elapsed / DAY)} days ago`
}

export function resumeProblemId(
  statuses: Map<string, Pick<ProblemStatus, "status" | "lastAt">>
) {
  let latest: [string, number] | null = null
  for (const [key, { status, lastAt }] of statuses) {
    if (status === "attempted" && (!latest || lastAt > latest[1])) {
      latest = [key, lastAt]
    }
  }
  return latest?.[0] ?? null
}
