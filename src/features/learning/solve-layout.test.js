import { expect, test } from "bun:test"

import { resizeResultSplit, resizeSplit } from "./solve-layout"

test("keeps a resizable split inside its minimum and maximum bounds", () => {
  expect(resizeSplit(40, -30, 25, 60)).toBe(25)
  expect(resizeSplit(40, 15, 25, 60)).toBe(55)
  expect(resizeSplit(40, 30, 25, 60)).toBe(60)
})

test("moving the result divider down reduces the result panel", () => {
  expect(resizeResultSplit(38, 10, 25, 60)).toBe(28)
  expect(resizeResultSplit(38, -10, 25, 60)).toBe(48)
})
