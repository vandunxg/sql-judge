import { Check, TriangleAlert, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { formatTimestamp, shortHash } from "./history-format"
import { historyChip, type HistoryEntry } from "./history-state"
import { historyCardClass, VerdictChip } from "./history-ui"
import { isJudgeVerdict, verdictLabel, verdictTone } from "./verdict-display"

type Props = { entry: HistoryEntry; now: number; onClose: () => void }

const fixtureIcon = { ok: Check, err: X, warn: TriangleAlert }

export function HistoryDetail({ entry, now, onClose }: Props) {
  const { record } = entry
  return (
    <aside
      id="history-detail"
      aria-labelledby="history-detail-title"
      className={cn(historyCardClass, "flex min-w-0 flex-[1_1_300px] flex-col gap-[18px] p-5")}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex flex-col gap-1.5">
          <VerdictChip {...historyChip(entry)} />
          <h2 id="history-detail-title" title={entry.title} className="line-clamp-2 text-base leading-[1.35] font-semibold">
            {entry.title}
          </h2>
        </div>
        <Button variant="ghost" size="icon-sm" aria-label="Close details" onClick={onClose}>
          <X />
        </Button>
      </div>
      <dl className="m-0 grid grid-cols-[110px_1fr] gap-x-3 gap-y-2.5 text-[13px]">
        <dt className="text-muted-foreground">{record.mode === "run" ? "Ran" : "Submitted"}</dt>
        <dd>{formatTimestamp(entry.createdAt, now)}</dd>
        <dt className="text-muted-foreground">Pack</dt>
        <dd className="min-w-0 break-words">{entry.packLabel}</dd>
        <dt className="text-muted-foreground">Pack hash</dt>
        <dd className="font-mono text-xs" title={record.packHash}>{shortHash(record.packHash)}</dd>
        <dt className="text-muted-foreground">Engine</dt>
        <dd className="min-w-0 break-words">{record.engineVersion}</dd>
      </dl>
      <div className="flex flex-col gap-2">
        <span className="text-[13px] font-medium">Fixtures</span>
        {record.tests.length > 0 ? (
          <ul className="m-0 flex list-none flex-col gap-0.5 p-0 text-[13px]">
            {record.tests.map((test, index) => {
              const verdict = isJudgeVerdict(test.verdict) ? test.verdict : null
              const source = { status: "completed" as const, verdict: verdict ?? undefined }
              const tone = verdict ? verdictTone(source) : "warn"
              const passed = tone === "ok"
              const Icon = fixtureIcon[tone]
              return (
                <li
                  key={`${test.testId}-${index}`}
                  className={cn("flex items-center gap-2 rounded-md px-2 py-1.5", !passed && (tone === "err" ? "bg-err-bg" : "bg-warn-bg"))}
                >
                  <Icon className={cn("size-3.5 shrink-0", passed ? "text-ok" : tone === "err" ? "text-err" : "text-warn")} role="img" aria-label={passed ? "Passed" : "Failed"} />
                  <span className="min-w-0 flex-1 break-words">{index + 1} · {test.testId}</span>
                  {passed ? null : (
                    <span className={cn("font-mono", tone === "err" ? "text-err" : "text-warn")}>
                      {verdict ? verdictLabel(source) : "Unknown verdict"}
                    </span>
                  )}
                </li>
              )
            })}
          </ul>
        ) : (
          <p className="text-[13px] text-muted-foreground">No fixture details were recorded for this {record.mode === "run" ? "run" : "submission"}.</p>
        )}
      </div>
      {entry.problemHref ? (
        <Button variant="outline" className="h-9" nativeButton={false} render={<a href={entry.problemHref} />}>
          Open problem
        </Button>
      ) : (
        <p className="text-[13px] text-muted-foreground">
          {record.problemId
            ? `This problem is not installed in ${entry.packLabel}, so it can't be opened.`
            : "This record was saved before problems were tracked, so it can't be opened."}
        </p>
      )}
    </aside>
  )
}
