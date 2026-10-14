const STORAGE_KEY = "sql-judge-drafts"

export function parseDrafts(raw: string | null): Record<string, string> {
  let value: unknown
  try {
    value = raw ? JSON.parse(raw) : null
  } catch {
    return {}
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return {}
  }
  return Object.fromEntries(
    Object.entries(value).filter((entry): entry is [string, string] => typeof entry[1] === "string")
  )
}

export function loadDrafts() {
  try {
    return parseDrafts(window.localStorage.getItem(STORAGE_KEY))
  } catch {
    return {}
  }
}

export function saveDrafts(drafts: Record<string, string> | null) {
  try {
    if (drafts) {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(drafts))
    } else {
      window.localStorage.removeItem(STORAGE_KEY)
    }
  } catch {
    // Storage can be unavailable; drafts then live only for this session.
  }
}
