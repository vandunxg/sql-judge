import { useEffect, useId, useState, type RefObject } from "react"
import { Check, Plus, ShieldCheck } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  probeExternalSandboxProfile,
  type ConnectionProfileSummary,
  type InstalledPack,
} from "@/lib/judge-api"
import { readExternalError } from "@/lib/external-errors"
import {
  DIALOG_FIELD_CLASS,
  EnvironmentDialog,
  ImpactItem,
  ImpactList,
} from "./environments-dialog"
import { packKey } from "./environments-model"
import { sandboxProfiles } from "./external-sandbox"

export function EnvironmentCreateDialog({
  packs,
  profiles,
  busy,
  returnFocus,
  onCancel,
  onCreateExternal,
}: {
  packs: InstalledPack[]
  profiles: ConnectionProfileSummary[]
  busy: boolean
  returnFocus: RefObject<HTMLElement | null>
  onCancel: () => void
  onCreateExternal: (
    pack: InstalledPack | null,
    envId: string,
    profileId: string,
    confirmed: boolean
  ) => Promise<string | null>
}) {
  const packId = useId()
  const environmentFieldId = useId()
  const [selectedKey, setSelectedKey] = useState(() =>
    packs[0] ? packKey(packs[0]) : ""
  )
  const eligibleProfiles = sandboxProfiles(profiles)
  const [profileId, setProfileId] = useState("")
  const [environmentId, setEnvironmentId] = useState("external-practice")
  const [probeResult, setProbeResult] = useState<{
    profileId: string
    database?: string
    error?: string
  } | null>(null)
  const [confirmed, setConfirmed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const pack =
    packs.find((candidate) => packKey(candidate) === selectedKey) ?? null
  const selectedProfileId = profileId || eligibleProfiles[0]?.profileId || ""
  const probe =
    probeResult?.profileId === selectedProfileId && probeResult.database
      ? probeResult
      : null
  const probeError =
    probeResult?.profileId === selectedProfileId
      ? (probeResult.error ?? null)
      : null
  const probing = Boolean(selectedProfileId && !probe && !probeError)

  useEffect(() => {
    if (!selectedProfileId) {
      return
    }
    let active = true
    void probeExternalSandboxProfile(selectedProfileId)
      .then((target) => {
        if (active) {
          setProbeResult({
            profileId: selectedProfileId,
            database: target.database,
          })
        }
      })
      .catch((reason) => {
        if (active) {
          setProbeResult({
            profileId: selectedProfileId,
            error: readExternalError(reason),
          })
        }
      })
    return () => {
      active = false
    }
  }, [selectedProfileId])

  async function submit() {
    const result = await onCreateExternal(
      pack,
      environmentId,
      selectedProfileId,
      confirmed
    )
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
          <Select
            value={selectedKey}
            onValueChange={(value) => value && setSelectedKey(value)}
          >
            <SelectTrigger
              id={packId}
              disabled={busy || packs.length === 0}
              className="w-full"
            >
              <SelectValue>
                {() =>
                  pack ? `${pack.title} ${pack.version}` : "No imported packs"
                }
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {packs.map((candidate) => (
                <SelectItem key={packKey(candidate)} value={packKey(candidate)}>
                  {candidate.title} {candidate.version}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-2 text-sm">
          <label>PostgreSQL profile</label>
          <Select
            value={selectedProfileId}
            onValueChange={(value) => {
              if (value) {
                setProfileId(value)
                setConfirmed(false)
                setError(null)
              }
            }}
          >
            <SelectTrigger
              disabled={busy || eligibleProfiles.length === 0}
              className="w-full"
            >
              <SelectValue>
                {() => {
                  const profile = eligibleProfiles.find(
                    (candidate) => candidate.profileId === selectedProfileId
                  )
                  return profile
                    ? `${profile.displayName} · ${profile.database}`
                    : "No saved PostgreSQL profile with a stored secret"
                }}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {eligibleProfiles.map((profile) => (
                <SelectItem key={profile.profileId} value={profile.profileId}>
                  {profile.displayName} · {profile.database}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {probing ? (
            <p className="text-[13px] text-muted-foreground" role="status">
              Verifying writable sandbox target...
            </p>
          ) : null}
          {probeError ? (
            <p className="text-[13px] text-err" role="alert">
              {probeError}
            </p>
          ) : null}
          {probe ? (
            <p className="text-[13px] text-ok" role="status">
              Verified PostgreSQL target: {probe.database}
            </p>
          ) : null}
        </div>
        <div className="flex flex-col gap-2 text-sm">
          <label htmlFor={environmentFieldId}>Environment ID</label>
          <Input
            id={environmentFieldId}
            value={environmentId}
            onChange={(event) => {
              setEnvironmentId(event.target.value)
              setError(null)
            }}
            placeholder="external-practice"
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
          <ImpactItem icon={<ShieldCheck className="text-ok" />} label="Target">
            {probe
              ? `PostgreSQL database ${probe.database}`
              : "verified before creation"}
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
        <label className="flex items-start gap-2 text-sm">
          <Checkbox
            checked={confirmed}
            onCheckedChange={(checked) => setConfirmed(checked === true)}
            disabled={busy || !probe}
          />
          I understand SQL Judge will create only a generated, marker-owned
          schema in this verified database.
        </label>
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
            disabled={busy || !pack || !probe || probing || !confirmed}
          >
            Create environment
          </Button>
        </div>
      </form>
    </EnvironmentDialog>
  )
}
