import { LockKeyhole, Plus } from "lucide-react"

import { Button } from "@/components/ui/button"
import type { ConnectionProfileSummary } from "@/lib/judge-api"

import type { ProfileCheck } from "./connections-check"
import { EngineChip, ErrorNotice, surfaceCardClass } from "./connections-ui"
import type { LearningLoadPhase } from "./load-state"

type ProfileListProps = {
  profiles: ConnectionProfileSummary[]
  phase: LearningLoadPhase
  error: string | null
  selectedId: string | null
  checks: Record<string, ProfileCheck>
  disabled: boolean
  onNew: () => void
  onSelect: (profile: ConnectionProfileSummary) => void
  onRetry: () => void
}

export function ConnectionsProfileList({
  profiles,
  phase,
  error,
  selectedId,
  checks,
  disabled,
  onNew,
  onSelect,
  onRetry,
}: ProfileListProps) {
  return (
    <section aria-label="Saved profiles" className="flex min-w-0 flex-[1_1_280px] flex-col gap-3">
      <Button variant="outline" className="justify-start bg-card shadow-xs" onClick={onNew} disabled={disabled}>
        <Plus /> New profile
      </Button>
      {phase === "loading" ? (
        <p role="status" className={`${surfaceCardClass} p-4 text-sm text-muted-foreground`}>
          Loading saved connection profiles...
        </p>
      ) : phase === "error" && error ? (
        <ErrorNotice message={error} onRetry={onRetry} />
      ) : phase === "empty" ? (
        <p className={`${surfaceCardClass} p-4 text-sm text-muted-foreground`}>
          No saved profiles yet. Create one to browse your own database read-only.
        </p>
      ) : (
        <ul className={`${surfaceCardClass} flex flex-col gap-0.5 p-1.5`}>
          {profiles.map((profile) => (
            <li key={profile.profileId}>
              <ProfileRow
                profile={profile}
                check={checks[profile.profileId]}
                selected={profile.profileId === selectedId}
                onSelect={() => onSelect(profile)}
              />
            </li>
          ))}
        </ul>
      )}
      <div className="flex gap-2.5 rounded-[10px] bg-muted px-3.5 py-3 text-[13px] text-muted-foreground">
        <LockKeyhole className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
        <span>
          Passwords live in your system keychain. They're never shown again, written to logs or
          included in backups.
        </span>
      </div>
    </section>
  )
}

function ProfileRow({
  profile,
  check,
  selected,
  onSelect,
}: {
  profile: ConnectionProfileSummary
  check: ProfileCheck | undefined
  selected: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-current={selected ? "true" : undefined}
      className="flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=true]:bg-muted"
    >
      <EngineChip engine={profile.engine} />
      <span className="flex min-w-0 flex-1 flex-col leading-[1.35]">
        <span className="truncate font-medium">{profile.displayName}</span>
        {check && !check.ok ? (
          <span className="truncate text-[12.5px] text-err">{check.message}</span>
        ) : (
          <span className="truncate text-[12.5px] text-muted-foreground">
            {profile.host}:{profile.port}
          </span>
        )}
      </span>
      {check ? (
        <span
          role="img"
          aria-label={check.ok ? "Read-only verified" : "Test failed"}
          className={`size-2 shrink-0 rounded-full ${check.ok ? "bg-ok" : "bg-err"}`}
        />
      ) : (
        <span className="shrink-0 text-xs text-muted-foreground">Not tested</span>
      )}
    </button>
  )
}
