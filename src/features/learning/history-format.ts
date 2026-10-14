import { relativeTime } from "./problem-status"

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

export function formatDuration(ms: number | null) {
  if (ms === null) return "—"
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`
}

export function shortHash(hash: string) {
  const separator = hash.indexOf(":") + 1
  const prefix = hash.slice(0, separator)
  const body = hash.slice(separator)
  return body.length > 12 ? `${prefix}${body.slice(0, 4)}…${body.slice(-4)}` : hash
}

function dayStart(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()
}

export function formatTimestamp(seconds: number | null, now: number) {
  if (seconds === null) return "Unknown time"
  const date = new Date(seconds * 1000)
  const today = new Date(now * 1000)
  const time = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`
  const days = Math.round((dayStart(today) - dayStart(date)) / 86_400_000)
  if (days === 0) return `Today, ${time}`
  if (days === 1) return `Yesterday, ${time}`
  const day = `${MONTHS[date.getMonth()]} ${date.getDate()}`
  return date.getFullYear() === today.getFullYear()
    ? `${day}, ${time}`
    : `${day}, ${date.getFullYear()}, ${time}`
}

export function whenLabel(seconds: number | null, now: number) {
  if (seconds === null) return "Unknown time"
  const label = relativeTime(seconds, now)
  return label[0].toUpperCase() + label.slice(1)
}
