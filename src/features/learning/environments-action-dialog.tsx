import { useId, useRef, useState, type RefObject } from "react"
import {
  Check,
  Copy,
  Package,
  RotateCcw,
  ShieldCheck,
  Trash2,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { PendingAction } from "./environments-commands"
import {
  DIALOG_FIELD_CLASS,
  EnvironmentDialog,
  ImpactItem,
  ImpactList,
} from "./environments-dialog"
import { confirmationMatches, uniqueNamespace } from "./environments-model"

const COPY = {
  reset: { verb: "Reset", icon: <RotateCcw />, confirm: "Reset environment" },
  delete: { verb: "Delete", icon: <Trash2 />, confirm: "Delete environment" },
  clone: { verb: "Clone", icon: <Copy />, confirm: "Clone environment" },
}

export function EnvironmentActionDialog({
  pending,
  packTitle,
  takenNamespaces,
  busy,
  returnFocus,
  onCancel,
  onConfirm,
}: {
  pending: PendingAction
  packTitle: string
  takenNamespaces: string[]
  busy: boolean
  returnFocus: RefObject<HTMLElement | null>
  onCancel: () => void
  onConfirm: (cloneNamespace: string) => Promise<string | null>
}) {
  const { action, environment } = pending
  const fieldId = useId()
  const fieldRef = useRef<HTMLInputElement>(null)
  const [typed, setTyped] = useState(() =>
    action === "clone"
      ? uniqueNamespace(environment.namespace, "copy", takenNamespaces)
      : ""
  )
  const [error, setError] = useState<string | null>(null)
  const destructive = action !== "clone"
  const ready = destructive
    ? confirmationMatches(typed, environment.namespace)
    : typed.trim().length > 0
  const copy = COPY[action]
  const identity = `${environment.packId} ${environment.packVersion} · ${environment.packDigest.slice(0, 12)}`

  async function submit() {
    if (!ready || busy) {
      return
    }
    setError(await onConfirm(typed))
  }

  return (
    <EnvironmentDialog
      open
      busy={busy}
      onClose={onCancel}
      icon={copy.icon}
      danger={destructive}
      initialFocus={fieldRef}
      returnFocus={returnFocus}
      title={
        <>
          {copy.verb}{" "}
          <span className="font-mono text-base">{environment.namespace}</span>
          {destructive ? "?" : null}
        </>
      }
      description={
        action === "reset"
          ? `The schema is rebuilt from ${packTitle} ${environment.packVersion}. Anything you changed inside it is lost. If the rebuild fails halfway, the current schema stays as it is.`
          : action === "delete"
            ? "The schema and its environment record are removed. This cannot be undone from the app."
            : `Creates a new practice schema from the same pinned ${packTitle} ${environment.packVersion}. The source schema is not changed.`
      }
    >
      <ImpactList>
        {action === "reset" ? (
          <ImpactItem icon={<Trash2 className="text-err" />} label="Recreated">
            as {pending.replacementNamespace}; {environment.namespace} is
            removed only after the new schema is ready
          </ImpactItem>
        ) : action === "delete" ? (
          <ImpactItem icon={<Trash2 className="text-err" />} label="Removed">
            schema {environment.namespace} and environment {environment.envId}
          </ImpactItem>
        ) : null}
        <ImpactItem
          icon={<Package className="text-muted-foreground" />}
          label="Pack"
        >
          <span className="font-mono">{identity}</span>
        </ImpactItem>
        {destructive ? (
          <ImpactItem icon={<ShieldCheck className="text-ok" />} label="Owner">
            managed by SQL Judge on this runtime; verified again before the{" "}
            {action}
          </ImpactItem>
        ) : null}
        <ImpactItem icon={<Check className="text-ok" />} label="Not affected">
          submission history,{" "}
          {destructive ? "other schemas" : environment.namespace}, your external
          connections
        </ImpactItem>
      </ImpactList>

      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          void submit()
        }}
      >
        <div className="flex flex-col gap-2 text-sm">
          <label htmlFor={fieldId}>
            {destructive ? (
              <>
                Type{" "}
                <span className="rounded bg-muted px-[5px] py-px font-mono font-medium">
                  {environment.namespace}
                </span>{" "}
                to confirm
              </>
            ) : (
              "New namespace"
            )}
          </label>
          <Input
            id={fieldId}
            ref={fieldRef}
            value={typed}
            onChange={(event) => {
              setTyped(event.target.value)
              setError(null)
            }}
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            aria-invalid={error ? true : undefined}
            className={DIALOG_FIELD_CLASS}
          />
          {error ? (
            <p className="text-[13px] text-err" role="alert">
              {error}
            </p>
          ) : null}
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={onCancel}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            disabled={!ready || busy}
            className={
              destructive
                ? "bg-destructive text-white hover:bg-destructive/90"
                : undefined
            }
          >
            {copy.confirm}
          </Button>
        </div>
      </form>
    </EnvironmentDialog>
  )
}
