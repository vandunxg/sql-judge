export const importStages = ["source", "structure", "runtime", "reference"] as const
export type ImportStage = (typeof importStages)[number]
export type ImportPhase = "ready" | "installing" | "installed" | "failed"
export type ImportCheckStatus =
  "pending" | "running" | "passed" | "failed" | "skipped" | "unknown"
export type ImportCheckStatuses = Record<ImportStage, ImportCheckStatus>

const stageMessages: Array<[ImportStage, RegExp]> = [
  ["source", /^(A local pack path is required|Pack source could not be loaded|Pack import is only available)/u],
  ["structure", /^Pack failed structural validation\.$/u],
  ["runtime", /^(Managed environment state is busy|Managed PostgreSQL runtime)/u],
  ["reference", /^Pack runtime validation/u],
]

/** Expects a message already sanitized by `readPackImportError`. */
export function importFailureStage(message: string): ImportStage | null {
  return stageMessages.find(([, pattern]) => pattern.test(message))?.[0] ?? null
}

export function importCheckStatuses(
  phase: ImportPhase,
  failedStage: ImportStage | null
): ImportCheckStatuses {
  const failedIndex = failedStage ? importStages.indexOf(failedStage) : -1
  const entries = importStages.map((stage, index): [ImportStage, ImportCheckStatus] => {
    switch (phase) {
      case "ready":
        return [stage, "pending"]
      case "installing":
        return [stage, "running"]
      case "installed":
        return [stage, "passed"]
      case "failed":
        if (failedIndex < 0) return [stage, "unknown"]
        return [stage, index < failedIndex ? "passed" : index === failedIndex ? "failed" : "skipped"]
    }
  })
  return Object.fromEntries(entries) as ImportCheckStatuses
}

export function importReportSummary(statuses: ImportCheckStatuses) {
  const values = Object.values(statuses)
  const count = (status: ImportCheckStatus) => values.filter((value) => value === status).length
  if (count("pending") === values.length) return `${values.length} checks before install`
  if (count("running") > 0) return "Checking…"
  if (count("unknown") > 0) return "Not completed"
  const failed = count("failed")
  return failed ? `${count("passed")} passed · ${failed} failed` : `${count("passed")} passed`
}

export function sourceName(path: string) {
  return path.trim().split(/[\\/]+/u).filter(Boolean).at(-1) ?? ""
}
