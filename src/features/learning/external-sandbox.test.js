import { describe, expect, test } from "bun:test"

import { sandboxProfiles } from "./external-sandbox"

describe("external sandbox profiles", () => {
  test("only exposes saved PostgreSQL profiles with a stored secret", () => {
    expect(
      sandboxProfiles([
        {
          profileId: "pg-ready",
          displayName: "Training PostgreSQL",
          engine: "postgresql",
          host: "db.example.test",
          port: 5432,
          database: "training",
          username: "sandbox",
          tlsMode: "verify_full",
          hasSecret: true,
        },
        {
          profileId: "pg-no-secret",
          displayName: "No secret",
          engine: "postgresql",
          host: "db.example.test",
          port: 5432,
          database: "training",
          username: "sandbox",
          tlsMode: "verify_full",
          hasSecret: false,
        },
        {
          profileId: "sqlserver",
          displayName: "SQL Server",
          engine: "sqlserver",
          host: "sql.example.test",
          port: 1433,
          database: "training",
          username: "sandbox",
          tlsMode: "verify_full",
          hasSecret: true,
        },
      ])
    ).toMatchObject([{ profileId: "pg-ready" }])
  })
})
