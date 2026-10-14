import { problemHref } from "@/app-route-state"
import type {
  InstalledPack,
  JudgeVerdict,
  ProblemDifficulty,
  SubmissionHistorySummary,
} from "@/lib/judge-api"

import { catalogProblems } from "./catalog"
import { historyProblemKey } from "./problem-status"
import { isJudgeVerdict, verdictLabel, verdictTone, type VerdictTone } from "./verdict-display"

export type HistoryTab = "all" | "submit" | "run"
export type HistoryRange = "1d" | "7d" | "30d" | "all"
export type HistoryFilters = {
  tab: HistoryTab
  query: string
  verdict: JudgeVerdict | "all"
  range: HistoryRange
}

export type HistoryEntry = {
  id: string
  record: SubmissionHistorySummary
  /** Unix seconds, or null when the stored value is not a timestamp. */
  createdAt: number | null
  verdict: JudgeVerdict | null
  title: string
  packName: string
  packLabel: string
  problemHref: string | null
  difficulty: ProblemDifficulty | null
  passed: number
  total: number
  elapsedMs: number | null
}

const DAY = 86400
const rangeSeconds: Record<Exclude<HistoryRange, "all">, number> = {
  "1d": DAY,
  "7d": 7 * DAY,
  "30d": 30 * DAY,
}

function parseCreatedAt(value: string) {
  return /^\d+$/.test(value) ? Number(value) : null
}

export function historyEntries(
  records: SubmissionHistorySummary[],
  packs: InstalledPack[]
): HistoryEntry[] {
  const problems = new Map(catalogProblems(packs).map((problem) => [problem.id, problem]))

  return records
    .map((record) => {
      const key = historyProblemKey(record)
      const problem = key ? problems.get(key) : undefined
      const pack = packs.find(
        (candidate) =>
          candidate.packId === record.packId &&
          candidate.version === record.packVersion &&
          candidate.digest === record.packHash
      )
      const tests = record.tests ?? []
      return {
        id: record.submissionId,
        record,
        createdAt: parseCreatedAt(record.createdAt),
        verdict: isJudgeVerdict(record.verdict) ? record.verdict : null,
        title: problem?.title ?? "Unknown problem",
        packName: pack?.title ?? record.packId,
        packLabel: `${pack?.title ?? record.packId} ${record.packVersion}`,
        problemHref: key && problem ? problemHref(key) : null,
        difficulty: problem?.difficulty ?? null,
        passed: tests.filter((test) => test.verdict === "Accepted").length,
        total: tests.length,
        elapsedMs: tests.length > 0 ? Math.max(...tests.map((test) => test.elapsedMs)) : null,
      }
    })
    .sort((a, b) => (b.createdAt ?? -Infinity) - (a.createdAt ?? -Infinity))
}

export type HistoryChipTone = VerdictTone | "run"

export function historyChip(entry: Pick<HistoryEntry, "verdict" | "record">): {
  label: string
  tone: HistoryChipTone
} {
  if (!entry.verdict) {
    return { label: "Unknown verdict", tone: "warn" }
  }
  if (entry.record.mode === "run" && entry.verdict === "Accepted") {
    return { label: "Sample run", tone: "run" }
  }
  const source = { status: "completed" as const, verdict: entry.verdict }
  return { label: verdictLabel(source), tone: verdictTone(source) }
}

export function filterHistory(entries: HistoryEntry[], filters: HistoryFilters, now: number) {
  const query = filters.query.trim().toLowerCase()
  const since = filters.range === "all" ? null : now - rangeSeconds[filters.range]

  return entries.filter(
    (entry) =>
      (filters.tab === "all" || entry.record.mode === filters.tab) &&
      (filters.verdict === "all" || entry.verdict === filters.verdict) &&
      (since === null || (entry.createdAt !== null && entry.createdAt >= since)) &&
      (query.length === 0 ||
        `${entry.title} ${entry.packLabel}`.toLowerCase().includes(query))
  )
}
