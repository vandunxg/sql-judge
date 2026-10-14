import { ArrowRight, Check } from "lucide-react"

import { cn } from "@/lib/utils"

import { whenLabel } from "./history-format"
import { historyCardClass } from "./history-ui"
import { difficultyText, ProgressBar, progressTitleClass } from "./progress-overview"
import type { ProgressSummary } from "./progress-state"

const TOPIC_LIMIT = 8

export function TopicsCard({ summary }: { summary: ProgressSummary }) {
  const topics = summary.topics.slice(0, TOPIC_LIMIT)
  const hidden = summary.topics.length - topics.length
  return (
    <section aria-labelledby="topic-h" className={cn(historyCardClass, "flex flex-[1_1_420px] flex-col gap-3.5 px-6 py-5")}>
      <div className="flex items-baseline justify-between">
        <h2 id="topic-h" className={progressTitleClass}>By topic</h2>
        <span className="text-[12.5px] text-muted-foreground">solved / total</span>
      </div>
      {topics.length > 0 ? (
        <ul className="m-0 flex list-none flex-col gap-3.5 p-0">
          {topics.map((topic) => (
            <li key={topic.name} className="grid grid-cols-[140px_1fr_52px] items-center gap-3 text-[13px]">
              <span className="truncate" title={topic.name}>{topic.name}</span>
              <ProgressBar value={topic.solved} max={topic.total} minPercent={2} className="bg-foreground" />
              <span className="text-right font-mono text-muted-foreground">
                {topic.solved} / {topic.total}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-[13px] text-muted-foreground">Import a problem pack to see topics here.</p>
      )}
      {hidden > 0 ? <p className="text-[12.5px] text-muted-foreground">{hidden} smaller topics not shown</p> : null}
      {summary.suggestion ? (
        <a
          href={summary.suggestion.href}
          className="mt-1 flex items-center justify-between gap-3 rounded-lg bg-muted px-3.5 py-3 text-[13px] hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        >
          <span>
            <span className="text-muted-foreground">Room to grow in {summary.suggestion.topic}. Try </span>
            <span className="min-w-0 truncate font-medium" title={summary.suggestion.title}>{summary.suggestion.title}</span>
          </span>
          <ArrowRight className="size-4 shrink-0" aria-hidden="true" />
        </a>
      ) : null}
    </section>
  )
}

const rowClass = "flex items-center gap-3 border-t px-5 py-[11px] text-sm"

export function RecentAcceptedCard({ summary, now }: { summary: ProgressSummary; now: number }) {
  return (
    <section aria-labelledby="recent-h" className={cn(historyCardClass, "flex-[1_1_420px] overflow-hidden")}>
      <div className="flex items-baseline justify-between px-5 pt-[18px] pb-3">
        <h2 id="recent-h" className={progressTitleClass}>Recently accepted</h2>
        <a href="#history" className="rounded-sm text-[13px] font-medium text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none">
          All history
        </a>
      </div>
      {summary.recentAccepted.length === 0 ? (
        <p className="border-t px-5 py-4 text-[13px] text-muted-foreground">Accepted submissions will appear here.</p>
      ) : null}
      {summary.recentAccepted.map((entry) => {
        const content = (
          <>
            <Check className="size-4 shrink-0 text-ok" role="img" aria-label="Accepted" />
            <span className="min-w-0 flex-1 truncate font-medium">
              {entry.title}
              {entry.problemHref ? null : <span className="font-normal text-muted-foreground"> · {entry.packLabel}</span>}
            </span>
            {entry.difficulty ? (
              <span className={cn("text-[13px] capitalize", difficultyText[entry.difficulty])}>{entry.difficulty}</span>
            ) : null}
            <span className="w-[84px] shrink-0 text-right text-[13px] text-muted-foreground">{whenLabel(entry.createdAt, now)}</span>
          </>
        )
        return entry.problemHref ? (
          <a key={entry.id} href={entry.problemHref} className={cn(rowClass, "hover:bg-accent focus-visible:bg-accent focus-visible:outline-none")}>
            {content}
          </a>
        ) : (
          <div key={entry.id} className={rowClass}>{content}</div>
        )
      })}
    </section>
  )
}
