import type { ExternalMetadata } from "@/lib/judge-api"

export type ExternalTable = ExternalMetadata["tables"][number]
export type SchemaGroup = { schema: string; tables: ExternalTable[] }

export function groupTablesBySchema(tables: ExternalTable[], filter: string): SchemaGroup[] {
  const needle = filter.trim().toLowerCase()
  const groups = new Map<string, ExternalTable[]>()
  for (const table of tables) {
    if (needle && !`${table.schema}.${table.name}`.toLowerCase().includes(needle)) {
      continue
    }
    groups.set(table.schema, [...(groups.get(table.schema) ?? []), table])
  }
  return [...groups].map(([schema, items]) => ({ schema, tables: items }))
}

// PostgreSQL relkind 'v'/'m' and SQL Server type_desc 'VIEW'.
export function isViewKind(kind: string): boolean {
  return kind === "v" || kind === "m" || kind.toUpperCase() === "VIEW"
}
