import { useState, type FormEvent } from "react"
import { Upload } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

import { packsOutlineButtonClass } from "./packs-styles"

type PacksImportZoneProps = {
  disabled: boolean
  desktopAvailable: boolean
  error: string | null
  onPick: (kind: "archive" | "directory") => void
  onReview: (path: string) => void
}

export function PacksImportZone({ disabled, desktopAvailable, error, onPick, onReview }: PacksImportZoneProps) {
  const [path, setPath] = useState("")
  const [pathError, setPathError] = useState<string | null>(null)

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!path.trim()) {
      setPathError("Enter a local pack directory or .sqljudge.zip path.")
      return
    }
    setPathError(null)
    onReview(path.trim())
  }

  const message = pathError ?? error

  return (
    <section
      aria-labelledby="pack-import-h"
      className="flex flex-col gap-5 rounded-xl border-[1.5px] border-dashed border-input bg-sidebar px-6 py-7"
    >
      <div className="flex flex-wrap items-center gap-5">
        <div className="flex size-11 items-center justify-center rounded-[10px] border bg-card">
          <Upload aria-hidden="true" className="size-5" />
        </div>
        <div className="flex flex-[1_1_280px] flex-col gap-0.5">
          <h2 id="pack-import-h" className="font-medium">
            Import a <span className="font-mono text-[13px]">.sqljudge.zip</span> archive or a pack folder
          </h2>
          <p className="text-[13px] text-muted-foreground">
            You'll review the checks before anything is installed. Nothing is published unless every check passes.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" className={packsOutlineButtonClass} disabled={disabled} onClick={() => onPick("archive")}>
            Choose archive
          </Button>
          <Button variant="ghost" disabled={disabled} onClick={() => onPick("directory")}>
            Choose folder
          </Button>
        </div>
      </div>

      <form className="flex flex-col gap-2 sm:flex-row" onSubmit={submit} noValidate>
        <Input
          aria-label="Local pack path"
          aria-invalid={pathError ? true : undefined}
          value={path}
          onChange={(event) => setPath(event.target.value)}
          placeholder="Or enter a path: /path/to/pack.sqljudge.zip"
          className="h-9 bg-card font-mono text-[13px]"
          disabled={disabled}
        />
        <Button type="submit" variant="outline" className={packsOutlineButtonClass} disabled={disabled}>
          Review pack
        </Button>
      </form>

      {!desktopAvailable ? (
        <p className="rounded-lg bg-warn-bg p-3 text-sm text-warn" role="status">
          Pack import is available from the SQL Judge desktop app. Browser preview is read-only.
        </p>
      ) : null}
      {message ? (
        <p className="text-sm text-err" role="alert">
          {message}
        </p>
      ) : null}
    </section>
  )
}
