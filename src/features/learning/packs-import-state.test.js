import { describe, expect, test } from "bun:test"

import { readPackImportError } from "@/lib/pack-errors"
import {
  importCheckStatuses,
  importFailureStage,
  importReportSummary,
  sourceName,
} from "./packs-import-state"

describe("import failure stage", () => {
  test("maps sanitized native errors to the stage that failed", () => {
    expect(importFailureStage(readPackImportError("Pack source could not be loaded."))).toBe("source")
    expect(importFailureStage(readPackImportError("Pack failed structural validation."))).toBe("structure")
    expect(importFailureStage(readPackImportError("Managed PostgreSQL runtime could not be started."))).toBe("runtime")
    expect(importFailureStage(readPackImportError("Managed environment state is busy."))).toBe("runtime")
    expect(importFailureStage(readPackImportError("Pack runtime validation failed for p1/t2."))).toBe("reference")
    expect(importFailureStage(readPackImportError("Pack runtime validation or publication failed."))).toBe("reference")
  })

  test("does not guess a stage for unknown failures", () => {
    expect(importFailureStage(readPackImportError(new Error("secret postgres://u:p@h")))).toBeNull()
  })
})

describe("import check statuses", () => {
  test("marks stages before the failure as passed and later ones as skipped", () => {
    expect(importCheckStatuses("failed", "runtime")).toEqual({
      source: "passed",
      structure: "passed",
      runtime: "failed",
      reference: "skipped",
    })
  })

  test("never claims a pass when the failing stage is unknown", () => {
    expect(Object.values(importCheckStatuses("failed", null))).toEqual([
      "unknown",
      "unknown",
      "unknown",
      "unknown",
    ])
  })

  test("follows the import lifecycle", () => {
    expect(importCheckStatuses("ready", null).source).toBe("pending")
    expect(importCheckStatuses("installing", null).reference).toBe("running")
    expect(Object.values(importCheckStatuses("installed", null))).toEqual([
      "passed",
      "passed",
      "passed",
      "passed",
    ])
  })

  test("summarises the report", () => {
    expect(importReportSummary(importCheckStatuses("ready", null))).toBe("4 checks before install")
    expect(importReportSummary(importCheckStatuses("installing", null))).toBe("Checking…")
    expect(importReportSummary(importCheckStatuses("installed", null))).toBe("4 passed")
    expect(importReportSummary(importCheckStatuses("failed", "structure"))).toBe("1 passed · 1 failed")
    expect(importReportSummary(importCheckStatuses("failed", null))).toBe("Not completed")
  })
})

describe("source name", () => {
  test("shows the last path segment for posix and windows paths", () => {
    expect(sourceName("/home/me/Downloads/wf.sqljudge.zip")).toBe("wf.sqljudge.zip")
    expect(sourceName("C:\\packs\\hr-sql-101\\")).toBe("hr-sql-101")
    expect(sourceName("  ")).toBe("")
  })
})
