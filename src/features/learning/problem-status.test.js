import { describe, expect, test } from "bun:test"

import {
  difficultyProgress,
  historyProblemKey,
  problemStatuses,
  relativeTime,
  resumeProblemId,
} from "./problem-status"

const record = (overrides) => ({
  submissionId: "s",
  packId: "hr",
  packVersion: "1.0.0",
  packHash: "abc",
  engineVersion: "pg",
  policyVersion: "p1",
  verdict: "WrongAnswer",
  createdAt: "100",
  problemId: "p1",
  mode: "submit",
  tests: [],
  ...overrides,
})

describe("problem status", () => {
  test("joins history records to catalog problem keys", () => {
    expect(historyProblemKey(record({}))).toBe("hr@1.0.0#abc:p1")
    expect(historyProblemKey(record({ problemId: null }))).toBeNull()
  })

  test("marks a problem solved once any submission is accepted", () => {
    const statuses = problemStatuses([
      record({ verdict: "Accepted", createdAt: "100" }),
      record({ verdict: "WrongAnswer", createdAt: "200" }),
    ])
    expect(statuses.get("hr@1.0.0#abc:p1")).toEqual({
      status: "solved",
      lastVerdict: "WrongAnswer",
      lastAt: 200,
    })
  })

  test("ignores sample runs and records without a problem", () => {
    const statuses = problemStatuses([
      record({ mode: "run", verdict: "Accepted" }),
      record({ problemId: null, verdict: "Accepted" }),
      record({ problemId: "p2", verdict: "SyntaxError" }),
    ])
    expect([...statuses.keys()]).toEqual(["hr@1.0.0#abc:p2"])
    expect(statuses.get("hr@1.0.0#abc:p2").status).toBe("attempted")
  })

  test("counts solved problems per difficulty", () => {
    const problems = [
      { id: "a", difficulty: "easy" },
      { id: "b", difficulty: "easy" },
      { id: "c", difficulty: "hard" },
    ]
    const statuses = new Map([
      ["a", { status: "solved" }],
      ["c", { status: "attempted" }],
    ])
    expect(difficultyProgress(problems, statuses)).toEqual({
      easy: { solved: 1, total: 2 },
      medium: { solved: 0, total: 0 },
      hard: { solved: 0, total: 1 },
    })
  })

  test("formats unix-second timestamps relative to now", () => {
    const now = 1_000_000
    expect(relativeTime(now - 20, now)).toBe("just now")
    expect(relativeTime(now - 12 * 60, now)).toBe("12 min ago")
    expect(relativeTime(now - 3 * 3600, now)).toBe("3 h ago")
    expect(relativeTime(now - 30 * 3600, now)).toBe("yesterday")
    expect(relativeTime(now - 3 * 86400, now)).toBe("3 days ago")
  })
})

test("resumes the most recently attempted unsolved problem", () => {
  const statuses = new Map([
    ["old", { status: "attempted", lastAt: 100 }],
    ["recent", { status: "attempted", lastAt: 300 }],
    ["solved", { status: "solved", lastAt: 500 }],
  ])
  expect(resumeProblemId(statuses)).toBe("recent")
  expect(resumeProblemId(new Map([["solved", { status: "solved", lastAt: 1 }]]))).toBeNull()
})
