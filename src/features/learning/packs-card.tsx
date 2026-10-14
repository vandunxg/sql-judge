import { useRef, useState } from "react"
import { BookOpen, Check, Copy, Ellipsis, Table2, Trash2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button, buttonVariants } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type { InstalledPack, ManagedEnvironmentSummary } from "@/lib/judge-api"

import { PackDeleteDialog } from "./packs-delete-dialog"
import type { ProblemStatus } from "./problem-status"
import {
  packDialectLabel,
  packEnvironmentBadge,
  packEnvironments,
  packMonogram,
  packProgress,
  packTableCount,
  shortDigest,
} from "./packs-state"
import {
  packsMonogramClasses,
  packsOutlineButtonClass,
  packsPillClass,
  packsSmallButtonClass,
  packsSurfaceClass,
  packsToneClass,
} from "./packs-styles"

type PackCardProps = {
  pack: InstalledPack
  index: number
  statuses: Map<string, Pick<ProblemStatus, "status">> | null
  environments: ManagedEnvironmentSummary[] | null
  onDeleted: () => void
}

export function PackCard({ pack, index, statuses, environments, onDeleted }: PackCardProps) {
  const [deleting, setDeleting] = useState(false)
  const menuRef = useRef<HTMLButtonElement>(null)
  const progress = statuses ? packProgress(pack, statuses) : null
  const tables = packTableCount(pack)
  const environment = environments ? packEnvironmentBadge(pack, environments) : null
  const titleId = `pack-${index}-title`

  return (
    <article aria-labelledby={titleId} className={`${packsSurfaceClass} flex flex-col gap-[18px] p-[22px]`}>
      <div className="flex items-start gap-3.5">
        <div
          aria-hidden="true"
          className={`flex size-10 shrink-0 items-center justify-center rounded-[10px] font-semibold ${packsMonogramClasses[index % packsMonogramClasses.length]}`}
        >
          {packMonogram(pack.title)}
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 id={titleId} className="text-base font-semibold">{pack.title}</h3>
            <Badge variant="outline" className={`${packsPillClass} bg-card font-mono font-normal`}>
              v{pack.version}
            </Badge>
          </div>
          <p className="truncate font-mono text-[13.5px] text-muted-foreground">{pack.packId}</p>
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger
            ref={menuRef}
            aria-label={`More actions for ${pack.title}`}
            className={buttonVariants({ variant: "ghost", size: "icon", className: "shrink-0" })}
          >
            <Ellipsis aria-hidden="true" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-auto min-w-40">
            <DropdownMenuItem variant="destructive" onClick={() => setDeleting(true)}>
              <Trash2 aria-hidden="true" />
              Delete pack…
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div className="flex flex-wrap gap-x-[18px] gap-y-1.5 text-[13px] text-muted-foreground [&_svg]:size-3.5">
        <span className="inline-flex items-center gap-1.5">
          <BookOpen aria-hidden="true" />
          {pack.problems.length} {pack.problems.length === 1 ? "problem" : "problems"}
        </span>
        <span className="inline-flex items-center gap-1.5">
          <Table2 aria-hidden="true" />
          {tables} {tables === 1 ? "table" : "tables"}
        </span>
        <span>{packDialectLabel(pack.dialect)}</span>
      </div>

      {pack.problems.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          This installed pack contains no problems yet. Import a pack with validated problem manifests to start practicing.
        </p>
      ) : progress ? (
        <div className="flex flex-col gap-1.5">
          <div className="flex justify-between text-[13px]">
            <span>{progress.solved} of {progress.total} solved</span>
            <span className="text-muted-foreground">{progress.percent}%</span>
          </div>
          <div aria-hidden="true" className="h-1.5 overflow-hidden rounded-full bg-muted">
            <span className="block h-full rounded-full bg-ok" style={{ width: `${progress.percent}%` }} />
          </div>
        </div>
      ) : null}

      <div className="flex flex-wrap items-center justify-between gap-2.5 border-t pt-3.5">
        <DigestLabel digest={pack.digest} />
        <div className="flex flex-wrap items-center gap-1.5">
          {environment ? (
            <Badge className={`${packsPillClass} ${packsToneClass[environment.tone]}`}>{environment.label}</Badge>
          ) : null}
          {pack.problems.length > 0 ? (
            <Button
              variant="outline"
              className={`${packsSmallButtonClass} ${packsOutlineButtonClass}`}
              nativeButton={false}
              render={<a href={`#problems?${new URLSearchParams({ pack: pack.packId })}`} />}
            >
              Open problems
            </Button>
          ) : null}
        </div>
      </div>
      {deleting ? (
        <PackDeleteDialog
          pack={pack}
          environments={environments ? packEnvironments(pack, environments) : null}
          returnFocus={menuRef}
          onCancel={() => setDeleting(false)}
          onDeleted={() => {
            setDeleting(false)
            onDeleted()
          }}
        />
      ) : null}
    </article>
  )
}

function DigestLabel({ digest }: { digest: string }) {
  const [copied, setCopied] = useState(false)

  async function copy() {
    try {
      await navigator.clipboard.writeText(digest)
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-xs text-muted-foreground" title={`sha256:${digest}`}>
      {shortDigest(digest)}
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={copied ? "Pack hash copied" : "Copy pack hash"}
        onClick={() => void copy()}
        onBlur={() => setCopied(false)}
        className="[&_svg]:size-[13px]"
      >
        {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
      </Button>
    </span>
  )
}
