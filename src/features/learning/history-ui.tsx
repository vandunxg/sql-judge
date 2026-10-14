import type { ReactNode } from "react"
import { Check, LoaderCircle, Play, RefreshCw, TriangleAlert, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import type { HistoryChipTone } from "./history-state"

export const historyCardClass =
  "rounded-xl border border-border bg-card text-card-foreground shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export const chipToneClass: Record<HistoryChipTone, string> = {
  ok: "bg-ok-bg text-ok",
  err: "bg-err-bg text-err",
  warn: "bg-warn-bg text-warn",
  run: "bg-muted text-muted-foreground",
}

const chipIcon = { ok: Check, err: X, warn: TriangleAlert, run: Play }

export function VerdictChip({ label, tone }: { label: string; tone: HistoryChipTone }) {
  const Icon = chipIcon[tone]
  return (
    <span
      className={cn(
        "inline-flex h-6 items-center gap-[5px] self-start rounded-full px-[9px] text-[12.5px] font-medium whitespace-nowrap",
        chipToneClass[tone]
      )}
    >
      <Icon className="size-[13px]" aria-hidden="true" />
      {label}
    </span>
  )
}

export function ScreenHeading({ title, children }: { title: string; children: ReactNode }) {
  return (
    <header className="flex flex-col gap-1.5">
      <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">{title}</h1>
      <p className="text-[15px] text-muted-foreground">{children}</p>
    </header>
  )
}

export function HistoryLoadState({ error, onRetry }: { error: string | null; onRetry: () => void }) {
  if (error) {
    return (
      <div className={cn(historyCardClass, "flex flex-col items-center gap-3 p-8 text-center")} role="alert">
        <p className="text-sm text-err">{error}</p>
        <Button variant="outline" size="sm" onClick={onRetry}>
          <RefreshCw /> Retry
        </Button>
      </div>
    )
  }
  return (
    <p className={cn(historyCardClass, "flex items-center gap-2 p-6 text-sm text-muted-foreground")} role="status">
      <LoaderCircle className="size-4 animate-spin" /> Loading local history...
    </p>
  )
}
