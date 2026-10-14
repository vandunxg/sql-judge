import { useEffect, useState } from "react"
import { History } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useAppSettings } from "@/lib/app-settings"
import { cn } from "@/lib/utils"

import { useHistoryData } from "./history-data"
import { HistoryDetail } from "./history-detail"
import { HistoryFilterBar } from "./history-filters"
import { filterHistory, paginateHistory, type HistoryFilters } from "./history-state"
import { historyRowButtonId, HistoryTable } from "./history-table"
import { historyCardClass, HistoryLoadState, ScreenHeading } from "./history-ui"

const defaultFilters: HistoryFilters = { tab: "all", query: "", verdict: "all", range: "30d" }

export function HistoryScreen() {
  const { data, error, retry } = useHistoryData()
  const { compactTables } = useAppSettings()
  const [filters, setFilters] = useState(defaultFilters)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [page, setPage] = useState(0)

  const visible = data ? filterHistory(data.entries, filters, data.loadedAt) : []
  const paged = paginateHistory(visible, page)
  const selected = paged.items.find((entry) => entry.id === selectedId) ?? null

  function updateFilters(patch: Partial<HistoryFilters>) {
    setFilters((current) => ({ ...current, ...patch }))
    setPage(0)
    setSelectedId(null)
  }

  function close() {
    if (!selected) return
    setSelectedId(null)
    document.getElementById(historyRowButtonId(selected.id))?.focus()
  }

  useEffect(() => {
    if (!selected) return
    const id = selected.id
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented) return
      setSelectedId(null)
      document.getElementById(historyRowButtonId(id))?.focus()
    }
    document.addEventListener("keydown", onKeyDown)
    return () => document.removeEventListener("keydown", onKeyDown)
  }, [selected])

  return (
    <main className="min-h-0 flex-1 overflow-auto px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1240px] flex-col gap-[22px]">
        <ScreenHeading title="History">
          Every run and submission, kept on this device. The SQL text and any credentials are
          never saved here.
        </ScreenHeading>
        {!data ? (
          <HistoryLoadState error={error} onRetry={retry} />
        ) : (
          <>
            <HistoryFilterBar
              filters={filters}
              onChange={updateFilters}
            />
            {data.packsUnavailable ? (
              <p className="text-[13px] text-muted-foreground" role="status">
                Installed packs could not be loaded, so problem titles are unavailable.
              </p>
            ) : null}
            {visible.length > 0 ? (
              <div className="flex flex-wrap items-start gap-5">
                <div className="min-w-0 flex-[3_1_600px]">
                  <HistoryTable
                    entries={paged.items}
                    selectedId={selected?.id ?? null}
                    onSelect={setSelectedId}
                    compact={compactTables}
                    now={data.loadedAt}
                  />
                  {paged.totalPages > 1 ? (
                    <HistoryPagination
                      page={paged.page}
                      totalPages={paged.totalPages}
                      start={paged.start}
                      end={paged.end}
                      total={visible.length}
                      onPageChange={(nextPage) => {
                        setPage(nextPage)
                        setSelectedId(null)
                      }}
                    />
                  ) : null}
                </div>
                {selected ? <HistoryDetail entry={selected} now={data.loadedAt} onClose={close} /> : null}
              </div>
            ) : (
        <EmptyHistory
          hasRecords={data.entries.length > 0}
          onClear={() => updateFilters(defaultFilters)}
          showAll={() => updateFilters({ ...defaultFilters, range: "all" })}
              />
            )}
          </>
        )}
      </div>
    </main>
  )
}

function HistoryPagination({
  page,
  totalPages,
  start,
  end,
  total,
  onPageChange,
}: {
  page: number
  totalPages: number
  start: number
  end: number
  total: number
  onPageChange: (page: number) => void
}) {
  return (
    <div className="flex items-center justify-between gap-3 px-1 pt-3 text-[13px] text-muted-foreground">
      <span>Showing {start + 1}–{end} of {total}</span>
      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={page === 0}
          onClick={() => onPageChange(page - 1)}
        >
          Previous
        </Button>
        <span aria-live="polite">Page {page + 1} of {totalPages}</span>
        <Button
          variant="outline"
          size="sm"
          disabled={page === totalPages - 1}
          onClick={() => onPageChange(page + 1)}
        >
          Next
        </Button>
      </div>
    </div>
  )
}

function EmptyHistory({ hasRecords, onClear, showAll }: { hasRecords: boolean; onClear: () => void; showAll: () => void }) {
  return (
    <div className={cn(historyCardClass, "flex flex-col items-center gap-2 p-12 text-center")}>
      <History className="size-8 text-muted-foreground/50" aria-hidden="true" />
      <p className="text-sm font-medium">{hasRecords ? "Nothing matches these filters" : "No runs or submissions yet"}</p>
      <p className="text-sm text-muted-foreground">
        {hasRecords
          ? "Try another search, verdict or time range."
          : "Run or submit a solution from a problem to build your local history."}
      </p>
      {hasRecords ? (
        <div className="mt-2 flex flex-wrap justify-center gap-2">
          <Button variant="outline" size="sm" onClick={onClear}>Clear filters</Button>
          <Button variant="ghost" size="sm" onClick={showAll}>Show all time</Button>
        </div>
      ) : (
        <Button className="mt-2" variant="outline" size="sm" nativeButton={false} render={<a href="#problems" />}>
          Browse problems
        </Button>
      )}
    </div>
  )
}
