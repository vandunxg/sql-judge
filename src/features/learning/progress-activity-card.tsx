import { cn } from "@/lib/utils"

import { historyCardClass } from "./history-ui"
import type { activityCalendar, ActivityDay } from "./progress-activity"
import { progressTitleClass } from "./progress-overview"

// The design's heatmap ramp has no shared token yet; values come from the Progress export.
const emptyCell = "bg-[#eeeef1] dark:bg-[#1d1d22]"
const heatCell = "bg-[#15803d] dark:bg-[#22c55e]"
const levelClass: Record<ActivityDay["level"], string> = {
  0: emptyCell,
  1: cn(heatCell, "opacity-35"),
  2: cn(heatCell, "opacity-60"),
  3: heatCell,
}
const CELL = "size-[13px] rounded-[3px]"
const WEEK_WIDTH = 16

type Calendar = ReturnType<typeof activityCalendar>

export function ActivityCard({ calendar }: { calendar: Calendar }) {
  const weeks = calendar.weeks.length
  return (
    <section aria-labelledby="act-h" className={cn(historyCardClass, "flex flex-col gap-3.5 px-6 py-5")}>
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="act-h" className={progressTitleClass}>
          {calendar.total} {calendar.total === 1 ? "submission" : "submissions"} in the last 6 months
        </h2>
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground" aria-hidden="true">
          Less
          {([0, 1, 2, 3] as const).map((level) => (
            <span key={level} className={cn(CELL, levelClass[level])} />
          ))}
          More
        </div>
      </div>
      <div className="overflow-x-auto contain-inline-size">
        <div className="inline-flex flex-col gap-1.5">
          <div className="relative h-[18px] text-xs text-muted-foreground" style={{ width: weeks * WEEK_WIDTH }} aria-hidden="true">
            {calendar.months.map((month) => (
              <span key={`${month.label}-${month.week}`} className="absolute top-0" style={{ left: month.week * WEEK_WIDTH }}>
                {month.label}
              </span>
            ))}
          </div>
          <div role="img" aria-label={`Daily submission activity, last ${weeks} weeks`} className="flex gap-[3px]">
            {calendar.weeks.map((week) => (
              <div key={week[0].date} className="flex flex-col gap-[3px]">
                {week.map((day) => (
                  <span
                    key={day.date}
                    className={cn(CELL, levelClass[day.level])}
                    title={day.future ? undefined : `${day.count} ${day.count === 1 ? "submission" : "submissions"} on ${day.date}`}
                  />
                ))}
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
