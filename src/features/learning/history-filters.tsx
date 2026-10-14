import { Search } from "lucide-react"

import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { JudgeVerdict } from "@/lib/judge-api"

import type { HistoryFilters, HistoryRange, HistoryTab } from "./history-state"
import { verdictLabel } from "./verdict-display"

const tabs: Array<[HistoryTab, string]> = [
  ["all", "All"],
  ["submit", "Submissions"],
  ["run", "Sample runs"],
]

const ranges: Array<[HistoryRange, string]> = [
  ["1d", "Last 24 hours"],
  ["7d", "Last 7 days"],
  ["30d", "Last 30 days"],
  ["all", "All time"],
]

const verdicts: JudgeVerdict[] = [
  "Accepted",
  "WrongAnswer",
  "SyntaxError",
  "RuntimeError",
  "PolicyViolation",
  "TimeLimitExceeded",
  "ResultLimitExceeded",
  "Cancelled",
  "JudgeError",
]

const label = (verdict: JudgeVerdict) =>
  verdictLabel({ status: "completed", verdict })

const triggerClass =
  "h-9 gap-2 rounded-lg border-border bg-card px-3.5 text-sm font-medium shadow-[0_1px_2px_rgba(16,16,24,0.05)] hover:bg-accent dark:bg-card dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)] dark:hover:bg-accent"

type Props = {
  filters: HistoryFilters
  onChange: (patch: Partial<HistoryFilters>) => void
}

export function HistoryFilterBar({ filters, onChange }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <Tabs
        value={filters.tab}
        onValueChange={(tab) => onChange({ tab: tab as HistoryTab })}
      >
        <TabsList
          aria-label="Kind"
          className="h-auto gap-0.5 rounded-[10px] p-[3px]"
        >
          {tabs.map(([value, text]) => (
            <TabsTrigger
              key={value}
              value={value}
              className="h-[30px] flex-none rounded-[7px] border-0 px-3 text-[13px] text-muted-foreground data-active:text-foreground data-active:shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:data-active:bg-card"
            >
              {text}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
      <label className="flex h-9 min-w-0 flex-[1_1_220px] items-center gap-2 rounded-lg border border-input bg-card px-3 shadow-[0_1px_2px_rgba(16,16,24,0.05)] focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]">
        <Search
          className="size-4 shrink-0 text-muted-foreground"
          aria-hidden="true"
        />
        <Input
          type="search"
          value={filters.query}
          onChange={(event) => onChange({ query: event.target.value })}
          placeholder="Search problems"
          aria-label="Search history"
          className="h-auto min-w-0 flex-1 border-0 bg-transparent px-0 text-sm shadow-none focus-visible:ring-0"
        />
      </label>
      <Select
        value={filters.verdict}
        onValueChange={(verdict) => verdict && onChange({ verdict })}
      >
        <SelectTrigger className={triggerClass} aria-label="Filter by verdict">
          <SelectValue>
            {(value: HistoryFilters["verdict"]) =>
              value === "all" ? "Verdict" : label(value)
            }
          </SelectValue>
        </SelectTrigger>
        <SelectContent
          align="start"
          alignItemWithTrigger={false}
          className="w-auto min-w-(--anchor-width)"
        >
          <SelectItem value="all">All verdicts</SelectItem>
          {verdicts.map((verdict) => (
            <SelectItem key={verdict} value={verdict}>
              {label(verdict)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Select
        value={filters.range}
        onValueChange={(range) => range && onChange({ range })}
      >
        <SelectTrigger className={triggerClass} aria-label="Time range">
          <SelectValue>
            {(value: HistoryRange) =>
              ranges.find(([key]) => key === value)?.[1]
            }
          </SelectValue>
        </SelectTrigger>
        <SelectContent
          align="end"
          alignItemWithTrigger={false}
          className="w-auto min-w-(--anchor-width)"
        >
          {ranges.map(([value, text]) => (
            <SelectItem key={value} value={value}>
              {text}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}
