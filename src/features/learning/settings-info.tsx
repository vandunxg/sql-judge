import { useEffect, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { isTauri } from "@tauri-apps/api/core"

import { SettingRow, SettingsSection } from "./settings-rows"

export function SettingsJudgeLimits() {
  const readOnly = <span className="text-[13px] text-muted-foreground">Set by each problem</span>
  return (
    <SettingsSection id="judge" title="Judge limits">
      <SettingRow
        title="Statement timeout"
        description="Each problem's pack sets it, within a cap the native judge enforces. Queries that run longer stop with “Time limit exceeded”."
      >
        {readOnly}
      </SettingRow>
      <SettingRow
        title="Row and result size limits"
        description="Each problem's pack sets them, within the judge's caps. Larger results stop with “Result limit exceeded” and are never accepted."
      >
        {readOnly}
      </SettingRow>
    </SettingsSection>
  )
}

const privacyStatements = [
  {
    title: "History",
    description: "Submission history keeps verdicts, timings and pack versions. SQL text, passwords and connection strings are never stored in it.",
  },
  {
    title: "Passwords",
    description: "Saved connection passwords live in your OS keychain. If the keychain is unavailable, they are kept in memory for this session only.",
  },
  {
    title: "Settings and drafts",
    description: "Your settings and SQL drafts stay on this device.",
  },
]

export function SettingsPrivacy() {
  return (
    <SettingsSection id="privacy" title="Privacy">
      {privacyStatements.map((statement) => (
        <SettingRow key={statement.title} title={statement.title} description={statement.description} />
      ))}
    </SettingsSection>
  )
}

export function SettingsAbout() {
  const [version, setVersion] = useState<string | null>(null)

  useEffect(() => {
    if (!isTauri()) {
      return
    }
    let active = true
    void getVersion()
      .then((nextVersion) => active && setVersion(nextVersion))
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [])

  return (
    <SettingsSection id="about" title="About">
      <SettingRow
        title={version ? `SQL Judge ${version}` : "SQL Judge"}
        description={version ? "Desktop app" : "Version is shown in the desktop app."}
      />
      <SettingRow
        title="Pack authoring"
        description={
          <>
            Guide: <code className="font-mono text-[12.5px]">docs/agents/AI-PACK-GENERATION.md</code>. JSON schemas:{" "}
            <code className="font-mono text-[12.5px]">schemas/pack.schema.json</code> and{" "}
            <code className="font-mono text-[12.5px]">schemas/problem.schema.json</code>.
          </>
        }
      />
    </SettingsSection>
  )
}
