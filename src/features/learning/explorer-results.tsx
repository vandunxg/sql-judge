import { useState } from "react"
import { Copy, LoaderCircle } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useAppSettings } from "@/lib/app-settings"

import { ErrorNotice, smallButtonClass } from "./connections-ui"
import { EXPLORER_LIMITS } from "./explorer-api"
import { toCsv } from "./explorer-csv"
import type { ExplorerSession, TimedResult } from "./explorer-use-session"

export function ExplorerResults({ session }: { session: ExplorerSession }) {
  const { queryResult } = session
  const runFailure = session.failure?.step === "run" ? session.failure.message : null
  return (
    <>
      <div className="flex min-h-[45px] flex-wrap items-center gap-x-4 gap-y-2 border-b px-4 py-1.5 text-[13px]">
        <h2 className="font-medium">Results</h2>
        {queryResult ? (
          <span className="text-muted-foreground">
            {queryResult.result.rows.length} {queryResult.result.rows.length === 1 ? "row" : "rows"} ·{" "}
            {queryResult.elapsedMs} ms
          </span>
        ) : null}
        <div className="flex-1" />
        <span className="text-muted-foreground">Showing up to {EXPLORER_LIMITS.maxRows} rows</span>
        {queryResult ? <CopyCsvButton queryResult={queryResult} /> : null}
      </div>
      <div className="min-h-[200px] flex-1 overflow-auto">
        {session.busy === "run" ? (
          <p role="status" className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
            <LoaderCircle className="size-4 animate-spin" aria-hidden="true" /> Running read-only SELECT...
          </p>
        ) : runFailure ? (
          <div className="p-4">
            <ErrorNotice message={runFailure} onRetry={() => void session.run()} retryLabel="Run again" />
          </div>
        ) : !queryResult ? (
          <p className="p-4 text-sm text-muted-foreground">
            {session.verified
              ? "Run a single read-only SELECT to see rows here."
              : "Connect to verify read-only access, then run a SELECT."}
          </p>
        ) : queryResult.result.rows.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">The query returned no rows.</p>
        ) : (
          <ResultGrid queryResult={queryResult} />
        )}
      </div>
    </>
  )
}

function CopyCsvButton({ queryResult }: { queryResult: TimedResult }) {
  const [status, setStatus] = useState<string | null>(null)

  async function copy() {
    const { columns, rows } = queryResult.result
    try {
      await navigator.clipboard.writeText(toCsv(columns.map((column) => column.name), rows))
      setStatus(`Copied ${rows.length} rows`)
    } catch {
      setStatus("Clipboard unavailable")
    }
  }

  return (
    <>
      {status ? (
        <span role="status" className="text-muted-foreground">
          {status}
        </span>
      ) : null}
      <Button variant="ghost" className={smallButtonClass} onClick={() => void copy()}>
        <Copy className="size-3.5" /> Copy as CSV
      </Button>
    </>
  )
}

function ResultGrid({ queryResult }: { queryResult: TimedResult }) {
  const { compactTables } = useAppSettings()
  const { columns, rows } = queryResult.result
  const cell = `border-r border-b font-mono whitespace-nowrap ${compactTables ? "py-1" : "py-[7px]"}`
  return (
    <table className="w-full border-collapse text-[13px]">
      <thead>
        <tr>
          <th scope="col" className="sticky top-0 w-11 border-r border-b bg-sidebar px-2.5 py-2 text-right font-medium text-muted-foreground">
            #
          </th>
          {columns.map((column, columnIndex) => (
            <th
              key={columnIndex}
              scope="col"
              className="sticky top-0 border-r border-b bg-sidebar px-3.5 py-2 text-left font-medium whitespace-nowrap"
            >
              {column.name}
              <small className="block font-mono text-[11px] font-normal text-muted-foreground">{column.type}</small>
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, rowIndex) => (
          <tr key={rowIndex} className="hover:[&>td]:bg-muted">
            <td className={`${cell} px-2.5 text-right text-muted-foreground`}>{rowIndex + 1}</td>
            {row.map((value, columnIndex) => (
              <td key={columnIndex} className={`${cell} px-3.5`}>
                {value === null ? <span className="text-muted-foreground italic">NULL</span> : String(value)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}
