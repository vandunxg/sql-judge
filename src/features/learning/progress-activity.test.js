import { describe, expect, test } from "bun:test"

import { activeDaysInMonth, activityCalendar, streaks } from "./progress-activity"

const at = (...parts) => new Date(...parts).getTime() / 1000
const entry = (createdAt, mode = "submit") => ({ createdAt, record: { mode } })

// Wednesday, October 14 2026.
const now = at(2026, 9, 14, 12)

describe("progress activity", () => {
  test("buckets submissions into 26 Sunday-first weeks ending this week", () => {
    const calendar = activityCalendar(
      [
        entry(at(2026, 9, 14, 9)),
        entry(at(2026, 9, 14, 10)),
        entry(at(2026, 9, 13, 9)),
        entry(at(2026, 9, 13, 10), "run"),
        entry(at(2026, 9, 12, 1)),
        entry(at(2026, 9, 12, 2)),
        entry(at(2026, 9, 12, 3)),
        entry(at(2026, 9, 12, 4)),
        entry(at(2025, 0, 1)),
        entry(null),
      ],
      now
    )
    expect(calendar.weeks).toHaveLength(26)
    expect(calendar.weeks.every((week) => week.length === 7)).toBe(true)
    const thisWeek = calendar.weeks[25]
    expect(thisWeek[0].date).toBe("2026-10-11")
    expect(thisWeek.slice(1, 4).map((day) => [day.count, day.level])).toEqual([[4, 3], [1, 1], [2, 2]])
    expect(thisWeek[4].future).toBe(true)
    expect(calendar.weeks[0][0].date).toBe("2026-04-19")
    expect(calendar.total).toBe(7)
  })

  test("labels each month at the week where it starts", () => {
    const { months } = activityCalendar([], now)
    expect(months.map((month) => month.label)).toEqual(["May", "Jun", "Jul", "Aug", "Sep", "Oct"])
    expect(months[0]).toEqual({ label: "May", week: 2 })
  })

  test("measures current and longest daily submission streaks", () => {
    const days = (list) => list.map(([month, day]) => entry(at(2026, month, day, 10)))
    expect(streaks(days([[9, 14], [9, 13], [9, 12], [9, 1], [9, 2], [9, 3], [9, 4], [9, 5]]), now)).toEqual({ current: 3, longest: 5 })
    expect(streaks(days([[9, 13], [9, 12]]), now)).toEqual({ current: 2, longest: 2 })
    expect(streaks(days([[9, 11]]), now)).toEqual({ current: 0, longest: 1 })
    expect(streaks([entry(at(2026, 9, 14, 9), "run")], now)).toEqual({ current: 0, longest: 0 })
  })

  test("counts active submission days in the current month", () => {
    expect(
      activeDaysInMonth([entry(at(2026, 9, 14, 9)), entry(at(2026, 9, 14, 11)), entry(at(2026, 9, 2)), entry(at(2026, 8, 30))], now)
    ).toEqual({ count: 2, month: "October" })
  })
})
