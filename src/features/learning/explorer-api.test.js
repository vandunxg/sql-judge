import { describe, expect, test } from "bun:test"

import {
  executeExternalSelect,
  executeExternalSqlServerSelect,
  listExternalMetadata,
  listExternalSqlServerMetadata,
  testExternalConnection,
  testExternalSqlServer,
} from "@/lib/judge-api"

import { EXPLORER_LIMITS, externalCommandsFor } from "./explorer-api"

describe("external explorer command routing", () => {
  test("keeps PostgreSQL and SQL Server on separate native commands", () => {
    expect({
      postgresql: externalCommandsFor("postgresql"),
      sqlserver: externalCommandsFor("sqlserver"),
    }).toEqual({
      postgresql: {
        test: testExternalConnection,
        metadata: listExternalMetadata,
        select: executeExternalSelect,
      },
      sqlserver: {
        test: testExternalSqlServer,
        metadata: listExternalSqlServerMetadata,
        select: executeExternalSqlServerSelect,
      },
    })
  })

  test("bounds every SELECT to 3 seconds, 100 rows and 256 KiB", () => {
    expect(EXPLORER_LIMITS).toEqual({
      statementTimeoutMs: 3_000,
      maxRows: 100,
      maxBytes: 256 * 1024,
    })
  })
})
