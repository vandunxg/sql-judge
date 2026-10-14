import { describe, expect, test } from "bun:test"

import { toCsv } from "./explorer-csv"

describe("explorer CSV export", () => {
  test("escapes quotes, commas and newlines and keeps NULL distinct from empty text", () => {
    expect(
      toCsv(
        ["id", "note, text", 'say "hi"'],
        [
          [1, 'a "quoted" value', "line1\nline2"],
          [2, null, ""],
          [3, true, "carriage\rreturn"],
        ]
      )
    ).toBe(
      [
        'id,"note, text","say ""hi"""',
        '1,"a ""quoted"" value","line1\nline2"',
        '2,,""',
        '3,true,"carriage\rreturn"',
      ].join("\r\n")
    )
  })

  test("returns only the header row when there are no rows", () => {
    expect(toCsv(["a", "b"], [])).toBe("a,b")
  })
})
