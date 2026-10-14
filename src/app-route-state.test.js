import { expect, test } from "bun:test"

import { parseHash, problemKeyFromHash, problemHref, routeForHash, routeForLaunch } from "./app-route-state"

test("opens the application on Home", () => {
  expect(routeForLaunch()).toBe("home")
})

test("opens the Problems list from its route", () => {
  expect(routeForLaunch("#problems")).toBe("problems")
})

test("opens Solve from a problem route", () => {
  expect(routeForLaunch("#solve/hr%401.2%23digest%3Asecond-salary")).toBe("solve")
})

test("keeps every sidebar route reachable by hash", () => {
  for (const route of [
    "history",
    "progress",
    "packs",
    "environments",
    "connections",
    "explorer",
    "settings",
  ]) {
    expect(routeForLaunch(`#${route}`)).toBe(route)
  }
  expect(routeForLaunch("#solve/")).toBe("home")
  expect(routeForLaunch("#unknown")).toBe("home")
})

test("reads the route and query parameters from the hash", () => {
  const { route, params } = parseHash("#explorer?profile=analytics+replica%26x%3D1")
  expect(route).toBe("explorer")
  expect(params.get("profile")).toBe("analytics replica&x=1")
  expect(routeForHash("#problems?pack=hr")).toBe("problems")
})

test("falls back to Home for unknown or empty hashes", () => {
  expect(parseHash("#nope").route).toBe("home")
  expect(parseHash("").route).toBe("home")
})

test("builds a round-trippable Solve link to a problem", () => {
  const key = "hr@1.0.0#abc:p1"
  const href = problemHref(key)
  expect(href).toBe("#solve/hr%401.0.0%23abc%3Ap1")
  expect(routeForHash(href)).toBe("solve")
  expect(decodeURIComponent(href.slice("#solve/".length))).toBe(key)
})

test("reads the current problem key after navigating between solve hashes", () => {
  expect(problemKeyFromHash("#solve/first")).toBe("first")
  expect(problemKeyFromHash("#solve/second")).toBe("second")
})
