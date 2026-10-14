import type { ReactNode } from "react"
import { cn } from "cn"

export const settingsFocusRing =
  "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"

export function SettingsSection({
  id,
  title,
  children,
}: {
  id: string
  title: string
  children: ReactNode
}) {
  return (
    <section id={id} aria-labelledby={`${id}-title`} className="flex scroll-mt-6 flex-col gap-3">
      <h2 id={`${id}-title`} className="text-base font-semibold tracking-[-0.01em]">
        {title}
      </h2>
      <div className="rounded-xl border border-border bg-card text-sm shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]">
        {children}
      </div>
    </section>
  )
}

export function SettingRow({
  title,
  titleId,
  description,
  children,
}: {
  title: ReactNode
  titleId?: string
  description?: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border-t border-border px-[22px] py-[18px] first:border-t-0">
      <div className="flex flex-[1_1_300px] flex-col gap-0.5">
        <span id={titleId} className="font-medium">
          {title}
        </span>
        {description ? <span className="text-[13px] text-muted-foreground">{description}</span> : null}
      </div>
      {children}
    </div>
  )
}

export function SettingSwitch({
  checked,
  onCheckedChange,
  labelledBy,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  labelledBy: string
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-labelledby={labelledBy}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "relative h-[22px] w-[38px] shrink-0 cursor-pointer rounded-full",
        settingsFocusRing,
        checked ? "bg-primary" : "bg-input"
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "absolute top-[3px] size-4 rounded-full shadow-[0_1px_2px_rgba(0,0,0,0.25)] transition-[left]",
          checked ? "left-[19px] bg-primary-foreground" : "left-[3px] bg-white"
        )}
      />
    </button>
  )
}
