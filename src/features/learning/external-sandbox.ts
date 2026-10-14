import type { ConnectionProfileSummary } from "@/lib/judge-api"

export function sandboxProfiles(profiles: ConnectionProfileSummary[]) {
  return profiles.filter(
    (profile) => profile.engine === "postgresql" && profile.hasSecret
  )
}
