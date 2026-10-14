import { describe, expect, test } from "bun:test"

import { historyEntries } from "./history-state"
import { progressSummary } from "./progress-state"

const problem = (id, difficulty, topics) => ({
  id, title: `Problem ${id}`, difficulty, dialect: "postgresql", topics, statement: "", schema: [], sampleRows: [], starterSql: null,
})

const pack = {
  packId: "hr",
  version: "1.0.0",
  title: "HR",
  dialect: "postgresql",
  digest: "d1",
  problems: [
    problem("e1", "easy", ["joins"]),
    problem("e2", "easy", ["joins", "aggregation"]),
    problem("m1", "medium", ["window"]),
    problem("m2", "medium", ["window"]),
    problem("h1", "hard", ["aggregation"]),
  ],
}

const record = (overrides) => ({
  submissionId: overrides.submissionId ?? `${overrides.problemId}-${overrides.createdAt}`,
  packId: "hr",
  packVersion: "1.0.0",
  packHash: "d1",
  engineVersion: "pg",
  policyVersion: "p1",
  verdict: "WrongAnswer",
  createdAt: "100",
  problemId: "e1",
  mode: "submit",
  tests: [],
  ...overrides,
})

const records = [
  record({ problemId: "e1", verdict: "Accepted", createdAt: "100" }),
  record({ problemId: "e1", verdict: "Accepted", createdAt: "400" }),
  record({ problemId: "e2", verdict: "WrongAnswer", createdAt: "200" }),
  record({ problemId: "m1", verdict: "Accepted", createdAt: "300" }),
  record({ problemId: "m2", verdict: "Accepted", createdAt: "350", mode: "run" }),
  record({ problemId: "gone", packId: "old", verdict: "Accepted", createdAt: "500" }),
  record({ problemId: null, verdict: "SyntaxError", createdAt: "50", mode: "run" }),
]

const summary = () => progressSummary(records, historyEntries(records, [pack]), [pack])

describe("progress summary", () => {
  test("counts distinct accepted installed problems per difficulty", () => {
    expect(summary()).toMatchObject({
      solved: 2,
      total: 5,
      byDifficulty: {
        easy: { solved: 1, total: 2 },
        medium: { solved: 1, total: 2 },
        hard: { solved: 0, total: 1 },
      },
    })
  })

  test("derives acceptance rate from submissions and counts sample runs separately", () => {
    expect(summary()).toMatchObject({ submissions: 5, acceptedSubmissions: 4, acceptanceRate: 80, sampleRuns: 2 })
    expect(progressSummary([], [], [pack]).acceptanceRate).toBe(0)
  })

  test("reports solved over total per topic, largest topics first", () => {
    const [first, ...rest] = progressSummary(records, historyEntries(records, [pack]), [
      { ...pack, problems: [...pack.problems, problem("m3", "medium", ["window"])] },
    ]).topics
    expect(first).toEqual({ name: "window", solved: 1, total: 3 })
    expect(rest.map((topic) => topic.name)).toEqual(["joins", "aggregation"])
    expect(summary().topics).toEqual([
      { name: "joins", solved: 1, total: 2 },
      { name: "aggregation", solved: 0, total: 2 },
      { name: "window", solved: 1, total: 2 },
    ])
  })

  test("suggests the first unsolved problem in the weakest topic", () => {
    expect(summary().suggestion).toEqual({
      topic: "aggregation",
      title: "Problem e2",
      href: "#solve/hr%401.0.0%23d1%3Ae2",
    })
  })

  test("lists the latest acceptance per problem, newest first", () => {
    expect(summary().recentAccepted.map((entry) => [entry.title, entry.createdAt])).toEqual([
      ["Unknown problem", 500],
      ["Problem e1", 400],
      ["Problem m1", 300],
    ])
  })
})
