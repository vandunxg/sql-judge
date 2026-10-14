import { Check, Database } from "lucide-react"

import { Button } from "@/components/ui/button"
import type { ManagedRuntimeStatus } from "@/lib/judge-api"
import { cn } from "@/lib/utils"
import { CARD_CLASS, SMALL_BUTTON_CLASS, StatusBadge } from "./environments-ui"

const GUARANTEES = [
  "Reachable only from this device",
  "Your SQL runs as a read-only role",
  "Ownership checked before every reset",
]

export function RuntimeCard({
  status,
  loading,
  busy,
  onStart,
}: {
  status: ManagedRuntimeStatus | null
  loading: boolean
  busy: boolean
  onStart: () => void
}) {
  const available = status?.available ?? false
  const badge = loading
    ? { tone: "info" as const, label: "Checking" }
    : available
      ? { tone: "ok" as const, label: "Running" }
      : { tone: "warn" as const, label: status ? "Stopped" : "Unknown" }

  return (
    <section
      aria-labelledby="environments-runtime-heading"
      className={cn(CARD_CLASS, "flex flex-col gap-[18px] px-[22px] py-5")}
    >
      <div className="flex flex-wrap items-center gap-4">
        <div
          className={cn(
            "flex size-[42px] shrink-0 items-center justify-center rounded-[10px]",
            available ? "bg-ok-bg text-ok" : "bg-warn-bg text-warn"
          )}
          aria-hidden="true"
        >
          <Database className="size-5" />
        </div>
        <div className="flex min-w-0 flex-[1_1_260px] flex-col gap-0.5">
          <div className="flex items-center gap-2.5">
            <h2
              id="environments-runtime-heading"
              className="text-base font-semibold"
            >
              Local runtime
            </h2>
            <StatusBadge tone={badge.tone} pending={loading} dot={!loading}>
              {badge.label}
            </StatusBadge>
          </div>
          <p className="text-[13px] text-muted-foreground">
            {status?.message ??
              (loading
                ? "Checking managed PostgreSQL..."
                : "Runtime status could not be read.")}
          </p>
        </div>
        {status && !available ? (
          <Button
            variant="outline"
            className={cn(
              SMALL_BUTTON_CLASS,
              "bg-card shadow-[0_1px_2px_rgba(16,16,24,0.05)]"
            )}
            onClick={onStart}
            disabled={busy}
          >
            Start runtime
          </Button>
        ) : null}
      </div>
      <ul className="flex flex-wrap gap-x-7 gap-y-2.5 border-t pt-4">
        {GUARANTEES.map((guarantee) => (
          <li key={guarantee} className="flex items-center gap-2 text-[13px]">
            <Check className="size-3.5 text-ok" aria-hidden="true" />
            {guarantee}
          </li>
        ))}
      </ul>
    </section>
  )
}
