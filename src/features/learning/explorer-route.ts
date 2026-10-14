import { parseHash } from "@/app-route-state"

export function explorerHref(profileId: string) {
  return `#explorer?${new URLSearchParams({ profile: profileId })}`
}

export function profileIdFromHash(hash: string): string | null {
  const { route, params } = parseHash(hash)
  return route === "explorer" ? params.get("profile") : null
}
