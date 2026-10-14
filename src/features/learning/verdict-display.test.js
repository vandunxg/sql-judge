import { describe, expect, test } from "bun:test"

import { isJudgeVerdict, verdictLabel, verdictTone } from "./verdict-display"

describe("verdict display", () => {
  test("uses readable verdict labels", () => {
    expect(verdictLabel({ status: "completed", verdict: "WrongAnswer" })).toBe("Wrong answer")
    expect(verdictLabel({ status: "completed", verdict: "TimeLimitExceeded" })).toBe("Time limit exceeded")
    expect(verdictLabel({ status: "unavailable" })).toBe("Runner unavailable")
  })

  test("separates accepted, user-caused and infrastructure outcomes", () => {
    expect(verdictTone({ status: "completed", verdict: "Accepted" })).toBe("ok")
    expect(verdictTone({ status: "completed", verdict: "WrongAnswer" })).toBe("err")
    expect(verdictTone({ status: "completed", verdict: "PolicyViolation" })).toBe("warn")
    expect(verdictTone({ status: "completed", verdict: "Cancelled" })).toBe("warn")
    expect(verdictTone({ status: "completed", verdict: "JudgeError" })).toBe("warn")
    expect(verdictTone({ status: "error" })).toBe("warn")
  })
})

test("recognises only native verdict names", () => {
  expect(isJudgeVerdict("WrongAnswer")).toBe(true)
  expect(isJudgeVerdict("toString")).toBe(false)
  expect(isJudgeVerdict("Wrong answer")).toBe(false)
})
