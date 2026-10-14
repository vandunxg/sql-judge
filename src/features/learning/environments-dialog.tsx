import type { ReactNode, RefObject } from "react"
import { AlertDialog } from "@base-ui/react/alert-dialog"

import { cn } from "@/lib/utils"

export const DIALOG_FIELD_CLASS =
  "h-9 rounded-lg bg-card px-3 py-0 font-mono text-[13.5px] leading-6 md:text-[13.5px] md:leading-6 focus-visible:ring-ring/15 dark:bg-card"

/**
 * Modal shell for environment lifecycle dialogs. Base UI traps focus,
 * closes on Escape and returns focus to the opener; closing is ignored
 * while a native call is in flight so a pending action cannot be dismissed.
 */
export function EnvironmentDialog({
  open,
  busy,
  onClose,
  icon,
  danger = false,
  title,
  description,
  initialFocus,
  returnFocus,
  children,
}: {
  open: boolean
  busy: boolean
  onClose: () => void
  icon: ReactNode
  danger?: boolean
  title: ReactNode
  description: ReactNode
  initialFocus?: RefObject<HTMLElement | null>
  returnFocus?: RefObject<HTMLElement | null>
  children: ReactNode
}) {
  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) {
          onClose()
        }
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="fixed inset-0 z-50 bg-[rgb(9_9_11/0.45)] dark:bg-black/60" />
        <AlertDialog.Viewport className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto p-6">
          <AlertDialog.Popup
            aria-busy={busy}
            initialFocus={initialFocus}
            finalFocus={() =>
              returnFocus?.current?.isConnected ? returnFocus.current : true
            }
            className="flex w-full max-w-[520px] flex-col gap-5 rounded-[14px] border bg-card p-6 text-card-foreground shadow-[0_24px_64px_rgba(0,0,0,0.22)] outline-none"
          >
            <div className="flex items-start gap-3.5">
              <div
                className={cn(
                  "flex size-10 shrink-0 items-center justify-center rounded-full [&_svg]:size-[18px]",
                  danger ? "bg-err-bg text-err" : "bg-muted text-foreground"
                )}
                aria-hidden="true"
              >
                {icon}
              </div>
              <div className="flex min-w-0 flex-col gap-1.5">
                <AlertDialog.Title className="text-lg leading-normal font-semibold tracking-[-0.01em] break-words">
                  {title}
                </AlertDialog.Title>
                <AlertDialog.Description className="text-sm leading-normal text-muted-foreground">
                  {description}
                </AlertDialog.Description>
              </div>
            </div>
            {children}
          </AlertDialog.Popup>
        </AlertDialog.Viewport>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}

export function ImpactList({ children }: { children: ReactNode }) {
  return (
    <ul className="flex flex-col gap-2.5 rounded-[10px] border px-4 py-3.5">
      {children}
    </ul>
  )
}

export function ImpactItem({
  icon,
  label,
  children,
}: {
  icon: ReactNode
  label: string
  children: ReactNode
}) {
  return (
    <li className="grid grid-cols-[20px_1fr] gap-2.5 text-[13.5px] leading-normal [&>svg]:mt-0.5 [&>svg]:size-4">
      {icon}
      <span>
        <span className="font-medium">{label}:</span>{" "}
        <span className="text-muted-foreground">{children}</span>
      </span>
    </li>
  )
}
