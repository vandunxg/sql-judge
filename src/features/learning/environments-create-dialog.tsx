import { useId, useState, type RefObject } from "react"
import { Check, Plus, ShieldCheck } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { InstalledPack } from "@/lib/judge-api"
import { cn } from "@/lib/utils"
import {
  DIALOG_FIELD_CLASS,
  EnvironmentDialog,
  ImpactItem,
  ImpactList,
} from "./environments-dialog"
import { packKey } from "./environments-model"

export function EnvironmentCreateDialog({
  packs,
  busy,
  returnFocus,
  onCancel,
  onCreate,
}: {
  packs: InstalledPack[]
  busy: boolean
  returnFocus: RefObject<HTMLElement | null>
  onCancel: () => void
  onCreate: (
    pack: InstalledPack | null,
    namespace: string
  ) => Promise<string | null>
}) {
  const packId = useId()
  const namespaceId = useId()
  const [selectedKey, setSelectedKey] = useState(() =>
    packs[0] ? packKey(packs[0]) : ""
  )
  const [namespace, setNamespace] = useState("sj_practice")
  const [error, setError] = useState<string | null>(null)
  const pack =
    packs.find((candidate) => packKey(candidate) === selectedKey) ?? null

  async function submit() {
    const result = await onCreate(pack, namespace)
    if (result) {
      setError(result)
    } else {
      onCancel()
    }
  }

  return (
    <EnvironmentDialog
      open
      busy={busy}
      onClose={onCancel}
      icon={<Plus />}
      returnFocus={returnFocus}
      title="New environment"
      description="SQL Judge provisions an isolated practice schema from the exact installed pack you pick."
    >
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          void submit()
        }}
      >
        <div className="flex flex-col gap-2 text-sm">
          <label htmlFor={packId}>Pack</label>
          <select
            id={packId}
            value={selectedKey}
            onChange={(event) => setSelectedKey(event.target.value)}
            disabled={busy || packs.length === 0}
            className={cn(
              DIALOG_FIELD_CLASS,
              "w-full border border-input font-sans text-sm outline-none focus-visible:border-ring focus-visible:ring-3 disabled:opacity-50"
            )}
          >
            {packs.length === 0 ? (
              <option value="">No imported packs</option>
            ) : null}
            {packs.map((candidate) => (
              <option key={packKey(candidate)} value={packKey(candidate)}>
                {candidate.title} {candidate.version}
              </option>
            ))}
          </select>
        </div>
        <div className="flex flex-col gap-2 text-sm">
          <label htmlFor={namespaceId}>Namespace</label>
          <Input
            id={namespaceId}
            value={namespace}
            onChange={(event) => {
              setNamespace(event.target.value)
              setError(null)
            }}
            placeholder="sj_practice"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            aria-invalid={error ? true : undefined}
            className={DIALOG_FIELD_CLASS}
          />
        </div>
        <ImpactList>
          <ImpactItem
            icon={<ShieldCheck className="text-ok" />}
            label="Pinned pack"
          >
            <span className="font-mono">
              {pack
                ? `${pack.packId} ${pack.version} · ${pack.digest.slice(0, 12)}`
                : "import a pack first"}
            </span>
          </ImpactItem>
          <ImpactItem icon={<Check className="text-ok" />} label="Not affected">
            submission history, other schemas, your external connections
          </ImpactItem>
        </ImpactList>
        {error ? (
          <p className="text-[13px] text-err" role="alert">
            {error}
          </p>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={onCancel}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !pack}>
            Create environment
          </Button>
        </div>
      </form>
    </EnvironmentDialog>
  )
}
