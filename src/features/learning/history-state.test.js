import { describe, expect, test } from "bun:test"

import { filterHistory, historyChip, historyEntries } from "./history-state"

const pack = {
  packId: "hr",
  version: "1.2.0",
  title: "HR Analytics",
  dialect: "postgresql",
  digest: "sha256:abc",
  problems: [
    { id: "p1", title: "Second highest salary", difficulty: "medium", dialect: "postgresql", topics: ["window"], statement: "", schema: [], sampleRows: [], starterSql: null },
  ],
}

const record = (overrides) => ({
  submissionId: "s1",
  packId: "hr",
  packVersion: "1.2.0",
  packHash: "sha256:abc",
  engineVersion: "postgresql-managed",
  policyVersion: "policy-v1",
  verdict: "WrongAnswer",
  createdAt: "1000",
  problemId: "p1",
  mode: "submit",
  tests: [],
  ...overrides,
})

const DAY = 86400

describe("history entries", () => {
  test("joins installed problem title, difficulty and practice link", () => {
    const [entry] = historyEntries([record({})], [pack])
    expect(entry.title).toBe("Second highest salary")
    expect(entry.difficulty).toBe("medium")
    expect(entry.packLabel).toBe("HR Analytics 1.2.0")
    expect(entry.problemHref).toBe("#solve/hr%401.2.0%23sha256%3Aabc%3Ap1")
  })

  test("renders uninstalled or untracked problems honestly without a link", () => {
    const [missing, untracked, otherVersion] = historyEntries(
      [
        record({ submissionId: "a", createdAt: "300", packId: "old", packVersion: "0.9.0" }),
        record({ submissionId: "b", createdAt: "200", problemId: null }),
        record({ submissionId: "c", createdAt: "100", packVersion: "1.1.0" }),
      ],
      [pack]
    )
    expect(missing).toMatchObject({ title: "Unknown problem", packLabel: "old 0.9.0", problemHref: null, difficulty: null })
    expect(untracked).toMatchObject({ title: "Unknown problem", packLabel: "HR Analytics 1.2.0", problemHref: null })
    expect(otherVersion).toMatchObject({ title: "Unknown problem", packLabel: "hr 1.1.0", problemHref: null })
  })

  test("counts passed fixtures and uses the run duration stored on each fixture", () => {
    const [entry] = historyEntries(
      [record({ tests: [
        { testId: "basic", verdict: "Accepted", elapsedMs: 9 },
        { testId: "ties", verdict: "WrongAnswer", elapsedMs: 18 },
        { testId: "nulls", verdict: "Accepted", elapsedMs: 11 },
      ] })],
      [pack]
    )
    // The native judge stores the whole run duration on every fixture row (ADR-022).
    expect(entry).toMatchObject({ passed: 2, total: 3, elapsedMs: 18 })
    expect(historyEntries([record({})], [pack])[0]).toMatchObject({ passed: 0, total: 0, elapsedMs: null })
  })

  test("sorts newest first and keeps unparseable timestamps last", () => {
    const entries = historyEntries(
      [record({ submissionId: "old", createdAt: "10" }), record({ submissionId: "bad", createdAt: "2026-10-12T00:00:00Z" }), record({ submissionId: "new", createdAt: "20" })],
      [pack]
    )
    expect(entries.map((entry) => entry.id)).toEqual(["new", "old", "bad"])
    expect(entries[2].createdAt).toBeNull()
  })
})

describe("history verdicts", () => {
  test("accepts only native verdict names", () => {
    const verdict = (value) => historyEntries([record({ verdict: value })], [pack])[0].verdict
    expect(verdict("TimeLimitExceeded")).toBe("TimeLimitExceeded")
    expect(verdict("Bogus")).toBeNull()
    expect(verdict("toString")).toBeNull()
  })

  test("labels submissions, sample runs and unknown verdicts", () => {
    const chip = (overrides) => historyChip(historyEntries([record(overrides)], [pack])[0])
    expect(chip({ verdict: "WrongAnswer" })).toEqual({ label: "Wrong answer", tone: "err" })
    expect(chip({ verdict: "Accepted", mode: "run" })).toEqual({ label: "Sample run", tone: "run" })
    expect(chip({ verdict: "SyntaxError", mode: "run" })).toEqual({ label: "Syntax error", tone: "err" })
    expect(chip({ verdict: "Bogus" })).toEqual({ label: "Unknown verdict", tone: "warn" })
  })
})

describe("history filters", () => {
  const now = 100 * DAY
  const entries = historyEntries(
    [
      record({ submissionId: "recent-submit", createdAt: String(now - 60), verdict: "Accepted" }),
      record({ submissionId: "recent-run", createdAt: String(now - DAY / 2), mode: "run", verdict: "SyntaxError" }),
      record({ submissionId: "week-old", createdAt: String(now - 10 * DAY), packId: "orders", problemId: "p9" }),
      record({ submissionId: "ancient", createdAt: String(now - 60 * DAY) }),
      record({ submissionId: "undated", createdAt: "not-a-time" }),
    ],
    [pack]
  )
  const base = { tab: "all", query: "", verdict: "all", range: "all" }
  const ids = (filters) => filterHistory(entries, { ...base, ...filters }, now).map((entry) => entry.id)

  test("separates submissions from sample runs", () => {
    expect(ids({})).toHaveLength(5)
    expect(ids({ tab: "run" })).toEqual(["recent-run"])
    expect(ids({ tab: "submit" })).not.toContain("recent-run")
  })

  test("searches problem titles and pack labels case-insensitively", () => {
    expect(ids({ query: "  SECOND " })).toEqual(["recent-submit", "recent-run", "ancient", "undated"])
    expect(ids({ query: "orders" })).toEqual(["week-old"])
  })

  test("filters by verdict and time range", () => {
    expect(ids({ verdict: "Accepted" })).toEqual(["recent-submit"])
    expect(ids({ range: "1d" })).toEqual(["recent-submit", "recent-run"])
    expect(ids({ range: "30d" })).toEqual(["recent-submit", "recent-run", "week-old"])
    expect(ids({ range: "7d", tab: "submit" })).toEqual(["recent-submit"])
  })
})
