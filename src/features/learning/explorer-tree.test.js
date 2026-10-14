import { describe, expect, test } from "bun:test"

import { groupTablesBySchema, isViewKind } from "./explorer-tree"

const table = (schema, name, kind = "r") => ({ schema, name, kind, columns: [] })

describe("explorer schema tree", () => {
  test("groups tables by schema in metadata order and filters case-insensitively", () => {
    const tables = [
      table("public", "customers"),
      table("reporting", "daily"),
      table("public", "orders"),
      table("reporting", "customer_rollup", "v"),
    ]
    expect({
      all: groupTablesBySchema(tables, "").map((group) => [
        group.schema,
        group.tables.map((item) => item.name),
      ]),
      filtered: groupTablesBySchema(tables, "  CUSTOMER ").map((group) => [
        group.schema,
        group.tables.map((item) => item.name),
      ]),
      bySchema: groupTablesBySchema(tables, "reporting.").map((group) => group.tables.length),
      none: groupTablesBySchema(tables, "missing"),
    }).toEqual({
      all: [
        ["public", ["customers", "orders"]],
        ["reporting", ["daily", "customer_rollup"]],
      ],
      filtered: [
        ["public", ["customers"]],
        ["reporting", ["customer_rollup"]],
      ],
      bySchema: [2],
      none: [],
    })
  })

  test("recognizes PostgreSQL and SQL Server view kinds only", () => {
    expect(["v", "m", "VIEW", "r", "p", "USER_TABLE", "f"].map(isViewKind)).toEqual([
      true,
      true,
      true,
      false,
      false,
      false,
      false,
    ])
  })
})
