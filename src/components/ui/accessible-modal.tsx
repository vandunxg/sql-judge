import {
  useEffect,
  useId,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react"
import { createPortal } from "react-dom"

import {
  nextModalFocusIndex,
  selectModalInitialFocus,
  shouldFocusModalContainer,
  shouldDismissModalOnEscape,
} from "@/lib/modal-a11y"
import { cn } from "@/lib/utils"

const FOCUSABLE_SELECTOR =
  'a[href], area[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
}

export function AccessibleModal({
  open,
  title,
  description,
  busy = false,
  initialFocusRef,
  restoreFocusRef,
  onCancel,
  children,
  className,
  headerClassName,
  role = "alertdialog",
}: {
  open: boolean
  title: ReactNode
  description: ReactNode
  busy?: boolean
  initialFocusRef?: RefObject<HTMLElement | null>
  restoreFocusRef?: RefObject<HTMLElement | null>
  onCancel: () => void
  children: ReactNode
  className?: string
  headerClassName?: string
  role?: "dialog" | "alertdialog"
}) {
  const dialogRef = useRef<HTMLElement>(null)
  const titleId = useId()
  const descriptionId = useId()

  useEffect(() => {
    if (!open || typeof document === "undefined") {
      return
    }

    const previouslyFocused =
      restoreFocusRef?.current ??
      (document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null)
    const overlay = dialogRef.current?.parentElement
    const backgroundElements = Array.from(document.body.children).filter(
      (element): element is HTMLElement => element !== overlay
    )
    const previousAttributes = backgroundElements.map((element) => ({
      element,
      inert: element.inert,
      ariaHidden: element.getAttribute("aria-hidden"),
    }))

    for (const element of backgroundElements) {
      element.inert = true
      element.setAttribute("aria-hidden", "true")
    }

    const focusTimer = window.setTimeout(() => {
      const dialog = dialogRef.current
      if (!dialog) {
        return
      }
      const focusable = getFocusableElements(dialog)
      const preferred = initialFocusRef?.current
      const target = selectModalInitialFocus(
        preferred && focusable.includes(preferred) ? preferred : null,
        focusable,
        dialog
      )
      target.focus()
    }, 0)

    return () => {
      window.clearTimeout(focusTimer)
      for (const { element, inert, ariaHidden } of previousAttributes) {
        element.inert = inert
        if (ariaHidden === null) {
          element.removeAttribute("aria-hidden")
        } else {
          element.setAttribute("aria-hidden", ariaHidden)
        }
      }
      if (previouslyFocused?.isConnected) {
        previouslyFocused.focus()
      }
    }
  }, [initialFocusRef, open, restoreFocusRef])

  useEffect(() => {
    if (!open || !busy) {
      return
    }
    const dialog = dialogRef.current
    if (
      dialog &&
      shouldFocusModalContainer(busy, getFocusableElements(dialog).length)
    ) {
      dialog.focus()
    }
  }, [busy, open])

  function handleKeyDown(event: ReactKeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && shouldDismissModalOnEscape(busy)) {
      event.preventDefault()
      onCancel()
      return
    }
    if (event.key !== "Tab") {
      return
    }

    const focusable = getFocusableElements(event.currentTarget)
    if (focusable.length === 0) {
      event.preventDefault()
      event.currentTarget.focus()
      return
    }

    const currentIndex = focusable.indexOf(
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : focusable[0]
    )
    const nextIndex = nextModalFocusIndex(
      currentIndex < 0 ? 0 : currentIndex,
      focusable.length,
      event.shiftKey
    )
    event.preventDefault()
    focusable[nextIndex]?.focus()
  }

  if (!open || typeof document === "undefined") {
    return null
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4 backdrop-blur-sm">
      <section
        ref={dialogRef}
        className={cn(
          "w-full max-w-md rounded-lg border border-amber-500/40 bg-background p-4 shadow-lg",
          className
        )}
        role={role}
        aria-modal="true"
        aria-busy={busy}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        tabIndex={-1}
        onKeyDown={handleKeyDown}
      >
        <header className={headerClassName}>
          <h2 id={titleId} className="text-sm font-semibold">
            {title}
          </h2>
          <p
            id={descriptionId}
            className="mt-1 text-xs leading-5 text-muted-foreground"
          >
            {description}
          </p>
        </header>
        {children}
      </section>
    </div>,
    document.body
  )
}
