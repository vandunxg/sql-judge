import { useEffect, useRef, useState } from "react"
import { LoaderCircle, Package } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  importPack,
  type InstalledPack,
  type PackImportResult,
} from "@/lib/judge-api"
import { readPackImportError } from "@/lib/pack-errors"

import { PacksImportSteps, PacksValidationReport } from "./packs-checks"
import {
  importCheckStatuses,
  importFailureStage,
  sourceName,
  type ImportPhase,
  type ImportStage,
} from "./packs-import-state"
import { PacksInstalledSummary } from "./packs-installed-summary"
import {
  packsHeadingClass,
  packsSmallButtonClass,
  packsSurfaceClass,
} from "./packs-styles"

type PacksReviewProps = {
  path: string
  disabled: boolean
  onClose: () => void
  refreshAfterImport: () => Promise<InstalledPack[] | null>
}

type Installed = {
  result: PackImportResult
  pack: InstalledPack | null
  refreshFailed: boolean
}

export function PacksReview({
  path,
  disabled,
  onClose,
  refreshAfterImport,
}: PacksReviewProps) {
  const [phase, setPhase] = useState<ImportPhase>("ready")
  const [failedStage, setFailedStage] = useState<ImportStage | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [installed, setInstalled] = useState<Installed | null>(null)
  const headingRef = useRef<HTMLHeadingElement>(null)
  const name = sourceName(path) || path
  const busy = phase === "installing"

  useEffect(() => headingRef.current?.focus(), [])

  useEffect(() => {
    if (error) {
      toast.error(error)
    }
  }, [error])

  async function install() {
    setPhase("installing")
    setError(null)
    setFailedStage(null)
    try {
      const result = await importPack(path)
      const packs = await refreshAfterImport()
      const pack =
        packs?.find(
          (candidate) =>
            candidate.packId === result.packId &&
            candidate.version === result.version &&
            candidate.digest === result.digest
        ) ?? null
      setInstalled({ result, pack, refreshFailed: packs === null })
      setPhase("installed")
    } catch (reason) {
      const message = readPackImportError(reason)
      setError(message)
      setFailedStage(importFailureStage(message))
      setPhase("failed")
    }
  }

  const title = installed
    ? `${installed.pack?.title ?? installed.result.packId} is ${installed.result.status === "already_installed" ? "already installed" : "installed"}`
    : phase === "failed"
      ? `${name} was not installed`
      : phase === "installing"
        ? `Checking ${name}…`
        : `Review ${name} before installing`
  const subtitle = installed
    ? `${installed.result.message} ${installed.result.packId} v${installed.result.version} (${installed.result.status === "already_installed" ? "already installed" : "published"}).${installed.refreshFailed ? " The catalog refresh failed; retry it from the Packs list." : ""}`
    : phase === "failed"
      ? "Nothing was published. Fix the pack or choose another source, then try again."
      : "SQL Judge checks the pack in a throwaway database first. Nothing is installed unless every check passes."

  return (
    <main className="min-h-0 flex-1 overflow-y-auto px-4 py-8 text-sm sm:px-8 lg:px-10 lg:pb-12">
      <div className="mx-auto flex max-w-[820px] flex-col gap-6">
        <PacksImportSteps phase={phase} />

        <div className="flex flex-col gap-1.5">
          <h1
            ref={headingRef}
            tabIndex={-1}
            className={`${packsHeadingClass} break-words outline-none`}
          >
            {title}
          </h1>
          <p
            className="text-[15px] text-muted-foreground"
            role="status"
            aria-live="polite"
          >
            {subtitle}
          </p>
        </div>

        <section
          aria-labelledby="sum-h"
          className={`${packsSurfaceClass} flex flex-col gap-[18px] px-[22px] py-5`}
        >
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex size-[38px] items-center justify-center rounded-[9px] bg-muted">
              <Package aria-hidden="true" className="size-4" />
            </div>
            <div className="flex min-w-0 flex-1 flex-col">
              <h2 id="sum-h" className="truncate text-[15px] font-semibold">
                {name}
              </h2>
              <span className="font-mono text-[13px] break-all text-muted-foreground">
                {path}
              </span>
            </div>
            {installed ? null : (
              <Button
                variant="ghost"
                className={packsSmallButtonClass}
                disabled={busy}
                onClick={onClose}
              >
                Choose another
              </Button>
            )}
          </div>
          {installed ? (
            <PacksInstalledSummary
              result={installed.result}
              pack={installed.pack}
            />
          ) : null}
        </section>

        <PacksValidationReport
          statuses={importCheckStatuses(phase, failedStage)}
        />

        <div className="flex flex-wrap items-center justify-end gap-2 pt-1">
          {installed ? (
            <>
              <Button variant="ghost" onClick={onClose}>
                Back to packs
              </Button>
              {installed.pack && installed.pack.problems.length > 0 ? (
                <Button
                  nativeButton={false}
                  render={
                    <a
                      href={`#problems?${new URLSearchParams({ pack: installed.pack.packId })}`}
                    />
                  }
                >
                  Open problems
                </Button>
              ) : null}
            </>
          ) : (
            <>
              <Button variant="ghost" disabled={busy} onClick={onClose}>
                Cancel
              </Button>
              <Button
                disabled={disabled || busy}
                onClick={() => void install()}
              >
                {busy ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" />
                ) : null}
                {busy
                  ? "Installing…"
                  : phase === "failed"
                    ? "Try again"
                    : "Install pack"}
              </Button>
            </>
          )}
        </div>
      </div>
    </main>
  )
}
