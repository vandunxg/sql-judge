type ActivityEntry = { createdAt: number | null; record: { mode: "run" | "submit" } }

export type ActivityDay = { date: string; count: number; level: 0 | 1 | 2 | 3; future: boolean }

const WEEKS = 26
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

function dateKey(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0")
  return `${date.getFullYear()}-${month}-${String(date.getDate()).padStart(2, "0")}`
}

function submitDays(entries: ActivityEntry[]) {
  const counts = new Map<string, number>()
  for (const entry of entries) {
    if (entry.record.mode !== "submit" || entry.createdAt === null) continue
    const key = dateKey(new Date(entry.createdAt * 1000))
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return counts
}

function addDays(date: Date, days: number) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days)
}

function level(count: number): ActivityDay["level"] {
  if (count === 0) return 0
  if (count === 1) return 1
  return count < 4 ? 2 : 3
}

export function activityCalendar(entries: ActivityEntry[], now: number) {
  const counts = submitDays(entries)
  const today = new Date(now * 1000)
  const todayKey = dateKey(today)
  const firstDay = addDays(today, -today.getDay() - (WEEKS - 1) * 7)
  let total = 0
  let future = false
  const weeks: ActivityDay[][] = []
  const months: Array<{ label: string; week: number }> = []

  for (let week = 0; week < WEEKS; week += 1) {
    const sunday = addDays(firstDay, week * 7)
    if (week > 0 && sunday.getMonth() !== addDays(sunday, -7).getMonth()) {
      months.push({ label: MONTHS[sunday.getMonth()], week })
    }
    weeks.push(
      Array.from({ length: 7 }, (_, offset) => {
        const date = dateKey(addDays(sunday, offset))
        const count = future ? 0 : (counts.get(date) ?? 0)
        total += count
        const day = { date, count, level: level(count), future }
        future ||= date === todayKey
        return day
      })
    )
  }
  return { weeks, months, total }
}

export function streaks(entries: ActivityEntry[], now: number) {
  const days = submitDays(entries)
  const today = new Date(now * 1000)
  let longest = 0
  let run = 0
  for (const key of [...days.keys()].sort()) {
    const [year, month, day] = key.split("-").map(Number)
    const previous = dateKey(new Date(year, month - 1, day - 1))
    run = days.has(previous) ? run + 1 : 1
    longest = Math.max(longest, run)
  }
  let current = 0
  let cursor = days.has(dateKey(today)) ? today : addDays(today, -1)
  while (days.has(dateKey(cursor))) {
    current += 1
    cursor = addDays(cursor, -1)
  }
  return { current, longest }
}

export function activeDaysInMonth(entries: ActivityEntry[], now: number) {
  const today = new Date(now * 1000)
  const prefix = dateKey(today).slice(0, 8)
  const count = [...submitDays(entries).keys()].filter((key) => key.startsWith(prefix)).length
  return { count, month: today.toLocaleString("en-US", { month: "long" }) }
}
