import { describe, expect, test } from "bun:test"

import { formatDuration, formatTimestamp, shortHash, whenLabel } from "./history-format"

const at = (...parts) => new Date(...parts).getTime() / 1000

describe("history formatting", () => {
  test("formats fixture time in ms or seconds", () => {
    expect(formatDuration(38)).toBe("38 ms")
    expect(formatDuration(999)).toBe("999 ms")
    expect(formatDuration(2000)).toBe("2.0 s")
    expect(formatDuration(null)).toBe("—")
  })

  test("abbreviates long pack hashes and keeps short ones", () => {
    expect(shortHash("sha256:9f2c00000000000000a71e")).toBe("sha256:9f2c…a71e")
    expect(shortHash("0123456789abcdef0123")).toBe("0123…0123")
    expect(shortHash("abc")).toBe("abc")
  })

  test("formats submitted time relative to the local calendar day", () => {
    const now = at(2026, 9, 14, 22, 0)
    expect(formatTimestamp(at(2026, 9, 14, 21, 14), now)).toBe("Today, 21:14")
    expect(formatTimestamp(at(2026, 9, 13, 9, 5), now)).toBe("Yesterday, 09:05")
    expect(formatTimestamp(at(2026, 9, 2, 21, 14), now)).toBe("Oct 2, 21:14")
    expect(formatTimestamp(at(2025, 11, 31, 8, 0), now)).toBe("Dec 31, 2025, 08:00")
    expect(formatTimestamp(null, now)).toBe("Unknown time")
  })

  test("capitalizes the shared relative time label", () => {
    const now = 10000
    expect(whenLabel(now - 30, now)).toBe("Just now")
    expect(whenLabel(now - 720, now)).toBe("12 min ago")
    expect(whenLabel(null, now)).toBe("Unknown time")
  })
})
