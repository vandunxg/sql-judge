import { describe, expect, test } from "bun:test"

import { explorerHref, profileIdFromHash } from "./explorer-route"

describe("explorer route", () => {
  test("round-trips an encoded profile id through the hash", () => {
    const href = explorerHref("analytics replica&x=1")
    expect({
      href,
      parsed: profileIdFromHash(href),
      missing: profileIdFromHash("#explorer"),
      otherRoute: profileIdFromHash("#connections?profile=hr"),
    }).toEqual({
      href: "#explorer?profile=analytics+replica%26x%3D1",
      parsed: "analytics replica&x=1",
      missing: null,
      otherRoute: null,
    })
  })
})
