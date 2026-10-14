import { expect, test } from "bun:test"

import { parseDrafts } from "./draft-store"

test("keeps only string drafts from stored JSON", () => {
  expect(parseDrafts('{"a":"SELECT 1","b":2,"c":null}')).toEqual({ a: "SELECT 1" })
})

test("treats malformed or non-object storage as empty", () => {
  expect(parseDrafts(null)).toEqual({})
  expect(parseDrafts("{oops")).toEqual({})
  expect(parseDrafts("[1,2]")).toEqual({})
})
