import { expect, test } from "bun:test"

import { parseHash, problemHref, routeForLaunch } from "./app-route-state"

test("opens the application on Home", () => {
  expect(routeForLaunch()).toBe("home")
})

test("reads the route and query parameters from the hash", () => {
  const { route, params } = parseHash("#practice?problem=hr%401.0.0%23abc%3Ap1")
  expect(route).toBe("practice")
  expect(params.get("problem")).toBe("hr@1.0.0#abc:p1")
})

test("falls back to Home for unknown or empty hashes", () => {
  expect(parseHash("#nope").route).toBe("home")
  expect(parseHash("").route).toBe("home")
})

test("builds a round-trippable deep link to a problem", () => {
  const key = "hr@1.0.0#abc:p1"
  expect(parseHash(problemHref(key)).params.get("problem")).toBe(key)
})
