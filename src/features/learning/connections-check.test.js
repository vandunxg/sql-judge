import { describe, expect, test } from "bun:test"

import { profileCheckFromInfo } from "./connections-check"

const info = { database: "analytics", username: "reader", serverVersion: "PostgreSQL 16.4", readOnly: true }
const checkedAt = new Date("2026-10-14T10:00:00Z")

describe("connection test outcome", () => {
  test("accepts only a backend-verified read-only connection", () => {
    expect({
      verified: profileCheckFromInfo(info, 84, checkedAt),
      unverified: profileCheckFromInfo({ ...info, readOnly: false }, 84, checkedAt),
    }).toEqual({
      verified: { ok: true, info, elapsedMs: 84, checkedAt },
      unverified: {
        ok: false,
        message:
          "The backend did not verify a read-only connection. Metadata and SELECT actions are disabled.",
      },
    })
  })
})
