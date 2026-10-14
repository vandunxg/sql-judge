export const appRoutes = [
  "home",
  "problems",
  "practice",
  "history",
  "progress",
  "packs",
  "environments",
  "connections",
  "explorer",
  "settings",
] as const

export type AppRoute = (typeof appRoutes)[number]

export function routeForLaunch() {
  return "home" as const
}

export function parseHash(hash: string) {
  const [path, query = ""] = hash.replace(/^#/, "").split("?", 2)
  const route = appRoutes.find((candidate) => candidate === path) ?? "home"
  return { route, params: new URLSearchParams(query) }
}

export function problemHref(problemKey: string) {
  return `#practice?${new URLSearchParams({ problem: problemKey })}`
}
