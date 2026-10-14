import { describe, expect, test } from "bun:test"

import {
  confirmationMatches,
  environmentBlockedReason,
  environmentProfileSelection,
  environmentStatus,
  isManagedNamespace,
  isCurrentEnvironmentLoad,
  namespaceError,
  packKey,
  packLabel,
  environmentSourceLabel,
  postgresqlProfiles,
  uniqueNamespace,
} from "./environments-model"

const environment = {
  envId: "sj_hr_practice",
  instanceId: "instance-1",
  packId: "hr-sql-101",
  packVersion: "1.2.0",
  packDigest: "a".repeat(64),
  namespace: "sj_hr_practice",
  mode: "managed",
  state: "ready",
}

const pack = {
  packId: "hr-sql-101",
  version: "1.2.0",
  title: "HR Analytics",
  dialect: "postgresql",
  digest: "a".repeat(64),
  problems: [],
}

describe("environments model", () => {
  test("keys packs by exact id, version and digest", () => {
    expect(packKey(pack)).toBe(`hr-sql-101@1.2.0#${"a".repeat(64)}`)
  })

  test("labels a pack only when id, version and digest all match", () => {
    expect(packLabel(environment, [pack])).toBe("HR Analytics")
    expect(packLabel(environment, [{ ...pack, digest: "b".repeat(64) }])).toBe(
      "hr-sql-101"
    )
    expect(packLabel(environment, [{ ...pack, version: "1.3.0" }])).toBe(
      "hr-sql-101"
    )
  })

  test("mirrors the native managed namespace rule", () => {
    expect(isManagedNamespace("sj_hr_practice")).toBe(true)
    expect(isManagedNamespace("sj_2")).toBe(true)
    expect(isManagedNamespace("public")).toBe(false)
    expect(isManagedNamespace("sj_HR")).toBe(false)
    expect(isManagedNamespace("sj_hr-practice")).toBe(false)
    expect(isManagedNamespace(`sj_${"a".repeat(61)}`)).toBe(false)
    expect(isManagedNamespace("")).toBe(false)
  })

  test("derives a free replacement namespace without stacking suffixes", () => {
    expect(uniqueNamespace("sj_hr_practice", "r", [])).toBe("sj_hr_practice_r1")
    expect(
      uniqueNamespace("sj_hr_practice_r1", "r", [
        "sj_hr_practice_r1",
        "sj_hr_practice_r2",
      ])
    ).toBe("sj_hr_practice_r3")
    expect(
      uniqueNamespace("sj_hr_practice", "copy", ["sj_hr_practice_copy1"])
    ).toBe("sj_hr_practice_copy2")
  })

  test("keeps derived namespaces within the native length limit", () => {
    const long = `sj_${"a".repeat(60)}`
    const next = uniqueNamespace(long, "r", [long])
    expect(next.length).toBeLessThanOrEqual(63)
    expect(isManagedNamespace(next)).toBe(true)
    expect(next).not.toBe(long)
  })

  test("requires the exact namespace to confirm", () => {
    expect(confirmationMatches("sj_hr_practice", "sj_hr_practice")).toBe(true)
    expect(confirmationMatches(" sj_hr_practice ", "sj_hr_practice")).toBe(true)
    expect(confirmationMatches("sj_hr_practic", "sj_hr_practice")).toBe(false)
    expect(confirmationMatches("SJ_HR_PRACTICE", "sj_hr_practice")).toBe(false)
    expect(confirmationMatches("", "")).toBe(false)
  })

  test("maps lifecycle states to readable status tones", () => {
    expect(environmentStatus("ready")).toEqual({
      label: "Ready",
      tone: "ok",
      pending: false,
    })
    expect(environmentStatus("running")).toEqual({
      label: "Judging",
      tone: "info",
      pending: true,
    })
    expect(environmentStatus("needs_recovery").tone).toBe("warn")
    expect(environmentStatus("error").tone).toBe("err")
    expect(environmentStatus("rebuilding").pending).toBe(true)
  })

  test("explains why lifecycle actions are blocked", () => {
    expect(environmentBlockedReason(environment, false)).toBeNull()
    expect(environmentBlockedReason(environment, true)).toBe(
      "Another environment action is in progress."
    )
    expect(
      environmentBlockedReason({ ...environment, state: "running" }, false)
    ).toBe("Available after the current judge finishes.")
    expect(
      environmentBlockedReason(
        { ...environment, state: "needs_recovery" },
        false
      )
    ).toBe(
      "Interrupted operation needs recovery before this schema can change."
    )
    expect(
      environmentBlockedReason(
        { ...environment, mode: "external_explorer" },
        false
      )
    ).toBe("Only schemas owned by SQL Judge can change.")
    expect(
      environmentBlockedReason(
        { ...environment, mode: "external_sandbox" },
        false
      )
    ).toBeNull()
  })

  test("rejects invalid or already used namespaces before a native call", () => {
    expect(namespaceError("sj_new", [environment])).toBeNull()
    expect(namespaceError("public", [environment])).toBe(
      "Use a namespace that starts with sj_ and has only lowercase letters, digits and underscores."
    )
    expect(namespaceError("sj_hr_practice", [environment])).toBe(
      "Another environment already uses this namespace."
    )
  })

  test("offers only PostgreSQL profiles and keeps local as the fallback source", () => {
    const profiles = [
      { profileId: "pg-a", displayName: "Training PG", engine: "postgresql" },
      {
        profileId: "sql-a",
        displayName: "Reporting SQL Server",
        engine: "sqlserver",
      },
    ]
    expect(postgresqlProfiles(profiles)).toEqual([profiles[0]])
    expect(environmentSourceLabel(null, profiles)).toBe("Local runtime")
    expect(environmentSourceLabel("pg-a", profiles)).toBe("Training PG")
  })

  test("ignores stale environment loads", () => {
    expect(isCurrentEnvironmentLoad(2, 2)).toBe(true)
    expect(isCurrentEnvironmentLoad(1, 2)).toBe(false)
  })

  test("falls back to local when the selected profile disappears", () => {
    expect(
      environmentProfileSelection("pg-a", [
        { profileId: "pg-b", displayName: "Other PG", engine: "postgresql" },
      ])
    ).toBe(null)
    expect(
      environmentProfileSelection("pg-a", [
        { profileId: "pg-a", displayName: "Training PG", engine: "postgresql" },
      ])
    ).toBe("pg-a")
  })
})
