import { describe, expect, test } from "bun:test"

import { adjacentProblems, resultSummary, verdictPresentation } from "./solve-state"

describe("solve state", () => {
  test("finds neighbouring problems in catalog order", () => {
    const problems = [{ id: "a" }, { id: "b" }, { id: "c" }]
    expect(adjacentProblems(problems, "b")).toEqual({ previous: { id: "a" }, next: { id: "c" }, position: 2 })
    expect(adjacentProblems(problems, "a")).toEqual({ previous: undefined, next: { id: "b" }, position: 1 })
    expect(adjacentProblems(problems, "missing")).toEqual({ previous: undefined, next: undefined, position: 0 })
  })

  test("summarises completed rows, columns and duration", () => {
    expect(
      resultSummary({ status: "completed", durationMs: 21, message: "", columns: [{ name: "a", type: "text" }, { name: "b", type: "int" }], rows: [["x", 1], ["y", 2], ["z", 3], ["w", null]] })
    ).toBe("4 rows · 2 columns · 21 ms")
    expect(resultSummary({ status: "completed", durationMs: 3, message: "", columns: [{ name: "a", type: "text" }], rows: [["x"]] })).toBe("1 row · 1 column · 3 ms")
    expect(resultSummary({ status: "error", durationMs: 0, message: "boom" })).toBe("0 ms")
  })

  test("maps every verdict to a text label and non-color tone", () => {
    expect(verdictPresentation({ status: "completed", verdict: "Accepted" })).toEqual({ label: "Accepted", tone: "ok" })
    expect(verdictPresentation({ status: "completed", verdict: "WrongAnswer" })).toEqual({ label: "Wrong answer", tone: "err" })
    expect(verdictPresentation({ status: "completed", verdict: "TimeLimitExceeded" })).toEqual({ label: "Time limit exceeded", tone: "warn" })
    expect(verdictPresentation({ status: "completed", verdict: "Cancelled" })).toEqual({ label: "Cancelled", tone: "muted" })
    expect(verdictPresentation({ status: "completed", verdict: "JudgeError" })).toEqual({ label: "Judge error", tone: "warn" })
    expect(verdictPresentation({ status: "unavailable" })).toEqual({ label: "Runner unavailable", tone: "warn" })
    expect(verdictPresentation({ status: "error" })).toEqual({ label: "Judge error", tone: "warn" })
  })
})

describe("problem environment", () => {
  test("picks the managed environment pinned to the exact pack identity, preferring ready ones", async () => {
    const { environmentForProblem } = await import("./solve-state")
    const problem = { packId: "hr", packVersion: "1.2.0", packDigest: "d1" }
    const env = (namespace, state, extra = {}) => ({ envId: namespace, instanceId: "i", packId: "hr", packVersion: "1.2.0", packDigest: "d1", namespace, mode: "managed", state, ...extra })
    expect(environmentForProblem([env("sj_old", "error"), env("sj_hr", "ready")], problem)).toEqual({ namespace: "sj_hr", ready: true })
    expect(environmentForProblem([env("sj_busy", "rebuilding")], problem)).toEqual({ namespace: "sj_busy", ready: false })
    expect(environmentForProblem([env("sj_other", "ready", { packDigest: "d2" })], problem)).toBeUndefined()
    expect(environmentForProblem([env("sj_ext", "ready", { mode: "external_explorer" })], problem)).toBeUndefined()
  })

  test("prefers the pack sandbox that Run uses", async () => {
    const { environmentForProblem, packSandboxEnvId } = await import("./solve-state")
    const digest = "0123456789abcdef" + "f".repeat(48)
    const problem = { packId: "hr", packVersion: "1.2.0", packDigest: digest }
    const env = (envId, namespace, state) => ({ envId, instanceId: "i", packId: "hr", packVersion: "1.2.0", packDigest: digest, namespace, mode: "managed", state })
    expect(packSandboxEnvId(digest)).toBe("pack-0123456789abcdef")
    expect(environmentForProblem([env("mine", "sj_mine", "ready"), env("pack-0123456789abcdef", "sj_pack_0123456789abcdef", "ready")], problem)).toEqual({ namespace: "sj_pack_0123456789abcdef", ready: true })
    expect(environmentForProblem([env("mine", "sj_mine", "ready"), env("pack-0123456789abcdef", "sj_pack_0123456789abcdef", "needs_recovery")], problem)).toEqual({ namespace: "sj_pack_0123456789abcdef", ready: false })
  })
})

describe("sample column order", () => {
  test("orders sample columns by the declared schema, then by first appearance", async () => {
    const { orderedColumns } = await import("./solve-state")
    const rows = [{ department_id: 1, id: 1, name: "Linh", salary: 1 }, { extra: true, id: 2 }]
    expect(orderedColumns(rows, ["id", "name", "department_id", "salary", "unused"])).toEqual(["id", "name", "department_id", "salary", "extra"])
    expect(orderedColumns(rows, [])).toEqual(["department_id", "id", "name", "salary", "extra"])
    expect(orderedColumns([], ["id"])).toEqual([])
  })

  test("describes how rows are compared", async () => {
    const { rowOrderNote, limitLabel } = await import("./solve-state")
    expect(rowOrderNote("unordered")).toBe("Any row order is accepted. Rows are compared as a multiset, so duplicate rows count.")
    expect(rowOrderNote("ordered")).toBe("Row order matters. Rows are compared in order, and duplicate rows count.")
    expect(limitLabel(2000)).toBe("2 s limit")
    expect(limitLabel(1500)).toBe("1.5 s limit")
    expect(limitLabel(250)).toBe("250 ms limit")
  })
})

describe("fixtures", () => {
  test("summarises passed fixtures and picks the first failing one", async () => {
    const { fixtureSummary, initialFixture, fixtureName } = await import("./solve-state")
    const tests = [
      { id: "basic", verdict: "Accepted", elapsedMs: 4, hidden: false },
      { id: "salary-ties", verdict: "WrongAnswer", elapsedMs: 9, hidden: true },
      { id: "null-salaries", verdict: "Accepted", elapsedMs: 5, hidden: true },
    ]
    expect(fixtureSummary(tests)).toBe("2 of 3 fixtures passed")
    expect(fixtureSummary([tests[0]])).toBe("1 of 1 fixture passed")
    expect(fixtureSummary([])).toBeUndefined()
    expect(initialFixture(tests)).toBe(1)
    expect(initialFixture([tests[0]])).toBe(0)
    expect(fixtureName("salary-ties")).toBe("salary ties")
  })
})
