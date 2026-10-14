import { useRef, useState, type RefObject } from "react"
import { Trash2 } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import type { ConnectionProfileSummary } from "@/lib/judge-api"

import { EnvironmentDialog } from "./environments-dialog"

export function ConnectionsDeleteDialog({
  profile,
  returnFocus,
  onCancel,
  onDelete,
}: {
  profile: ConnectionProfileSummary
  returnFocus: RefObject<HTMLElement | null>
  onCancel: () => void
  onDelete: () => Promise<string | null>
}) {
  const cancelRef = useRef<HTMLButtonElement>(null)
  const [busy, setBusy] = useState(false)

  async function confirm() {
    setBusy(true)
    const nextError = await onDelete()
    if (nextError) {
      toast.error(nextError)
      setBusy(false)
    }
  }

  return (
    <EnvironmentDialog
      open
      busy={busy}
      onClose={onCancel}
      icon={<Trash2 />}
      danger
      initialFocus={cancelRef}
      returnFocus={returnFocus}
      title={<span className="block min-w-0 truncate" title={`Delete ${profile.displayName}?`}>Delete {profile.displayName}?</span>}
      description={
        <span>
          The saved connection profile and its stored credential are removed
          from this device.
        </span>
      }
    >
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          ref={cancelRef}
          type="button"
          variant="outline"
          onClick={onCancel}
          disabled={busy}
        >
          Cancel
        </Button>
        <Button
          type="button"
          onClick={() => void confirm()}
          disabled={busy}
          className="bg-destructive text-white hover:bg-destructive/90"
        >
          {busy ? "Deleting..." : "Delete profile"}
        </Button>
      </div>
    </EnvironmentDialog>
  )
}
