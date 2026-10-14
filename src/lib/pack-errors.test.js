import { describe, expect, test } from "bun:test"

import { isPackImportDisabled, readPackDeleteError, readPackImportError } from "./pack-errors"

describe("readPackImportError", () => {
  test("preserves safe actionable native pack errors", () => {
    expect(readPackImportError("Pack source could not be loaded.")).toBe(
      "Pack source could not be loaded."
    )
    expect(
      readPackImportError("Pack runtime validation or publication failed.")
    ).toBe("Pack runtime validation or publication failed.")
    expect(
      readPackImportError({ message: "Pack source could not be loaded." })
    ).toBe("Pack source could not be loaded.")
  })

  test("explains an immutable version conflict", () => {
    const message = "Another pack is already installed with this ID and version."
    expect(readPackImportError(message)).toBe(message)
  })

  test("explains the local PostgreSQL prerequisite", () => {
    expect(
      readPackImportError("Managed PostgreSQL runtime could not be started.")
    ).toContain("initdb")
    expect(
      readPackImportError("Managed PostgreSQL runtime could not be started.")
    ).toContain("pg_ctl")
  })

  test("preserves safe runtime validation coordinates", () => {
    const message =
      "Pack runtime validation failed for eligible-task-filter/basic."
    expect(readPackImportError(message)).toBe(message)
  })

  test("does not expose unknown native error details", () => {
    expect(readPackImportError("/tmp/secret.sql: password=secret")).toBe(
      "Pack import failed closed. No unvalidated pack was published."
    )
    expect(readPackImportError(new Error("raw native error"))).toBe(
      "Pack import failed closed. No unvalidated pack was published."
    )
  })
})

describe("isPackImportDisabled", () => {
  test("disables native pack import outside desktop or while busy", () => {
    expect({
      browser: isPackImportDisabled(false, false),
      desktop: isPackImportDisabled(true, false),
      busy: isPackImportDisabled(true, true),
    }).toEqual({ browser: true, desktop: false, busy: true })
  })
})

describe("pack delete errors", () => {
  test("keeps safe native outcomes and hides everything else", () => {
    const kept = "A pack environment could not be deleted; the pack was kept."
    expect(readPackDeleteError(kept)).toBe(kept)
    expect(readPackDeleteError(new Error("The exact pack is not installed."))).toBe(
      "The exact pack is not installed."
    )
    expect(readPackDeleteError("/tmp/state.sqlite: disk I/O error")).toBe(
      "The pack was not deleted. Try again."
    )
  })
})
