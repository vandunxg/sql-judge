import { Database, LockKeyhole } from "lucide-react"
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
  onSelect: (profile: ConnectionProfileSummary) => void
  onRetry: () => void
}

export function ConnectionsProfileList({
  profiles,
  phase,
  error,
  selectedId,
  checks,
  onSelect,
  onRetry,
}: ProfileListProps) {
  return (
    <section
      aria-label="Saved profiles"
      className="flex min-w-0 flex-[1_1_280px] flex-col gap-3"
    >
      {phase === "loading" ? (
        <p
          role="status"
          className={`${surfaceCardClass} p-4 text-sm text-muted-foreground`}
        >
          Loading saved connection profiles...
        </p>
      ) : phase === "error" && error ? (
        <ErrorNotice message={error} onRetry={onRetry} />
      ) : phase === "empty" ? (
        <p className={`${surfaceCardClass} p-4 text-sm text-muted-foreground`}>
          No saved profiles yet. Create one to browse your own database
          read-only.
        </p>
      ) : (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
          {profiles.map((profile) => (
            <li key={profile.profileId} className="min-w-0">
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
          Passwords live in your system keychain. They're never shown again,
          written to logs or included in backups.
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
      className="group flex min-h-[190px] w-full min-w-0 flex-col items-start gap-4 overflow-hidden rounded-xl border border-border bg-card p-4 text-left text-sm shadow-[0_1px_2px_rgba(16,16,24,0.05)] transition-colors outline-none hover:border-ring/50 hover:bg-muted/40 focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=true]:border-ring aria-[current=true]:bg-muted/50 dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"
    >
      <span className="flex w-full items-start justify-between gap-3">
        <EngineChip engine={profile.engine} />
        {check ? (
          <span
            role="img"
            aria-label={check.ok ? "Read-only verified" : "Test failed"}
            className={`shrink-0 text-xs font-medium ${check.ok ? "text-ok" : "text-err"}`}
          >
            {check.ok ? "Verified" : "Test failed"}
          </span>
        ) : (
          <span className="text-xs text-muted-foreground">Not tested</span>
        )}
      </span>
      <span className="flex w-full min-w-0 flex-1 flex-col gap-1 leading-[1.35]">
        <span className="line-clamp-2 min-w-0 text-[15px] font-semibold break-words">
          {profile.displayName}
        </span>
        <span className="flex max-w-full min-w-0 items-center gap-1.5 truncate text-[12.5px] text-muted-foreground">
          <Database className="size-3.5 shrink-0" aria-hidden="true" />
          <span className="truncate">{profile.database}</span>
        </span>
        <span className="max-w-full truncate font-mono text-[12px] text-muted-foreground">
          {profile.host}:{profile.port}
        </span>
        {check && !check.ok ? (
          <span className="max-w-full truncate text-[12.5px] text-err">
            {check.message}
          </span>
        ) : null}
      </span>
      <span className="max-w-full truncate text-xs font-medium text-muted-foreground transition-colors group-hover:text-foreground">
        View connection details →
      </span>
    </button>
  )
}
