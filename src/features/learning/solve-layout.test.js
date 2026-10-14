import { expect, test } from "bun:test"

import { resizeSplit } from "./solve-layout"

test("keeps a resizable split inside its minimum and maximum bounds", () => {
  expect(resizeSplit(40, -30, 25, 60)).toBe(25)
  expect(resizeSplit(40, 15, 25, 60)).toBe(55)
  expect(resizeSplit(40, 30, 25, 60)).toBe(60)
})
