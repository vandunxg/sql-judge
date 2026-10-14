import type { PackTone } from "./packs-state"

export const packsShadowClass =
  "shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export const packsSurfaceClass = `rounded-xl border border-border bg-card text-card-foreground ${packsShadowClass}`

export const packsHeadingClass =
  "text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]"

export const packsSmallButtonClass = "h-8 gap-1.5 rounded-lg px-2.5 text-[13px]"

export const packsOutlineButtonClass = `border-border bg-card dark:border-border dark:bg-card ${packsShadowClass}`

export const packsPillClass = "h-[22px] rounded-full px-2 text-xs font-medium"

export const packsToneClass: Record<PackTone, string> = {
  ok: "border-transparent bg-ok-bg text-ok",
  warn: "border-transparent bg-warn-bg text-warn",
  err: "border-transparent bg-err-bg text-err",
  muted: "border-transparent bg-muted text-muted-foreground",
}

// Monogram tints follow the Packs export, mapped onto the shared status tokens.
export const packsMonogramClasses = [
  "bg-info-bg text-info",
  "bg-warn-bg text-warn",
]
