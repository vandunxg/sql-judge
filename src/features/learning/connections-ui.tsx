import type { ReactNode } from "react"
import { RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"

import type { Engine } from "./connections-profile"

export const surfaceCardClass =
  "rounded-xl border bg-card shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export const fieldInputClass = "h-9 bg-card px-3 text-sm md:text-sm dark:bg-card"

export const smallButtonClass = "h-8 gap-1.5 px-2.5 text-[13px]"

const ENGINE_CHIPS: Record<Engine, { short: string; className: string }> = {
  postgresql: {
    short: "PG",
    className: "bg-info-bg text-info",
  },
  sqlserver: {
    short: "MS",
    className: "bg-err-bg text-err",
  },
}

export function EngineChip({ engine, size = "md" }: { engine: Engine; size?: "sm" | "md" }) {
  const chip = ENGINE_CHIPS[engine]
  const sizeClass = size === "sm" ? "size-[22px] rounded-md text-[10px]" : "size-8 rounded-lg text-[11px]"
  return (
    <span
      aria-hidden="true"
      className={`flex shrink-0 items-center justify-center font-semibold ${sizeClass} ${chip.className}`}
    >
      {chip.short}
    </span>
  )
}

export function Field({
  label,
  htmlFor,
  className = "",
  children,
}: {
  label: string
  htmlFor: string
  className?: string
  children: ReactNode
}) {
  return (
    <div className={`flex min-w-0 flex-col gap-1.5 ${className}`}>
      <Label htmlFor={htmlFor} className="text-[13.5px] font-medium">
        {label}
      </Label>
      {children}
    </div>
  )
}

export function ErrorNotice({
  message,
  onRetry,
  retryLabel = "Retry",
}: {
  message: string
  onRetry?: () => void
  retryLabel?: string
}) {
  return (
    <div role="alert" className="rounded-lg border border-err/30 bg-err-bg p-3 text-sm text-err">
      <p>{message}</p>
      {onRetry ? (
        <Button className={`mt-2.5 ${smallButtonClass}`} variant="outline" onClick={onRetry}>
          <RefreshCw /> {retryLabel}
        </Button>
      ) : null}
    </div>
  )
}
