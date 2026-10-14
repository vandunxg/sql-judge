import { useState } from "react"
import {
  ChevronDown,
  ChevronRight,
  Eye,
  Folder,
  Search,
  Table2,
} from "lucide-react"

import type { ExternalMetadata } from "@/lib/judge-api"
import { Input } from "@/components/ui/input"

import {
  groupTablesBySchema,
  isViewKind,
  type ExternalTable,
} from "./explorer-tree"

const nodeClass =
  "flex h-[30px] w-full items-center gap-2 rounded-md px-2 text-left text-[13.5px] outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 [&_svg]:size-3.5 [&_svg]:shrink-0 [&_svg]:text-muted-foreground"

export function ExplorerSchemaTree({
  metadata,
  database,
}: {
  metadata: ExternalMetadata
  database: string
}) {
  const [filter, setFilter] = useState("")
  const [collapsedSchemas, setCollapsedSchemas] = useState<Set<string>>(
    new Set()
  )
  const [openTables, setOpenTables] = useState<Set<string>>(new Set())
  const groups = groupTablesBySchema(metadata.tables, filter)

  const toggle = (setter: typeof setOpenTables, key: string) =>
    setter((current) => {
      const next = new Set(current)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })

  return (
    <>
      <div className="p-3">
        <label className="flex h-8 items-center gap-2 rounded-lg border border-input bg-card px-3 text-[13px] has-focus-visible:border-ring has-focus-visible:ring-3 has-focus-visible:ring-ring/50">
          <Search
            className="size-3.5 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            type="search"
            placeholder="Filter tables"
            aria-label="Filter tables"
            className="h-auto min-w-0 flex-1 border-0 bg-transparent px-0 text-[13px] shadow-none focus-visible:ring-0"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        </label>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-2 pb-3">
        {metadata.truncated ? (
          <p className="px-2 pb-2 text-xs text-muted-foreground">
            Metadata was limited by the server-side row cap.
          </p>
        ) : null}
        {groups.length === 0 ? (
          <p className="px-2 py-3 text-sm text-muted-foreground">
            {metadata.tables.length === 0
              ? "No visible tables were returned."
              : "No tables match the filter."}
          </p>
        ) : (
          <ul
            aria-label={`Tables in ${database}`}
            className="flex flex-col gap-px"
          >
            {groups.map(({ schema, tables }) => {
              const open = filter.trim() !== "" || !collapsedSchemas.has(schema)
              return (
                <li key={schema}>
                  <button
                    type="button"
                    className={nodeClass}
                    aria-expanded={open}
                    onClick={() => toggle(setCollapsedSchemas, schema)}
                  >
                    {open ? <ChevronDown /> : <ChevronRight />}
                    <Folder />
                    <span className="truncate">{schema}</span>
                    <span className="ml-auto text-xs text-muted-foreground">
                      {tables.length}
                    </span>
                  </button>
                  {open ? (
                    <ul className="flex flex-col gap-px">
                      {tables.map((table) => (
                        <TableNode
                          key={table.name}
                          table={table}
                          open={openTables.has(`${schema}.${table.name}`)}
                          onToggle={() =>
                            toggle(setOpenTables, `${schema}.${table.name}`)
                          }
                        />
                      ))}
                    </ul>
                  ) : null}
                </li>
              )
            })}
          </ul>
        )}
      </div>
    </>
  )
}

function TableNode({
  table,
  open,
  onToggle,
}: {
  table: ExternalTable
  open: boolean
  onToggle: () => void
}) {
  const view = isViewKind(table.kind)
  return (
    <li>
      <button
        type="button"
        className={`${nodeClass} pl-[22px] ${open ? "bg-muted font-medium" : ""}`}
        aria-expanded={open}
        onClick={onToggle}
      >
        {open ? <ChevronDown /> : <ChevronRight />}
        {view ? <Eye aria-label="View" /> : <Table2 aria-hidden="true" />}
        <span className="truncate">{table.name}</span>
      </button>
      {open ? (
        <ul aria-label={`Columns of ${table.name}`}>
          {table.columns.map((column) => (
            <li
              key={column.name}
              className="flex h-[26px] items-center gap-2 pr-2 pl-11 text-[12.5px]"
            >
              <span className="w-3 shrink-0" aria-hidden="true" />
              <span className="truncate font-mono">{column.name}</span>
              <span className="ml-auto shrink-0 font-mono text-[11.5px] text-muted-foreground">
                {column.dataType}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </li>
  )
}
