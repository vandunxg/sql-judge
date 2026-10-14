import { FONT_SIZE_RANGE } from "@/lib/app-settings"

export const settingsSections = [
  { id: "appearance", label: "Appearance" },
  { id: "editor", label: "Editor" },
  { id: "judge", label: "Judge limits" },
  { id: "backup", label: "Backup and data" },
  { id: "privacy", label: "Privacy" },
  { id: "about", label: "About" },
] as const

export type SettingsSectionId = (typeof settingsSections)[number]["id"]

export { FONT_SIZE_RANGE }

export function activeSectionId<Id extends string>(
  sections: Array<{ id: Id; top: number }>,
  view: { offset: number; threshold: number; atEnd: boolean }
): Id | null {
  if (view.atEnd && sections.length > 0) {
    return sections[sections.length - 1].id
  }
  const passed = sections.filter((section) => section.top - view.offset <= view.threshold)
  return (passed.at(-1) ?? sections[0])?.id ?? null
}

export function fontSizeFromInput(text: string): number | null {
  const trimmed = text.trim()
  if (!/^\d+$/.test(trimmed)) {
    return null
  }
  const size = Number(trimmed)
  return size >= FONT_SIZE_RANGE.min && size <= FONT_SIZE_RANGE.max ? size : null
}
