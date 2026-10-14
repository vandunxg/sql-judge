import { cn } from "@/lib/utils"

import { formatDuration, whenLabel } from "./history-format"
import { historyChip, type HistoryEntry } from "./history-state"
import { historyCardClass, VerdictChip } from "./history-ui"

type Props = {
  entries: HistoryEntry[]
  selectedId: string | null
  onSelect: (id: string) => void
  compact: boolean
  now: number
}

export const historyRowButtonId = (id: string) => `history-row-${id}`

const thClass = "border-b px-4 py-2.5 text-left text-[13px] font-medium whitespace-nowrap text-muted-foreground"
const metaClass = "px-4 font-mono text-[13px] whitespace-nowrap text-muted-foreground"

export function HistoryTable({ entries, selectedId, onSelect, compact, now }: Props) {
  return (
    <div className={cn(historyCardClass, "min-w-0 flex-[3_1_600px] overflow-hidden")}>
      <div className="overflow-x-auto contain-inline-size">
        <table className="w-full border-collapse text-sm">
          <caption className="sr-only">Runs and submissions, newest first</caption>
          <thead>
            <tr>
              <th scope="col" className={cn(thClass, "w-[170px]")}>Verdict</th>
              <th scope="col" className={thClass}>Problem</th>
              <th scope="col" className={cn(thClass, "w-[90px]")}>Fixtures</th>
              <th scope="col" className={cn(thClass, "w-[90px]")}>Time</th>
              <th scope="col" className={cn(thClass, "w-[120px]")}>When</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => {
              const selected = entry.id === selectedId
              return (
                <tr
                  key={entry.id}
                  aria-current={selected ? "true" : undefined}
                  onClick={() => onSelect(entry.id)}
                  className={cn(
                    "group cursor-pointer border-b last:border-b-0 hover:bg-accent",
                    compact ? "h-10" : "h-[52px]",
                    selected && "bg-muted hover:bg-muted"
                  )}
                >
                  <td className={cn("px-4", selected && "shadow-[inset_2px_0_0_var(--foreground)]")}>
                    <VerdictChip {...historyChip(entry)} />
                  </td>
                  <td className="min-w-[240px] px-4 py-1.5">
                    <button
                      type="button"
                      id={historyRowButtonId(entry.id)}
                      aria-controls="history-detail"
                      aria-expanded={selected}
                      className="rounded-sm text-left font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      {entry.title}
                    </button>
                    <div className={cn("text-[12.5px] text-muted-foreground", compact && "sr-only")}>
                      {entry.record.mode === "run" ? "Ran on sample" : "Submission"} · {entry.packName}
                    </div>
                  </td>
                  <td className={metaClass}>
                    {entry.record.mode === "submit" && entry.total > 0 ? `${entry.passed} / ${entry.total}` : "—"}
                  </td>
                  <td className={metaClass}>{formatDuration(entry.elapsedMs)}</td>
                  <td className="px-4 text-[13px] whitespace-nowrap text-muted-foreground">
                    {whenLabel(entry.createdAt, now)}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
