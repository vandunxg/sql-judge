import { describe, expect, test } from "bun:test"

import { problemStatuses } from "./problem-status"
import {
  packEnvironmentBadge,
  packMonogram,
  packProgress,
  packTableCount,
  shortDigest,
} from "./packs-state"

const digest = "9f2c" + "0".repeat(56) + "a71e"
const pack = {
  packId: "hr",
  version: "1.2.0",
  digest,
  title: "HR Analytics",
  dialect: "postgresql",
  problems: [
    { id: "p1", schema: [{ name: "employees", columns: [] }, { name: "departments", columns: [] }] },
    { id: "p2", schema: [{ name: "employees", columns: [] }] },
    { id: "p3" },
  ],
}

function record(overrides) {
  return {
    submissionId: crypto.randomUUID(),
    packId: "hr",
    packVersion: "1.2.0",
    packHash: digest,
    engineVersion: "1",
    policyVersion: "1",
    verdict: "Accepted",
    createdAt: "100",
    problemId: "p1",
    mode: "submit",
    tests: [],
    ...overrides,
  }
}

function env(overrides) {
  return {
    envId: crypto.randomUUID(),
    instanceId: "i",
    packId: "hr",
    packVersion: "1.2.0",
    packDigest: digest,
    namespace: "ns",
    mode: "managed",
    state: "ready",
    ...overrides,
  }
}

describe("pack progress", () => {
  test("counts distinct accepted submits for this pack version only", () => {
    const statuses = problemStatuses([
      record({}),
      record({ createdAt: "200" }),
      record({ problemId: "p2", verdict: "WrongAnswer" }),
      record({ problemId: "p3", mode: "run" }),
      record({ problemId: "p2", packVersion: "1.1.0" }),
      record({ problemId: "p3", packHash: "other" }),
      record({ problemId: "gone" }),
    ])
    expect(packProgress(pack, statuses)).toEqual({ solved: 1, total: 3, percent: 33 })
  })

  test("reports zero percent for a pack without problems", () => {
    expect(packProgress({ ...pack, problems: [] }, new Map())).toEqual({
      solved: 0,
      total: 0,
      percent: 0,
    })
  })
})

describe("pack metadata", () => {
  test("counts distinct tables across problem schemas", () => {
    expect(packTableCount(pack)).toBe(2)
  })

  test("shortens a sha256 digest for display", () => {
    expect(shortDigest(digest)).toBe("sha256:9f2c…a71e")
    expect(shortDigest("abc")).toBe("sha256:abc")
  })

  test("builds a two-letter monogram", () => {
    expect(packMonogram("HR Analytics")).toBe("HR")
    expect(packMonogram("E-Commerce Orders")).toBe("EC")
    expect(packMonogram("window functions")).toBe("WF")
    expect(packMonogram("Joins")).toBe("JO")
    expect(packMonogram("")).toBe("?")
  })
})

describe("pack environment badge", () => {
  test("only counts managed environments for the exact pack identity", () => {
    expect(
      packEnvironmentBadge(pack, [
        env({}),
        env({ packDigest: "other" }),
        env({ mode: "external_explorer" }),
        env({ packId: "ec" }),
      ])
    ).toEqual({ label: "Environment ready", tone: "ok" })
  })

  test("summarises several environments with the worst tone", () => {
    expect(packEnvironmentBadge(pack, [env({}), env({})])).toEqual({
      label: "2 environments",
      tone: "ok",
    })
    expect(
      packEnvironmentBadge(pack, [env({}), env({ state: "needs_recovery" })])
    ).toEqual({ label: "2 environments", tone: "warn" })
  })

  test("labels single non-ready environments without colour alone", () => {
    expect(packEnvironmentBadge(pack, [env({ state: "error" })])).toEqual({
      label: "Environment error",
      tone: "err",
    })
    expect(packEnvironmentBadge(pack, [env({ state: "provisioning" })])).toEqual({
      label: "Environment provisioning",
      tone: "muted",
    })
  })

  test("reports a pack without environments", () => {
    expect(packEnvironmentBadge(pack, [])).toEqual({
      label: "No environment",
      tone: "muted",
    })
  })
})
