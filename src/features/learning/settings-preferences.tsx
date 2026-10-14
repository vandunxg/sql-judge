import { useState } from "react"
import { Monitor, Moon, Sun } from "lucide-react"
import { cn } from "cn"

import { useTheme } from "@/components/theme-provider"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { updateSettings, useAppSettings } from "@/lib/app-settings"

import { SettingRow, SettingsSection, SettingSwitch, settingsFocusRing } from "./settings-rows"
import { fontSizeFromInput, FONT_SIZE_RANGE } from "./settings-state"

const themeOptions = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Monitor },
] as const

export function SettingsAppearance() {
  const { theme, setTheme } = useTheme()
  const settings = useAppSettings()

  return (
    <SettingsSection id="appearance" title="Appearance">
      <SettingRow title="Theme" titleId="setting-theme" description="Match your system, or pick one.">
        <ToggleGroup
          aria-labelledby="setting-theme"
          spacing={0.5}
          value={[theme]}
          onValueChange={(next) => {
            const option = themeOptions.find(({ value }) => value === next[0])
            if (option) {
              setTheme(option.value)
            }
          }}
          className="rounded-[10px] bg-muted p-[3px]"
        >
          {themeOptions.map(({ value, label, icon: Icon }) => (
            <ToggleGroupItem
              key={value}
              value={value}
              className={cn(
                "h-[30px] gap-1.5 rounded-[7px] px-3 text-[13px] text-muted-foreground hover:bg-transparent hover:text-foreground focus-visible:ring-0 aria-pressed:bg-card aria-pressed:text-foreground aria-pressed:shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:aria-pressed:shadow-[0_1px_2px_rgba(0,0,0,0.5)] [&_svg:not([class*='size-'])]:size-3.5",
                settingsFocusRing
              )}
            >
              <Icon aria-hidden="true" />
              {label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </SettingRow>
      <SettingRow
        title="Compact tables"
        titleId="setting-compact"
        description="Fit more rows on screen in Problems and History."
      >
        <SettingSwitch
          checked={settings.compactTables}
          onCheckedChange={(compactTables) => updateSettings({ compactTables })}
          labelledBy="setting-compact"
        />
      </SettingRow>
    </SettingsSection>
  )
}

export function SettingsEditor() {
  const settings = useAppSettings()
  const [draftSize, setDraftSize] = useState<string | null>(null)

  function changeSize(text: string) {
    setDraftSize(text)
    const size = fontSizeFromInput(text)
    if (size !== null) {
      updateSettings({ editorFontSize: size })
    }
  }

  return (
    <SettingsSection id="editor" title="Editor">
      <SettingRow
        title="Font size"
        titleId="setting-font-size"
        description="Applies to the SQL editor."
      >
        <div className="flex h-9 items-center overflow-hidden rounded-lg border border-input bg-card focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-ring">
          <input
            type="number"
            inputMode="numeric"
            min={FONT_SIZE_RANGE.min}
            max={FONT_SIZE_RANGE.max}
            step={1}
            aria-labelledby="setting-font-size"
            value={draftSize ?? settings.editorFontSize}
            onChange={(event) => changeSize(event.target.value)}
            onBlur={() => setDraftSize(null)}
            className="h-full w-[72px] [appearance:textfield] bg-transparent px-2.5 text-right font-mono text-[13.5px] outline-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          />
          <span aria-hidden="true" className="flex h-full items-center border-l border-border bg-sidebar px-2.5 text-[13px] text-muted-foreground">
            px
          </span>
        </div>
      </SettingRow>
      <SettingRow
        title="Keep a draft for each problem"
        titleId="setting-keep-drafts"
        description="Your SQL stays put when you switch problems or restart."
      >
        <SettingSwitch
          checked={settings.keepDrafts}
          onCheckedChange={(keepDrafts) => updateSettings({ keepDrafts })}
          labelledBy="setting-keep-drafts"
        />
      </SettingRow>
    </SettingsSection>
  )
}
