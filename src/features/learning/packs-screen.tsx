import { useState } from "react"
import { Info, Plus, RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { pickPackSource } from "@/lib/judge-api"
import { isPackImportDisabled } from "@/lib/pack-errors"
import { isTauri } from "@/lib/tauri"

import { PackCard } from "./packs-card"
import { usePacksData } from "./packs-data"
import { PacksImportZone } from "./packs-import-zone"
import { PacksReview } from "./packs-review"
import { packsHeadingClass, packsSurfaceClass } from "./packs-styles"
import { problemStatuses } from "./problem-status"

export function PacksScreen() {
  const data = usePacksData()
  const [reviewPath, setReviewPath] = useState<string | null>(null)
  const [picking, setPicking] = useState(false)
  const [pickError, setPickError] = useState<string | null>(null)
  const desktopAvailable = isTauri()
  const importDisabled = isPackImportDisabled(desktopAvailable, picking)
  const statuses = data.history ? problemStatuses(data.history) : null

  async function handlePick(kind: "archive" | "directory") {
    setPicking(true)
    setPickError(null)
    try {
      const path = await pickPackSource(kind)
      if (path) setReviewPath(path)
    } catch {
      setPickError("Native pack selection is unavailable. Enter a local path instead.")
    } finally {
      setPicking(false)
    }
  }

  if (reviewPath !== null) {
    return (
      <PacksReview
        key={reviewPath}
        path={reviewPath}
        disabled={!desktopAvailable}
        onClose={() => setReviewPath(null)}
        refreshAfterImport={data.refreshAfterImport}
      />
    )
  }

  return (
    <main className="min-h-0 flex-1 overflow-y-auto px-4 py-8 text-sm sm:px-8 lg:px-10 lg:pb-12">
      <div className="mx-auto flex max-w-[1080px] flex-col gap-7">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div className="flex flex-col gap-1.5">
            <h1 className={packsHeadingClass}>Packs</h1>
            <p className="max-w-[62ch] text-[15px] text-muted-foreground">
              Problem sets installed on this device. Every version is checked in a throwaway database before it's installed, and never changes after.
            </p>
          </div>
          <Button disabled={importDisabled} onClick={() => void handlePick("archive")}>
            <Plus aria-hidden="true" />
            Import pack
          </Button>
        </header>

        <PacksImportZone
          disabled={importDisabled}
          desktopAvailable={desktopAvailable}
          error={pickError}
          onPick={(kind) => void handlePick(kind)}
          onReview={setReviewPath}
        />

        <section aria-labelledby="installed-h" className="flex flex-col gap-3.5">
          <h2 id="installed-h" className="text-[15px] font-semibold">
            Installed
            {data.catalogPhase === "ready" ? <span className="font-normal text-muted-foreground"> · {data.packs.length}</span> : null}
          </h2>

          {data.catalogPhase === "loading" ? (
            <div role="status" aria-label="Loading installed packs" className="grid grid-cols-[repeat(auto-fit,minmax(min(420px,100%),1fr))] gap-5">
              <Skeleton className="h-56 rounded-xl" />
              <Skeleton className="h-56 rounded-xl" />
            </div>
          ) : data.catalogPhase === "error" ? (
            <div role="alert" className={`${packsSurfaceClass} flex flex-col items-center gap-3 p-8 text-center`}>
              <p className="text-sm text-err">{data.catalogError}</p>
              <Button variant="outline" size="sm" onClick={data.retryCatalog}>
                <RefreshCw aria-hidden="true" />
                Retry
              </Button>
            </div>
          ) : data.catalogPhase === "empty" ? (
            <div className={`${packsSurfaceClass} p-8 text-center`}>
              <p className="text-sm font-medium">No installed packs yet</p>
              <p className="mt-1 text-sm text-muted-foreground">Import a validated pack to make practice content available.</p>
            </div>
          ) : (
            <>
              {data.extrasFailed ? (
                <div role="alert" className="flex flex-wrap items-center justify-between gap-3 rounded-lg bg-warn-bg px-4 py-3 text-sm text-warn">
                  <span>Progress or environment status could not be loaded. Packs are still available.</span>
                  <Button variant="outline" size="sm" onClick={data.retryExtras}>
                    <RefreshCw aria-hidden="true" />
                    Retry
                  </Button>
                </div>
              ) : null}
              <div className="grid grid-cols-[repeat(auto-fit,minmax(min(420px,100%),1fr))] gap-5">
                {data.packs.map((pack, index) => (
                  <PackCard
                    key={`${pack.packId}@${pack.version}#${pack.digest}`}
                    pack={pack}
                    index={index}
                    statuses={statuses}
                    environments={data.environments}
                  />
                ))}
              </div>
            </>
          )}
        </section>

        <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
          <Info aria-hidden="true" className="size-3.5 shrink-0" />
          Imported packs are untrusted content: setup SQL only runs in disposable, app-managed environments.
        </p>
      </div>
    </main>
  )
}
