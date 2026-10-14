import { LoaderCircle, RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import type { ConnectionProfileSummary } from "@/lib/judge-api"

import { ENGINE_LABELS, tlsShortLabel } from "./connections-profile"
import { ErrorNotice, smallButtonClass } from "./connections-ui"
import { EXPLORER_LIMITS } from "./explorer-api"
import { ExplorerResults } from "./explorer-results"
import { ExplorerSchemaTree } from "./explorer-schema-tree"
import { ExplorerSqlEditor } from "./explorer-sql-editor"
import { ExplorerToolbar } from "./explorer-toolbar"
import { useExplorerSession, type ExplorerSession } from "./explorer-use-session"

type WorkspaceProps = {
  profile: ConnectionProfileSummary
  profiles: ConnectionProfileSummary[]
  onSelect: (profileId: string) => void
}

export function ExplorerWorkspace({ profile, profiles, onSelect }: WorkspaceProps) {
  const session = useExplorerSession(profile)
  return (
    <>
      <ExplorerToolbar profile={profile} profiles={profiles} session={session} onSelect={onSelect} />
      <div className="@container flex min-h-0 flex-1 flex-wrap">
        <section
          aria-label="Schema browser"
          className="flex max-h-full min-h-[400px] max-w-full flex-[1_1_260px] flex-col border-r @max-[780px]:max-h-[400px] @max-[780px]:border-r-0 @max-[780px]:border-b"
        >
          <SchemaPanel session={session} profile={profile} />
        </section>
        <div className="flex min-h-[400px] min-w-0 flex-[999_1_520px] flex-col">
          <ExplorerSqlEditor
            value={session.query}
            label={`Read-only ${profile.engine === "sqlserver" ? "T-SQL SELECT" : "SELECT"} for ${profile.displayName}`}
            onChange={session.setQuery}
            onRun={() => void session.run()}
          />
          <ExplorerResults session={session} />
          <div className="flex flex-wrap gap-x-[18px] gap-y-1.5 border-t px-4 py-2 text-xs text-muted-foreground">
            <span>{session.verified ? "Read-only transaction" : "Read-only not verified yet"}</span>
            <span>{EXPLORER_LIMITS.statementTimeoutMs / 1000} s timeout</span>
            <span>{EXPLORER_LIMITS.maxBytes / 1024} KiB result cap</span>
            <span>{tlsShortLabel(profile.tlsMode)}</span>
            {session.info ? <span className="truncate">{session.info.serverVersion}</span> : null}
          </div>
        </div>
      </div>
    </>
  )
}

function SchemaPanel({ session, profile }: { session: ExplorerSession; profile: ConnectionProfileSummary }) {
  const { busy, failure, metadata } = session
  const message = (text: string, status = false) => (
    <p role={status ? "status" : undefined} className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
      {status ? <LoaderCircle className="size-4 shrink-0 animate-spin" aria-hidden="true" /> : null}
      {text}
    </p>
  )
  if (busy === "connect") {
    return message("Testing the connection and verifying read-only access...", true)
  }
  if (failure?.step === "connect") {
    return (
      <div className="p-3">
        <ErrorNotice message={failure.message} onRetry={() => void session.connect()} />
      </div>
    )
  }
  if (busy === "metadata") {
    return message("Loading tables...", true)
  }
  if (failure?.step === "metadata") {
    return (
      <div className="p-3">
        <ErrorNotice message={failure.message} onRetry={() => void session.loadMetadata()} />
      </div>
    )
  }
  if (metadata) {
    return <ExplorerSchemaTree metadata={metadata} database={profile.database} />
  }
  if (session.info) {
    return (
      <div className="p-4">
        <Button variant="outline" className={smallButtonClass} onClick={() => void session.loadMetadata()}>
          <RefreshCw /> Load tables
        </Button>
      </div>
    )
  }
  return message(
    `Connect to verify read-only access and load ${ENGINE_LABELS[profile.engine]} tables. Nothing connects automatically.`
  )
}
