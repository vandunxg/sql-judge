import type { ManagedRuntimeStatus } from "@/lib/judge-api"

export type RuntimeIndicator = {
  label: string
  detail: string
  tone: "ok" | "err" | "muted"
}

// undefined = still loading; null = the status call failed or no native runtime exists.
export function runtimeIndicator(
  status: ManagedRuntimeStatus | null | undefined,
  activeProfileName?: string
): RuntimeIndicator {
  const profileLabel = activeProfileName?.trim()
  if (status === undefined) {
    return {
      label: profileLabel ?? "Checking runtime",
      detail: profileLabel
        ? "Checking runtime · PostgreSQL"
        : "PostgreSQL · local socket",
      tone: "muted",
    }
  }
  if (status === null) {
    return {
      label: profileLabel ?? "Runtime unavailable",
      detail: profileLabel
        ? "Runtime unavailable · Desktop runtime not connected"
        : "Desktop runtime not connected",
      tone: "err",
    }
  }
  return status.available && status.state === "running"
    ? {
        label: profileLabel ?? "Runtime ready",
        detail: profileLabel
          ? "Runtime ready · PostgreSQL"
          : "PostgreSQL · local socket",
        tone: "ok",
      }
    : {
        label: profileLabel ?? "Runtime stopped",
        detail: profileLabel
          ? "Runtime stopped · Start it from Environments"
          : "Start it from Environments",
        tone: "err",
      }
}
