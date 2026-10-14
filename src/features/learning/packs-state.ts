import type { InstalledPack, ManagedEnvironmentSummary } from "@/lib/judge-api"

import { catalogProblemKey } from "./catalog"
import type { ProblemStatus } from "./problem-status"

export type PackTone = "ok" | "warn" | "err" | "muted"
type PackIdentity = Pick<InstalledPack, "packId" | "version" | "digest">

export function packProgress(
  pack: PackIdentity & Pick<InstalledPack, "problems">,
  statuses: Map<string, Pick<ProblemStatus, "status">>
) {
  const total = pack.problems.length
  const solved = pack.problems.filter(
    (problem) =>
      statuses.get(catalogProblemKey(pack, problem.id))?.status === "solved"
  ).length
  return { solved, total, percent: total ? Math.round((solved / total) * 100) : 0 }
}

const dialectLabels: Record<InstalledPack["dialect"], string> = {
  postgresql: "PostgreSQL",
  sqlserver: "SQL Server",
}

export function packDialectLabel(dialect: InstalledPack["dialect"]) {
  return dialectLabels[dialect] ?? dialect
}

export function packTableCount(pack: Pick<InstalledPack, "problems">) {
  return new Set(
    pack.problems.flatMap((problem) => (problem.schema ?? []).map((table) => table.name))
  ).size
}

export function shortDigest(digest: string) {
  return digest.length > 12
    ? `sha256:${digest.slice(0, 4)}…${digest.slice(-4)}`
    : `sha256:${digest}`
}

export function packMonogram(title: string) {
  const words = title.split(/[^\p{L}\p{N}]+/u).filter(Boolean)
  const [first = "", second] = words
  const acronym = first.length > 1 && first === first.toUpperCase()
  const letters = second && !acronym ? first[0] + second[0] : first.slice(0, 2)
  return letters.toUpperCase() || "?"
}

const stateBadges: Record<ManagedEnvironmentSummary["state"], PackTone> = {
  ready: "ok",
  running: "ok",
  provisioning: "muted",
  rebuilding: "muted",
  deleting: "muted",
  needs_recovery: "warn",
  error: "err",
}
const toneRank: PackTone[] = ["ok", "muted", "warn", "err"]

export function packEnvironments(
  pack: PackIdentity,
  environments: ManagedEnvironmentSummary[]
) {
  return environments.filter(
    (environment) =>
      environment.mode === "managed" &&
      environment.packId === pack.packId &&
      environment.packVersion === pack.version &&
      environment.packDigest === pack.digest
  )
}

/** Mirrors the native rule: a pack is deleted only when all its environments can be. */
export function packDeleteBlocker(environments: ManagedEnvironmentSummary[]) {
  const blocked = environments.find(
    (environment) => environment.state !== "ready" && environment.state !== "error"
  )
  return blocked
    ? `${blocked.namespace} ${blocked.state === "needs_recovery" ? "needs recovery" : `is ${blocked.state}`}. Wait for it to finish or recover it from Environments before deleting this pack.`
    : null
}

export function packEnvironmentBadge(
  pack: PackIdentity,
  environments: ManagedEnvironmentSummary[]
): { label: string; tone: PackTone } {
  const owned = packEnvironments(pack, environments)
  if (owned.length === 0) {
    return { label: "No environment", tone: "muted" }
  }
  const tone = owned
    .map((environment) => stateBadges[environment.state])
    .reduce((worst, next) =>
      toneRank.indexOf(next) > toneRank.indexOf(worst) ? next : worst
    )
  if (owned.length > 1) {
    return { label: `${owned.length} environments`, tone }
  }
  return { label: `Environment ${owned[0].state.replace("_", " ")}`, tone }
}
