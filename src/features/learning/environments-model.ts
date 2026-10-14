import type {
  ConnectionProfileSummary,
  InstalledPack,
  ManagedEnvironmentSummary,
} from "@/lib/judge-api"

export type EnvironmentTone = "ok" | "info" | "warn" | "err"

const MAX_NAMESPACE_LENGTH = 63

export function postgresqlProfiles(
  profiles: ConnectionProfileSummary[]
): ConnectionProfileSummary[] {
  return profiles.filter((profile) => profile.engine === "postgresql")
}

export function environmentSourceLabel(
  profileId: string | null,
  profiles: ConnectionProfileSummary[]
): string {
  if (!profileId) {
    return "Local runtime"
  }
  return (
    profiles.find((profile) => profile.profileId === profileId)?.displayName ??
    "External PostgreSQL"
  )
}

export function isCurrentEnvironmentLoad(
  loadId: number,
  currentLoadId: number
): boolean {
  return loadId === currentLoadId
}

export function environmentProfileSelection(
  selectedProfileId: string | null,
  profiles: ConnectionProfileSummary[]
): string | null {
  if (!selectedProfileId) {
    return null
  }
  return profiles.some((profile) => profile.profileId === selectedProfileId)
    ? selectedProfileId
    : null
}

export function packKey(pack: InstalledPack): string {
  return `${pack.packId}@${pack.version}#${pack.digest}`
}

export function packLabel(
  environment: ManagedEnvironmentSummary,
  packs: InstalledPack[]
): string {
  const pack = packs.find(
    (candidate) =>
      candidate.packId === environment.packId &&
      candidate.version === environment.packVersion &&
      candidate.digest === environment.packDigest
  )
  return pack?.title ?? environment.packId
}

// Mirrors the native ledger rule so invalid names are caught before the call;
// the native side still validates every request.
export function isManagedNamespace(namespace: string): boolean {
  return (
    namespace.length <= MAX_NAMESPACE_LENGTH &&
    /^sj_[a-z0-9_]*$/.test(namespace)
  )
}

export function uniqueNamespace(
  namespace: string,
  suffix: string,
  taken: string[]
): string {
  const base = namespace.replace(new RegExp(`_${suffix}\\d+$`), "")
  for (let index = 1; ; index += 1) {
    const tail = `_${suffix}${index}`
    const candidate = base.slice(0, MAX_NAMESPACE_LENGTH - tail.length) + tail
    if (candidate !== namespace && !taken.includes(candidate)) {
      return candidate
    }
  }
}

export function confirmationMatches(typed: string, namespace: string) {
  return namespace.length > 0 && typed.trim() === namespace
}

const STATUS: Record<
  ManagedEnvironmentSummary["state"],
  { label: string; tone: EnvironmentTone; pending: boolean }
> = {
  ready: { label: "Ready", tone: "ok", pending: false },
  running: { label: "Judging", tone: "info", pending: true },
  provisioning: { label: "Provisioning", tone: "info", pending: true },
  rebuilding: { label: "Rebuilding", tone: "info", pending: true },
  deleting: { label: "Deleting", tone: "info", pending: true },
  needs_recovery: { label: "Needs recovery", tone: "warn", pending: false },
  error: { label: "Error", tone: "err", pending: false },
}

export function environmentStatus(state: ManagedEnvironmentSummary["state"]) {
  return STATUS[state]
}

export function environmentBlockedReason(
  environment: ManagedEnvironmentSummary,
  busy: boolean
): string | null {
  if (busy) {
    return "Another environment action is in progress."
  }
  if (
    environment.mode !== "managed" &&
    environment.mode !== "external_sandbox"
  ) {
    return "Only schemas owned by SQL Judge can change."
  }
  switch (environment.state) {
    case "ready":
      return null
    case "running":
      return "Available after the current judge finishes."
    case "needs_recovery":
      return "Interrupted operation needs recovery before this schema can change."
    case "error":
      return "This schema is in an error state and cannot change from here."
    default:
      return "Available after the current operation finishes."
  }
}

export function namespaceError(
  namespace: string,
  environments: ManagedEnvironmentSummary[]
): string | null {
  if (!isManagedNamespace(namespace)) {
    return "Use a namespace that starts with sj_ and has only lowercase letters, digits and underscores."
  }
  if (environments.some((environment) => environment.namespace === namespace)) {
    return "Another environment already uses this namespace."
  }
  return null
}
