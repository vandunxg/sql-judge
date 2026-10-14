import { describe, expect, test } from "bun:test"

import {
  activeSectionId,
  fontSizeFromInput,
  FONT_SIZE_RANGE,
  settingsSections,
} from "./settings-state"

const sections = [
  { id: "appearance", top: 0 },
  { id: "editor", top: 240 },
  { id: "judge", top: 520 },
  { id: "about", top: 900 },
]

describe("settings section navigation", () => {
  test("lists the sections in design order", () => {
    expect(settingsSections.map((section) => section.id)).toEqual([
      "appearance",
      "editor",
      "judge",
      "backup",
      "privacy",
      "about",
    ])
  })

  test("highlights the last section whose heading passed the threshold", () => {
    expect(activeSectionId(sections, { offset: 0, threshold: 80, atEnd: false })).toBe("appearance")
    expect(activeSectionId(sections, { offset: 300, threshold: 80, atEnd: false })).toBe("editor")
    expect(activeSectionId(sections, { offset: 450, threshold: 80, atEnd: false })).toBe("judge")
  })

  test("highlights the final section once the page cannot scroll further", () => {
    expect(activeSectionId(sections, { offset: 600, threshold: 80, atEnd: true })).toBe("about")
  })

  test("falls back to the first section before any heading is reached", () => {
    expect(activeSectionId([{ id: "editor", top: 400 }], { offset: 0, threshold: 80, atEnd: false })).toBe(
      "editor"
    )
  })
})

describe("editor font size input", () => {
  test("uses the store's readable range", () => {
    expect(FONT_SIZE_RANGE).toEqual({ min: 11, max: 22 })
  })

  test("commits whole sizes inside the range", () => {
    expect(fontSizeFromInput("16")).toBe(16)
    expect(fontSizeFromInput(" 11 ")).toBe(11)
    expect(fontSizeFromInput("22")).toBe(22)
  })

  test("waits on partial, fractional, empty or out-of-range input", () => {
    expect(fontSizeFromInput("1")).toBeNull()
    expect(fontSizeFromInput("")).toBeNull()
    expect(fontSizeFromInput("14.5")).toBeNull()
    expect(fontSizeFromInput("99")).toBeNull()
    expect(fontSizeFromInput("abc")).toBeNull()
  })
})
