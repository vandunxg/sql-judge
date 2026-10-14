import { useSyncExternalStore } from "react"

export type AppSettings = {
  compactTables: boolean
  editorFontSize: number
  keepDrafts: boolean
}

export const defaultSettings: AppSettings = {
  compactTables: false,
  editorFontSize: 14,
  keepDrafts: true,
}

const STORAGE_KEY = "sql-judge-settings"
export const FONT_SIZE_RANGE = { min: 11, max: 22 }

export function parseSettings(raw: string | null): AppSettings {
  let value: unknown
  try {
    value = raw ? JSON.parse(raw) : null
  } catch {
    return defaultSettings
  }
  if (typeof value !== "object" || value === null) {
    return defaultSettings
  }
  const stored = value as Record<string, unknown>
  const bool = (key: "compactTables" | "keepDrafts") =>
    typeof stored[key] === "boolean" ? stored[key] : defaultSettings[key]
  const fontSize =
    typeof stored.editorFontSize === "number" && Number.isFinite(stored.editorFontSize)
      ? Math.min(FONT_SIZE_RANGE.max, Math.max(FONT_SIZE_RANGE.min, Math.round(stored.editorFontSize)))
      : defaultSettings.editorFontSize

  return {
    compactTables: bool("compactTables"),
    editorFontSize: fontSize,
    keepDrafts: bool("keepDrafts"),
  }
}

function readStorage() {
  try {
    return window.localStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

let current = parseSettings(readStorage())
const listeners = new Set<() => void>()

export function updateSettings(patch: Partial<AppSettings>) {
  current = parseSettings(JSON.stringify({ ...current, ...patch }))
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(current))
  } catch {
    // Storage can be unavailable; the in-memory value still applies this session.
  }
  listeners.forEach((listener) => listener())
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useAppSettings() {
  return useSyncExternalStore(subscribe, () => current)
}
