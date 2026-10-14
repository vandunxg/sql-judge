import { CircleCheck, LoaderCircle } from "lucide-react"

import { Button } from "@/components/ui/button"

import type { ProfileCheck } from "./connections-check"

type FooterProps = {
  check: ProfileCheck | undefined
  testing: boolean
  canTest: boolean
  canDelete: boolean
  disabled: boolean
  isNew: boolean
  onDelete: () => void
  onTest: () => void
  onCancel?: () => void
}

export function ConnectionsFormFooter({
  check,
  testing,
  canTest,
  canDelete,
  disabled,
  isNew,
  onDelete,
  onTest,
  onCancel,
}: FooterProps) {
  return (
    <div className="flex flex-wrap items-center gap-3 border-t bg-sidebar px-6 py-4">
      <div className="flex min-w-0 flex-[1_1_240px] text-[13px]">
        {testing ? (
          <span
            role="status"
            className="inline-flex items-center gap-2 text-muted-foreground"
          >
            <LoaderCircle
              className="size-[15px] animate-spin"
              aria-hidden="true"
            />
            Testing the connection and verifying read-only access...
          </span>
        ) : check?.ok ? (
          <span
            role="status"
            className="inline-flex items-center gap-2 text-ok"
          >
            <CircleCheck className="size-[15px] shrink-0" aria-hidden="true" />
            Connected in {check.elapsedMs} ms · read-only session confirmed
          </span>
        ) : check ? (
          <span role="alert" className="text-err">
            {check.message}
          </span>
        ) : (
          <span className="text-muted-foreground">
            {isNew
              ? "Save the profile, then test it. Nothing connects automatically."
              : "Test uses the saved profile. Nothing connects automatically."}
          </span>
        )}
      </div>
      {canDelete ? (
        <Button
          type="button"
          variant="ghost"
          className="text-err hover:text-err"
          onClick={onDelete}
          disabled={disabled}
        >
          Delete
        </Button>
      ) : null}
      {onCancel ? (
        <Button
          type="button"
          variant="outline"
          className="bg-card shadow-xs"
          onClick={onCancel}
          disabled={disabled}
        >
          Cancel
        </Button>
      ) : null}
      <Button
        type="button"
        variant="outline"
        className="bg-card shadow-xs"
        onClick={onTest}
        disabled={!canTest || testing}
      >
        {check && !check.ok ? "Retry test" : "Test connection"}
      </Button>
      <Button type="submit" disabled={disabled}>
        {isNew ? "Save profile" : "Save changes"}
      </Button>
    </div>
  )
}
