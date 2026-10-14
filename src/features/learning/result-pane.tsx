import { useState } from "react"
import { CheckCircle2, CircleAlert, LoaderCircle, Play, Send, XCircle } from "lucide-react"

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { JudgeRunResult } from "@/lib/judge-api"
import { cn } from "@/lib/utils"

import { paneClass, paneTabClass, paneTabsListClass, sampleTableClass } from "./solve-pane"
import type { JudgeMode } from "./use-judge-run"
import { verdictLabel, verdictTone } from "./verdict-display"

const MAX_RENDERED_ROWS = 100
const MAX_RENDERED_CELL_LENGTH = 512

const toneClass = { ok: "text-ok", err: "text-err", warn: "text-warn" }
const toneIcon = { ok: CheckCircle2, err: XCircle, warn: CircleAlert }

const copy = {
  run: { busy: "Running sample...", idle: "Run your query on the public sample to preview its output." },
  submit: { busy: "Submitting...", idle: "Submit to judge your query against every fixture." },
}

type ResultPaneProps = {
  result: JudgeRunResult | null
  resultMode: JudgeMode
  activeMode: JudgeMode | null
}

export function ResultPane({ result, resultMode, activeMode }: ResultPaneProps) {
  const [tab, setTab] = useState<JudgeMode>(resultMode)
  const [previousActiveMode, setPreviousActiveMode] = useState(activeMode)
  if (activeMode !== previousActiveMode) {
    setPreviousActiveMode(activeMode)
    if (activeMode) {
      setTab(activeMode)
    }
  }

  return (
    <Tabs
      value={tab}
      onValueChange={(value) => setTab(value as JudgeMode)}
      className={`${paneClass} min-h-[330px] shrink-0 gap-0`}
      aria-label="Results"
    >
      <TabsList className={paneTabsListClass} aria-label="Result sections">
        <TabsTrigger className={paneTabClass} value="run">
          <Play className="size-3.5" /> Run result
        </TabsTrigger>
        <TabsTrigger className={paneTabClass} value="submit">
          <Send className="size-3.5" /> Submission
        </TabsTrigger>
      </TabsList>
      {(["run", "submit"] as const).map((mode) => (
        <TabsContent key={mode} value={mode} className="flex flex-col gap-3.5 overflow-auto px-5 pt-4 pb-5">
          {activeMode === mode ? (
            <p className="flex items-center gap-2 text-[13px] text-muted-foreground" aria-live="polite">
              <LoaderCircle className="size-4 animate-spin" /> {copy[mode].busy}
            </p>
          ) : result && resultMode === mode ? (
            <ResultSummary result={result} />
          ) : (
            <p className="text-[13px] text-muted-foreground">{copy[mode].idle}</p>
          )}
        </TabsContent>
      ))}
    </Tabs>
  )
}

function ResultSummary({ result }: { result: JudgeRunResult }) {
  const tone = verdictTone(result)
  const Icon = toneIcon[tone]

  return (
    <>
      <div
        className="flex flex-wrap items-baseline gap-x-3.5 gap-y-1.5"
        role={result.status === "completed" ? "status" : "alert"}
      >
        <h2 className={cn("inline-flex items-center gap-2 text-[19px] font-semibold", toneClass[tone])}>
          <Icon className="size-5 self-center" /> {verdictLabel(result)}
        </h2>
        <span className="text-[13px] text-muted-foreground">
          {result.message} · {result.durationMs} ms
        </span>
      </div>
      <ResultTable result={result} />
    </>
  )
}

function ResultTable({ result }: { result: JudgeRunResult }) {
  if (result.status !== "completed" || !result.columns?.length) {
    return null
  }
  const rows = result.rows ?? []
  const visibleRows = rows.slice(0, MAX_RENDERED_ROWS)

  return (
    <section aria-label="Query result" className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-muted-foreground">
        {rows.length > visibleRows.length
          ? `Your output · showing ${visibleRows.length} of ${rows.length} rows`
          : `Your output · ${rows.length} row${rows.length === 1 ? "" : "s"}`}
      </span>
      <div className="max-h-72 self-start overflow-auto">
        <table className={sampleTableClass}>
          <thead>
            <tr>
              {result.columns.map((column) => (
                <th key={column.name}>
                  {column.name}
                  <small className="block font-mono text-[11px] font-normal text-muted-foreground">{column.type}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {visibleRows.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {row.map((cell, cellIndex) => {
                  const value = cell === null ? "NULL" : String(cell)
                  return (
                    <td
                      key={cellIndex}
                      title={value}
                      className={cn("max-w-64 truncate", cell === null && "text-muted-foreground italic")}
                    >
                      {value.slice(0, MAX_RENDERED_CELL_LENGTH)}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {visibleRows.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">Query returned no rows.</p>
      ) : null}
    </section>
  )
}
