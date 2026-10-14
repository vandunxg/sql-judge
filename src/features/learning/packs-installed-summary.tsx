import type { InstalledPack, PackImportResult } from "@/lib/judge-api"

import { packDialectLabel, packTableCount, shortDigest } from "./packs-state"

export function PacksInstalledSummary({ result, pack }: { result: PackImportResult; pack: InstalledPack | null }) {
  const items: Array<[string, string, boolean]> = [
    ["Version", result.version, true],
    ...(pack
      ? ([
          ["Dialect", packDialectLabel(pack.dialect), false],
          ["Problems", `${pack.problems.length} · ${packTableCount(pack)} tables`, false],
        ] as Array<[string, string, boolean]>)
      : []),
    ["Content hash", shortDigest(result.digest), true],
    ["Practice sandbox", result.sandbox === "ready" ? "Ready" : "Created on first Run", false],
  ]
  return (
    <dl className="m-0 grid grid-cols-[repeat(auto-fit,minmax(min(150px,100%),1fr))] gap-4 border-t pt-4">
      {items.map(([label, value, mono]) => (
        <div key={label} className="flex flex-col gap-0.5">
          <dt className="text-[12.5px] text-muted-foreground">{label}</dt>
          <dd className={`m-0 font-medium ${mono ? "font-mono" : ""} ${label === "Content hash" ? "text-[13px]" : ""}`} title={label === "Content hash" ? `sha256:${result.digest}` : undefined}>
            {value}
          </dd>
        </div>
      ))}
    </dl>
  )
}
