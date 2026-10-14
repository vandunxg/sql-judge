import type { AppRoute } from "@/components/app-sidebar"

const directRoutes: AppRoute[] = [
  "home",
  "problems",
  "environments",
  "history",
  "progress",
  "packs",
  "connections",
  "explorer",
  "settings",
]

export function routeForLaunch(hash = ""): AppRoute {
  return routeForHash(hash)
}

export function routeForHash(hash: string): AppRoute {
  return parseHash(hash).route
}

// Problem keys are URI-encoded, so the first `?` always starts the query string.
export function parseHash(hash: string): { route: AppRoute; params: URLSearchParams } {
  const [path, query = ""] = hash.replace(/^#/, "").split("?", 2)
  const params = new URLSearchParams(query)
  if ((directRoutes as string[]).includes(path)) {
    return { route: path as AppRoute, params }
  }
  return {
    route: path.startsWith("solve/") && path.length > "solve/".length ? "solve" : "home",
    params,
  }
}

export function problemHref(problemKey: string) {
  return `#solve/${encodeURIComponent(problemKey)}`
}

export function problemKeyFromHash(hash: string) {
  if (routeForHash(hash) !== "solve") {
    return undefined
  }
  const encodedKey = hash.slice("#solve/".length)
  try {
    return decodeURIComponent(encodedKey)
  } catch {
    return undefined
  }
}
