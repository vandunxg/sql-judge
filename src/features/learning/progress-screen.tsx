import { useHistoryData, type HistoryData } from "./history-data"
import { HistoryLoadState, ScreenHeading } from "./history-ui"
import { activeDaysInMonth, activityCalendar, streaks } from "./progress-activity"
import { ActivityCard } from "./progress-activity-card"
import { RecentAcceptedCard, TopicsCard } from "./progress-lists"
import { SolvedCard, TotalsCard } from "./progress-overview"
import { progressSummary } from "./progress-state"

const days = (count: number) => `${count} ${count === 1 ? "day" : "days"}`

export function ProgressScreen() {
  const { data, error, retry } = useHistoryData()

  return (
    <main className="min-h-0 flex-1 overflow-auto px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1120px] flex-col gap-5">
        <div className="mb-1">
          <ScreenHeading title="Progress">
            Counted from accepted submissions on this device. No account, nothing leaves your
            machine.
          </ScreenHeading>
        </div>
        {data ? <ProgressContent {...data} /> : <HistoryLoadState error={error} onRetry={retry} />}
      </div>
    </main>
  )
}

function ProgressContent({ records, entries, packs, loadedAt }: HistoryData) {
  const summary = progressSummary(records, entries, packs)
  const streak = streaks(entries, loadedAt)
  const active = activeDaysInMonth(entries, loadedAt)
  const stats = [
    {
      label: "Acceptance rate",
      value: `${summary.acceptanceRate}%`,
      caption: `${summary.acceptedSubmissions} accepted of ${summary.submissions} ${summary.submissions === 1 ? "submission" : "submissions"}`,
      className: "border-b sm:border-r",
    },
    {
      label: "Current streak",
      value: days(streak.current),
      caption: `Longest so far: ${days(streak.longest)}`,
      className: "border-b",
    },
    {
      label: "Active days",
      value: active.count,
      caption: `in ${active.month}`,
      className: "border-b sm:border-r sm:border-b-0",
    },
    {
      label: "Sample runs",
      value: summary.sampleRuns,
      caption: "not counted toward solved",
      className: "",
    },
  ]

  return (
    <>
      <div className="flex flex-wrap gap-5">
        <SolvedCard summary={summary} />
        <TotalsCard stats={stats} />
      </div>
      <ActivityCard calendar={activityCalendar(entries, loadedAt)} />
      <div className="flex flex-wrap items-start gap-5">
        <TopicsCard summary={summary} />
        <RecentAcceptedCard summary={summary} now={loadedAt} />
      </div>
    </>
  )
}
