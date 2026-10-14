import { useState } from "react"
import { cn } from "cn"
import { ArrowRight, LoaderCircle } from "lucide-react"

import { buttonVariants } from "@/components/ui/button"
import { isTauri } from "@/lib/tauri"

import { ErrorNotice } from "./connections-ui"
import { useConnectionProfiles } from "./connections-use-profiles"
import { explorerHref, profileIdFromHash } from "./explorer-route"
import { ExplorerWorkspace } from "./explorer-workspace"

export function ExplorerScreen() {
  const { profiles, phase, error, retry } = useConnectionProfiles()
  const [selectedId, setSelectedId] = useState(() => profileIdFromHash(window.location.hash))
  const profile = profiles.find((item) => item.profileId === selectedId) ?? profiles[0]

  function select(profileId: string) {
    setSelectedId(profileId)
    window.history.replaceState(null, "", explorerHref(profileId))
  }

  return (
    <main className="flex min-h-0 flex-1 flex-col overflow-auto">
      {phase === "loading" ? (
        <p role="status" className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin" aria-hidden="true" /> Loading saved connection profiles...
        </p>
      ) : phase === "error" && error ? (
        <div className="max-w-xl p-6">
          <ErrorNotice message={error} onRetry={retry} />
        </div>
      ) : !profile ? (
        <div className="mx-auto flex max-w-md flex-col items-start gap-3 px-6 py-16">
          <h2 className="text-lg font-semibold">No connection profiles</h2>
          <p className="text-sm text-muted-foreground">
            {isTauri()
              ? "Save a read-only profile for your own PostgreSQL or SQL Server database to browse its tables here."
              : "External connections are only available in the SQL Judge desktop app."}
          </p>
          <a href="#connections" className={cn(buttonVariants({ variant: "outline" }))}>
            Open Connections <ArrowRight />
          </a>
        </div>
      ) : (
        <ExplorerWorkspace key={profile.profileId} profile={profile} profiles={profiles} onSelect={select} />
      )}
    </main>
  )
}
