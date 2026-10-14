import { expect, test } from "bun:test"

import { defaultSettings, parseSettings } from "./app-settings"

test("uses defaults for missing or malformed storage", () => {
  expect(parseSettings(null)).toEqual(defaultSettings)
  expect(parseSettings("{not json")).toEqual(defaultSettings)
  expect(parseSettings('"text"')).toEqual(defaultSettings)
})

test("keeps valid values and drops unknown or mistyped fields", () => {
  expect(
    parseSettings('{"compactTables":true,"keepDrafts":"yes","extra":1}')
  ).toEqual({ ...defaultSettings, compactTables: true })
})

test("clamps the editor font size to a readable range", () => {
  expect(parseSettings('{"editorFontSize":4}').editorFontSize).toBe(11)
  expect(parseSettings('{"editorFontSize":99}').editorFontSize).toBe(22)
  expect(parseSettings('{"editorFontSize":15.6}').editorFontSize).toBe(16)
})
