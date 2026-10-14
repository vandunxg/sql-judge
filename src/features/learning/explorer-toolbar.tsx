import { ChevronsUpDown, LoaderCircle, Lock, Play, ShieldCheck } from "lucide-react"

import { Button } from "@/components/ui/button"
import type { ConnectionProfileSummary } from "@/lib/judge-api"

import { EngineChip, smallButtonClass } from "./connections-ui"
import type { ExplorerSession } from "./explorer-use-session"

type ToolbarProps = {
  profile: ConnectionProfileSummary
  profiles: ConnectionProfileSummary[]
  session: ExplorerSession
  onSelect: (profileId: string) => void
}

const badgeClass = "inline-flex h-[22px] items-center gap-[5px] rounded-full px-2 text-xs font-medium whitespace-nowrap"

export function ExplorerToolbar({ profile, profiles, session, onSelect }: ToolbarProps) {
  const connecting = session.busy === "connect"
  return (
    <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2">
      <label className="relative inline-flex h-8 min-w-0 items-center gap-2.5 rounded-lg pr-7 pl-2 hover:bg-muted has-focus-visible:ring-3 has-focus-visible:ring-ring/50">
        <EngineChip engine={profile.engine} size="sm" />
        <select
          aria-label="Connection profile"
          className="min-w-0 cursor-pointer appearance-none truncate bg-transparent text-sm font-medium outline-none"
          value={profile.profileId}
          onChange={(event) => onSelect(event.target.value)}
        >
          {profiles.map((item) => (
            <option key={item.profileId} value={item.profileId}>
              {item.displayName}
            </option>
          ))}
        </select>
        <ChevronsUpDown className="pointer-events-none absolute right-2 size-3.5 text-muted-foreground" aria-hidden="true" />
      </label>
      {session.verified ? (
        <span className={`${badgeClass} bg-ok-bg text-ok`}>
          <Lock className="size-3" aria-hidden="true" /> Read-only
        </span>
      ) : (
        <span className={`${badgeClass} border text-muted-foreground`} role={connecting ? "status" : undefined}>
          {connecting ? "Verifying read-only access..." : "Not connected"}
        </span>
      )}
      <Button
        variant="outline"
        className={`${smallButtonClass} bg-card shadow-xs`}
        onClick={() => void session.connect()}
        disabled={session.busy !== null}
      >
        {connecting ? <LoaderCircle className="animate-spin" /> : <ShieldCheck />}
        {session.info ? "Reconnect" : "Connect"}
      </Button>
      <div className="flex-1" />
      <Button
        variant="outline"
        className={`${smallButtonClass} bg-card shadow-xs`}
        title="Run (Ctrl Enter)"
        aria-keyshortcuts="Control+Enter"
        onClick={() => void session.run()}
        disabled={!session.verified || session.busy !== null || !session.query.trim()}
      >
        <Play className="size-[13px]" /> Run
        <span className="font-mono text-[11px] text-muted-foreground" aria-hidden="true">
          Ctrl ↵
        </span>
      </Button>
    </div>
  )
}
