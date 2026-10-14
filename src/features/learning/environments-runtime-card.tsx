import { Check, ChevronDown, Database, Plus, Zap } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type {
  ConnectionProfileSummary,
  ManagedRuntimeStatus,
} from "@/lib/judge-api"
import { cn } from "@/lib/utils"
import { CARD_CLASS, SMALL_BUTTON_CLASS, StatusBadge } from "./environments-ui"

const GUARANTEES = [
  "Reachable only from this device",
  "Your SQL runs as a read-only role",
  "Ownership checked before every reset",
]

export function RuntimeCard({
  status,
  loading,
  busy,
  onStart,
  profiles,
  selectedProfileId,
  onProfileChange,
  onManageConnections,
}: {
  status: ManagedRuntimeStatus | null
  loading: boolean
  busy: boolean
  onStart: () => void
  profiles: ConnectionProfileSummary[]
  selectedProfileId: string | null
  onProfileChange: (profileId: string | null) => void
  onManageConnections: () => void
}) {
  const localSelected = selectedProfileId === null
  const selectedProfile = profiles.find(
    (profile) => profile.profileId === selectedProfileId
  )
  const available = status?.available ?? false
  const badge = !localSelected
    ? { tone: "ok" as const, label: "Selected" }
    : loading
      ? { tone: "info" as const, label: "Checking" }
      : available
        ? { tone: "ok" as const, label: "Running" }
        : { tone: "warn" as const, label: status ? "Stopped" : "Unknown" }

  return (
    <section
      aria-labelledby="environments-runtime-heading"
      className={cn(CARD_CLASS, "flex flex-col gap-[18px] px-[22px] py-5")}
    >
      <div className="flex flex-wrap items-center gap-4">
        <div
          className={cn(
            "flex size-[42px] shrink-0 items-center justify-center rounded-[10px]",
            !localSelected || available
              ? "bg-ok-bg text-ok"
              : "bg-warn-bg text-warn"
          )}
          aria-hidden="true"
        >
          <Database className="size-5" />
        </div>
        <div className="flex min-w-0 flex-[1_1_260px] flex-col gap-0.5">
          <div className="flex items-center gap-2.5">
            <h2
              id="environments-runtime-heading"
              className="text-base font-semibold"
            >
              {localSelected ? "Local runtime" : "External PostgreSQL"}
            </h2>
            <StatusBadge tone={badge.tone} pending={loading} dot={!loading}>
              {badge.label}
            </StatusBadge>
          </div>
          <p className="text-[13px] text-muted-foreground">
            {localSelected
              ? (status?.message ??
                (loading
                  ? "Checking managed PostgreSQL..."
                  : "Runtime status could not be read."))
              : "Practice schemas from the selected PostgreSQL profile."}
          </p>
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger
            disabled={busy}
            aria-label="Environment source"
            render={
              <button
                type="button"
                className="flex min-w-[220px] items-center gap-2 rounded-xl bg-muted/60 px-2.5 py-2 text-left transition-colors hover:bg-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50"
              />
            }
          >
            <SourceMark local={localSelected} />
            <span className="flex min-w-0 flex-1 flex-col leading-tight">
              <span className="text-[11px] text-muted-foreground">
                Environment source
              </span>
              <span className="truncate text-[13px] font-semibold text-foreground">
                {localSelected
                  ? "Local runtime"
                  : (selectedProfile?.displayName ?? "External PostgreSQL")}
              </span>
            </span>
            <ChevronDown
              className="size-3.5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            sideOffset={8}
            className="w-[286px] rounded-xl p-1.5 shadow-[0_4px_16px_rgba(0,0,0,0.06)]"
          >
            <DropdownMenuGroup>
              <DropdownMenuLabel className="px-2 pt-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase">
                Environment sources
              </DropdownMenuLabel>
              <div className="px-2 pb-1.5 text-xs text-muted-foreground">
                Choose where practice schemas are loaded from.
              </div>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              className={cn(
                "my-0.5 min-h-12 gap-2 px-2 py-1.5",
                localSelected && "bg-accent"
              )}
              onClick={() => onProfileChange(null)}
            >
              <SourceMark local />
              <span className="flex min-w-0 flex-1 flex-col leading-tight">
                <span className="font-semibold">Local runtime</span>
                <span className="text-xs text-muted-foreground">Built-in</span>
              </span>
              {localSelected ? <SelectedCheck /> : null}
            </DropdownMenuItem>
            {profiles.length > 0 ? (
              <>
                <div className="px-2 pt-2 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase">
                  PostgreSQL profiles
                </div>
                {profiles.map((profile) => {
                  const selected = profile.profileId === selectedProfileId
                  return (
                    <DropdownMenuItem
                      key={profile.profileId}
                      className={cn(
                        "my-0.5 min-h-12 gap-2 px-2 py-1.5",
                        selected && "bg-accent"
                      )}
                      onClick={() => onProfileChange(profile.profileId)}
                    >
                      <SourceMark />
                      <span className="flex min-w-0 flex-1 flex-col leading-tight">
                        <span className="truncate font-semibold">
                          {profile.displayName}
                        </span>
                        <span className="truncate text-xs text-muted-foreground">
                          {profile.host}:{profile.port} · {profile.database}
                        </span>
                      </span>
                      {selected ? <SelectedCheck /> : null}
                    </DropdownMenuItem>
                  )
                })}
              </>
            ) : null}
            <DropdownMenuSeparator />
            <DropdownMenuItem
              className="my-0.5 gap-2 px-2 py-2 font-semibold"
              onClick={onManageConnections}
            >
              <span className="flex size-7 items-center justify-center rounded-lg bg-muted text-foreground">
                <Plus className="size-3.5" aria-hidden="true" />
              </span>
              Manage connection profiles
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        {localSelected && status && !available ? (
          <Button
            variant="outline"
            className={cn(
              SMALL_BUTTON_CLASS,
              "bg-card shadow-[0_1px_2px_rgba(16,16,24,0.05)]"
            )}
            onClick={onStart}
            disabled={busy}
          >
            Start runtime
          </Button>
        ) : null}
      </div>
      <ul className="flex flex-wrap gap-x-7 gap-y-2.5 border-t pt-4">
        {GUARANTEES.map((guarantee) => (
          <li key={guarantee} className="flex items-center gap-2 text-[13px]">
            <Check className="size-3.5 text-ok" aria-hidden="true" />
            {guarantee}
          </li>
        ))}
      </ul>
    </section>
  )
}

function SourceMark({ local = false }: { local?: boolean }) {
  const Icon = local ? Zap : Database
  return (
    <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-primary">
      <Icon className="size-4" aria-hidden="true" />
    </span>
  )
}

function SelectedCheck() {
  return (
    <span className="flex size-5 shrink-0 items-center justify-center rounded-full bg-foreground text-background">
      <Check className="size-3" aria-hidden="true" />
    </span>
  )
}
