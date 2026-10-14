import type { ReactNode } from "react"
import { LoaderCircle } from "lucide-react"

import { cn } from "@/lib/utils"
import type { EnvironmentTone } from "./environments-model"

export const CARD_CLASS =
  "rounded-xl border bg-card text-card-foreground shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export const SMALL_BUTTON_CLASS = "h-8 gap-1.5 px-2.5 text-[13px]"

// No shared info token exists yet; values come from the design export.
const TONE_CLASS: Record<EnvironmentTone, string> = {
  ok: "bg-ok-bg text-ok",
  info: "bg-info-bg text-info",
  warn: "bg-warn-bg text-warn",
  err: "bg-err-bg text-err",
}

export function StatusBadge({
  tone,
  pending = false,
  dot = false,
  children,
}: {
  tone: EnvironmentTone
  pending?: boolean
  dot?: boolean
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        "inline-flex h-[22px] shrink-0 items-center gap-1.5 rounded-full px-2 text-xs font-medium whitespace-nowrap",
        TONE_CLASS[tone]
      )}
    >
      {pending ? (
        <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
      ) : dot ? (
        <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
      ) : null}
      {children}
    </span>
  )
}
