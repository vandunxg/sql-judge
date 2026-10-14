import { useId, useRef, useState, type RefObject } from "react"
import { Check, Database, Package, ShieldCheck, Trash2, TriangleAlert } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { deletePack, type InstalledPack, type ManagedEnvironmentSummary } from "@/lib/judge-api"
import { readPackDeleteError } from "@/lib/pack-errors"

import { DIALOG_FIELD_CLASS, EnvironmentDialog, ImpactItem, ImpactList } from "./environments-dialog"
import { confirmationMatches } from "./environments-model"
import { packDeleteBlocker } from "./packs-state"

export function PackDeleteDialog({
  pack,
  environments,
  returnFocus,
  onCancel,
  onDeleted,
}: {
  pack: InstalledPack
  /** The pack's managed environments, or null when they could not be loaded. */
  environments: ManagedEnvironmentSummary[] | null
  returnFocus: RefObject<HTMLElement | null>
  onCancel: () => void
  onDeleted: () => void
}) {
  const fieldId = useId()
  const fieldRef = useRef<HTMLInputElement>(null)
  const [typed, setTyped] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const blocker = environments ? packDeleteBlocker(environments) : null
  const ready = confirmationMatches(typed, pack.packId) && !blocker
  const problems = `${pack.problems.length} ${pack.problems.length === 1 ? "problem" : "problems"}`

  async function submit() {
    if (!ready || busy) {
      return
    }
    setBusy(true)
    setError(null)
    try {
      await deletePack({ packId: pack.packId, version: pack.version, digest: pack.digest, confirmed: true })
      onDeleted()
    } catch (reason) {
      setError(readPackDeleteError(reason))
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
      initialFocus={fieldRef}
      returnFocus={returnFocus}
      title={
        <>
          Delete {pack.title} <span className="font-mono text-base">v{pack.version}</span>?
        </>
      }
      description="The pack and its practice environments are removed from this device. Import the pack again to get it back."
    >
      <ImpactList>
        <ImpactItem icon={<Package className="text-err" />} label="Removed">
          {pack.title} v{pack.version} and its {problems}
        </ImpactItem>
        <ImpactItem icon={<Database className="text-err" />} label="Environments">
          {environments === null
            ? "status could not be loaded; every environment of this pack is deleted too"
            : environments.length === 0
              ? "none"
              : environments.map((environment) => environment.namespace).join(", ")}
        </ImpactItem>
        {environments === null || environments.length > 0 ? (
          <ImpactItem icon={<ShieldCheck className="text-ok" />} label="Owner">
            managed by SQL Judge on this runtime; verified again before each delete
          </ImpactItem>
        ) : null}
        <ImpactItem icon={<Check className="text-ok" />} label="Not affected">
          submission history (shown as from a removed pack), other packs, your external connections
        </ImpactItem>
      </ImpactList>

      {blocker ? (
        <p className="flex gap-2 rounded-lg bg-warn-bg p-3 text-[13px] text-warn" role="status">
          <TriangleAlert aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
          {blocker}
        </p>
      ) : null}

      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          void submit()
        }}
      >
        <div className="flex flex-col gap-2 text-sm">
          <label htmlFor={fieldId}>
            Type{" "}
            <span className="rounded bg-muted px-[5px] py-px font-mono font-medium">{pack.packId}</span>{" "}
            to confirm
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
          <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button
            type="submit"
            disabled={!ready || busy}
            className="bg-destructive text-white hover:bg-destructive/90"
          >
            {busy ? "Deleting…" : "Delete pack"}
          </Button>
        </div>
      </form>
    </EnvironmentDialog>
  )
}
