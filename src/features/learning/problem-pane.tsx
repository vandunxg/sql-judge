import type { ReactNode } from "react"
import { FileText, Table2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

import { hasSampleData, sampleColumns, type LearningProblem } from "./catalog"
import { DifficultyBadge } from "./difficulty-badge"
import {
  paneClass,
  paneTabClass,
  paneTabsListClass,
  sampleTableClass,
} from "./solve-pane"

export const dialectLabels = { postgresql: "PostgreSQL", sqlserver: "SQL Server" }

type ProblemPaneProps = {
  problem: LearningProblem | null
  packLabel: string
  notice: ReactNode
}

export function ProblemPane({ problem, packLabel, notice }: ProblemPaneProps) {
  return (
    <Tabs defaultValue="description" className={`${paneClass} flex-1 gap-0`} aria-label="Problem">
      <TabsList className={paneTabsListClass} aria-label="Problem sections">
        <TabsTrigger className={paneTabClass} value="description">
          <FileText className="size-3.5" /> Description
        </TabsTrigger>
        <TabsTrigger className={paneTabClass} value="schema">
          <Table2 className="size-3.5" /> Schema
        </TabsTrigger>
      </TabsList>
      <TabsContent value="description" className="flex min-h-0 flex-col gap-[22px] overflow-auto px-6 pt-[22px] pb-7">
        {problem ? <Description problem={problem} packLabel={packLabel} /> : notice}
      </TabsContent>
      <TabsContent value="schema" className="flex min-h-0 flex-col gap-3 overflow-auto px-6 pt-[22px] pb-7">
        {problem && problem.schema.length > 0 ? (
          problem.schema.map((table) => (
            <table key={table.name} className={`${sampleTableClass} self-start`}>
              <caption className="pb-1.5 text-left font-mono text-xs text-muted-foreground">{table.name}</caption>
              <thead><tr><th>column</th><th>type</th></tr></thead>
              <tbody>
                {table.columns.map((column) => (
                  <tr key={column.name}>
                    <td>{column.name}</td>
                    <td className="text-muted-foreground">{column.type}{column.nullable ? " · nullable" : ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ))
        ) : (
          <p className="text-[13px] text-muted-foreground">Schema metadata is unavailable for this problem.</p>
        )}
      </TabsContent>
    </Tabs>
  )
}

function Description({ problem, packLabel }: { problem: LearningProblem; packLabel: string }) {
  const columns = sampleColumns(problem)

  return (
    <>
      <div className="flex flex-col gap-3">
        <h1 className="text-[21px] leading-[1.3] font-semibold tracking-[-0.015em]">{problem.title}</h1>
        <div className="flex flex-wrap items-center gap-1.5">
          <DifficultyBadge difficulty={problem.difficulty} />
          {problem.topics.map((topic) => <Badge key={topic} variant="outline">{topic}</Badge>)}
        </div>
      </div>
      <p className="max-w-[64ch] text-[14.5px] leading-[1.65] whitespace-pre-line">{problem.statement}</p>
      {columns.length > 0 ? (
        <section className="flex flex-col gap-2.5">
          <h3 className="text-sm font-semibold">Return columns</h3>
          <div className="flex flex-wrap gap-2">
            {columns.map((column) => <Badge key={column} variant="outline" className="font-mono font-normal">{column}</Badge>)}
          </div>
        </section>
      ) : null}
      <section className="flex flex-col gap-2.5">
        <h3 className="text-sm font-semibold">Example output</h3>
        {hasSampleData(problem) ? (
          <table className={`${sampleTableClass} self-start`}>
            <thead><tr>{columns.map((column) => <th key={column}>{column}</th>)}</tr></thead>
            <tbody>
              {problem.sampleRows.map((row, index) => (
                <tr key={index}>
                  {columns.map((column) => (
                    <td key={column} className={row[column] === null ? "text-muted-foreground italic" : undefined}>
                      {String(row[column] ?? "NULL")}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="text-[13px] text-muted-foreground">Public sample output is unavailable for this problem.</p>
        )}
      </section>
      <div className="mt-auto flex flex-wrap gap-x-4 gap-y-1.5 border-t pt-3.5 text-[12.5px] text-muted-foreground">
        <span>{packLabel}</span>
        <span>{dialectLabels[problem.dialect]}</span>
      </div>
    </>
  )
}
