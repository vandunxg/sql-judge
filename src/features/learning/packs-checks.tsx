import { Check, CircleDashed, CircleHelp, LoaderCircle, Minus, X } from "lucide-react"

import {
  importReportSummary,
  importStages,
  type ImportCheckStatus,
  type ImportCheckStatuses,
  type ImportPhase,
  type ImportStage,
} from "./packs-import-state"
import { packsSurfaceClass } from "./packs-styles"

const checks: Record<ImportStage, { title: string; detail: string }> = {
  source: {
    title: "Pack source",
    detail: "The archive or folder is read from this device.",
  },
  structure: {
    title: "Archive structure and manifest",
    detail: "No paths outside the pack, no links, no duplicate entries, bounded sizes, and a manifest that matches the supported schema.",
  },
  runtime: {
    title: "Disposable database",
    detail: "The managed PostgreSQL runtime on this device starts and its ownership is verified.",
  },
  reference: {
    title: "Schema setup and reference queries",
    detail: "Every problem's setup and reference query runs on its fixtures in a throwaway database before publication.",
  },
}

const statusIcons: Record<ImportCheckStatus, { label: string; className: string; icon: typeof Check }> = {
  passed: { label: "Passed", className: "bg-ok-bg text-ok", icon: Check },
  failed: { label: "Failed", className: "bg-card text-err", icon: X },
  running: { label: "Checking", className: "bg-muted text-muted-foreground", icon: LoaderCircle },
  pending: { label: "Not run yet", className: "bg-muted text-muted-foreground", icon: CircleDashed },
  skipped: { label: "Skipped", className: "bg-muted text-muted-foreground", icon: Minus },
  unknown: { label: "Not confirmed", className: "bg-muted text-muted-foreground", icon: CircleHelp },
}

export function PacksValidationReport({ statuses }: { statuses: ImportCheckStatuses }) {
  return (
    <section aria-labelledby="checks-h" aria-busy={Object.values(statuses).includes("running")} className={`${packsSurfaceClass} overflow-hidden`}>
      <div className="flex items-baseline justify-between gap-3 px-5 py-4">
        <h2 id="checks-h" className="text-[15px] font-semibold">Validation report</h2>
        <span className="text-[13px] text-muted-foreground">{importReportSummary(statuses)}</span>
      </div>
      {importStages.map((stage) => {
        const status = statuses[stage]
        const { label, className, icon: Icon } = statusIcons[status]
        return (
          <div key={stage} className={`flex gap-3.5 border-t px-5 py-3.5 ${status === "failed" ? "bg-err-bg" : ""}`}>
            <span className={`mt-px flex size-[22px] shrink-0 items-center justify-center rounded-full ${className}`}>
              <Icon role="img" aria-label={label} className={`size-[13px] stroke-[2.5] ${status === "running" ? "animate-spin" : ""}`} />
            </span>
            <div>
              <div className="font-medium">{checks[stage].title}</div>
              <div className="text-[13px] text-muted-foreground">{checks[stage].detail}</div>
            </div>
          </div>
        )
      })}
    </section>
  )
}

const steps = ["Choose file", "Review", "Install"] as const

export function PacksImportSteps({ phase }: { phase: ImportPhase }) {
  const current = phase === "installing" || phase === "installed" ? 2 : 1
  return (
    <ol aria-label="Import steps" className="m-0 flex list-none flex-wrap items-center gap-3 p-0">
      {steps.map((step, index) => {
        const done = index < current || phase === "installed"
        const active = index === current && !done
        return [
          index > 0 ? <li key={`${step}-line`} aria-hidden="true" className="h-px w-10 bg-border" /> : null,
          <li key={step} aria-current={active ? "step" : undefined} className="flex items-center gap-2.5 text-sm">
            <span
              className={`flex size-[26px] shrink-0 items-center justify-center rounded-full text-xs font-semibold ${
                done ? "bg-ok-bg text-ok" : active ? "bg-primary text-primary-foreground" : "border border-input text-muted-foreground"
              }`}
            >
              {done ? <Check role="img" aria-label="Done" className="size-[13px] stroke-[2.5]" /> : index + 1}
            </span>
            <span className={active ? "font-medium" : "text-muted-foreground"}>{step}</span>
          </li>,
        ]
      })}
    </ol>
  )
}
