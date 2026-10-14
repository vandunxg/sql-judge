import { filterProblems, type LearningProblem, type ProblemDifficulty } from "./catalog"
import type { ProblemStatus } from "./problem-status"

export type StatusFilter = "all" | "solved" | "attempted" | "todo"

export type CatalogFilters = {
  query: string
  topic: string
  packId: string
  difficulty: ProblemDifficulty | "all"
  status: StatusFilter
}

const PAGE_WINDOW = 5

export function topicCounts(problems: Pick<LearningProblem, "topics">[]) {
  const counts = new Map<string, number>()
  for (const topic of problems.flatMap((problem) => problem.topics)) {
    counts.set(topic, (counts.get(topic) ?? 0) + 1)
  }
  return [...counts].map(([topic, count]) => ({ topic, count }))
}

export function filterCatalog(
  problems: LearningProblem[],
  filters: CatalogFilters,
  statuses: Map<string, Pick<ProblemStatus, "status">>
) {
  return filterProblems(problems, filters.query, "all", filters.difficulty).filter(
    (problem) =>
      (filters.topic === "all" || problem.topics.includes(filters.topic)) &&
      (filters.packId === "all" || problem.packId === filters.packId) &&
      (filters.status === "all" ||
        (statuses.get(problem.id)?.status ?? "todo") === filters.status)
  )
}

export function pageWindow(page: number, totalPages: number) {
  const size = Math.min(PAGE_WINDOW, totalPages)
  const start = Math.min(Math.max(0, page - Math.floor(PAGE_WINDOW / 2)), totalPages - size)
  return Array.from({ length: size }, (_, index) => start + index)
}
