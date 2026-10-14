import { describe, expect, test } from "bun:test"

import { lastAttemptLabel, problemProgress, progressTotals, relativeTime, submittedAtMs, matchesStatus, submissionsOnly } from "./problem-progress"

const NOW = Date.parse("2026-10-14T12:00:00Z")
const sub = (problemId, verdict, iso, extra = {}) => ({ packId: "hr", packVersion: "1.2.0", packHash: "d1", problemId, verdict, createdAt: iso, ...extra })
const key = (packId, version, digest, problemId) => `${packId}@${version}#${digest}:${problemId}`

describe("problem progress", () => {
  test("parses unix-second and ISO timestamps", () => {
    expect(submittedAtMs("1791945199")).toBe(1791945199000)
    expect(submittedAtMs("2026-10-12T00:00:00Z")).toBe(Date.parse("2026-10-12T00:00:00Z"))
    expect(submittedAtMs("garbage")).toBeUndefined()
  })

  test("formats relative time like the artboard", () => {
    expect(relativeTime(NOW - 20_000, NOW)).toBe("just now")
    expect(relativeTime(NOW - 12 * 60_000, NOW)).toBe("12 min ago")
    expect(relativeTime(NOW - 5 * 3_600_000, NOW)).toBe("5 h ago")
    expect(relativeTime(NOW - 30 * 3_600_000, NOW)).toBe("yesterday")
    expect(relativeTime(NOW - 3 * 86_400_000, NOW)).toBe("3 days ago")
    expect(relativeTime(NOW - 9 * 86_400_000, NOW)).toBe("last week")
    expect(relativeTime(NOW - 20 * 86_400_000, NOW)).toBe("2 weeks ago")
    expect(relativeTime(NOW - 90 * 86_400_000, NOW)).toBe("3 months ago")
  })

  test("derives solved, attempted and the latest attempt per exact problem identity", () => {
    const history = [
      sub("a", "WrongAnswer", "2026-10-14T11:00:00Z"),
      sub("a", "Accepted", "2026-10-12T12:00:00Z"),
      sub("b", "TimeLimitExceeded", "2026-10-13T10:00:00Z"),
      sub("b", "WrongAnswer", "2026-10-14T11:48:00Z"),
      sub(null, "Accepted", "2026-10-14T11:59:00Z"),
      sub("c", "Accepted", "2026-10-14T11:59:00Z", { packHash: "other" }),
    ]
    const progress = problemProgress(history, key)
    expect(progress.get(key("hr", "1.2.0", "d1", "a"))).toMatchObject({ status: "solved", last: { verdict: "WrongAnswer" } })
    expect(progress.get(key("hr", "1.2.0", "d1", "b"))).toMatchObject({ status: "attempted", last: { verdict: "WrongAnswer" } })
    expect(progress.get(key("hr", "1.2.0", "d1", "c"))).toBeUndefined()
    expect(progress.get(key("hr", "1.2.0", "other", "c"))?.status).toBe("solved")
    expect(progress.get(key("hr", "1.2.0", "d1", "a")).count).toBe(2)
  })

  test("keeps sample runs out of submission progress", () => {
    const history = [
      sub("a", "Accepted", "2026-10-14T11:00:00Z", { mode: "run" }),
      sub("a", "WrongAnswer", "2026-10-14T10:00:00Z", { mode: "submit" }),
      sub("b", "Accepted", "2026-10-14T09:00:00Z", { mode: "run" }),
    ]
    const submissions = submissionsOnly(history)
    expect(submissions).toHaveLength(1)
    const progress = problemProgress(submissions, key)
    expect(progress.get(key("hr", "1.2.0", "d1", "a"))).toMatchObject({ status: "attempted", count: 1 })
    expect(progress.get(key("hr", "1.2.0", "d1", "b"))).toBeUndefined()
  })

  test("labels the last attempt with a short verdict", () => {
    expect(lastAttemptLabel({ verdict: "Accepted", at: NOW - 2 * 86_400_000 }, NOW)).toBe("Accepted · 2 days ago")
    expect(lastAttemptLabel({ verdict: "TimeLimitExceeded", at: NOW - 30 * 3_600_000 }, NOW)).toBe("Timeout · yesterday")
    expect(lastAttemptLabel(undefined, NOW)).toBe("—")
  })

  test("totals solved problems by difficulty and filters by status", () => {
    const problems = [{ id: "x", difficulty: "easy" }, { id: "y", difficulty: "easy" }, { id: "z", difficulty: "hard" }]
    const progress = new Map([["x", { status: "solved" }], ["z", { status: "attempted" }]])
    expect(progressTotals(problems, progress)).toEqual({ solved: 1, total: 3, easy: { solved: 1, total: 2 }, medium: { solved: 0, total: 0 }, hard: { solved: 0, total: 1 } })
    expect(problems.filter((p) => matchesStatus(progress.get(p.id), "unattempted")).map((p) => p.id)).toEqual(["y"])
    expect(problems.filter((p) => matchesStatus(progress.get(p.id), "attempted")).map((p) => p.id)).toEqual(["z"])
    expect(problems.filter((p) => matchesStatus(progress.get(p.id), "all"))).toHaveLength(3)
  })
})
