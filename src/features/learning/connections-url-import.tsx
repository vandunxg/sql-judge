import { useState } from "react"
import { Link2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { parseConnectionUrl, type ParsedConnectionProfile } from "@/lib/connection-url"

import { fieldInputClass, smallButtonClass } from "./connections-ui"

export function ConnectionsUrlImport({
  disabled,
  onParsed,
}: {
  disabled: boolean
  onParsed: (parsed: ParsedConnectionProfile) => void
}) {
  const [open, setOpen] = useState(false)
  const [url, setUrl] = useState("")
  const [error, setError] = useState<string | null>(null)

  function fill() {
    try {
      onParsed(parseConnectionUrl(url))
      setUrl("")
      setError(null)
      setOpen(false)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Connection URL could not be parsed.")
    }
  }

  if (!open) {
    return (
      <Button
        type="button"
        variant="ghost"
        className={`${smallButtonClass} self-start px-1.5 text-muted-foreground`}
        onClick={() => setOpen(true)}
        aria-expanded={false}
      >
        <Link2 className="size-3.5" /> Fill from a connection URL
      </Button>
    )
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border bg-muted/40 p-3">
      <label htmlFor="connection-url" className="text-[13.5px] font-medium">
        Connection URL
      </label>
      <div className="flex flex-col gap-2 sm:flex-row">
        <Input
          id="connection-url"
          type="text"
          inputMode="url"
          autoComplete="off"
          spellCheck={false}
          className={`${fieldInputClass} font-mono`}
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault()
              fill()
            }
          }}
          placeholder="postgresql://user:password@host:5432/database"
          disabled={disabled}
          aria-invalid={error ? true : undefined}
          aria-describedby="connection-url-hint"
        />
        <Button type="button" variant="outline" onClick={fill} disabled={disabled || !url.trim()}>
          Fill fields
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => {
            setUrl("")
            setError(null)
            setOpen(false)
          }}
        >
          Cancel
        </Button>
      </div>
      {error ? (
        <p role="alert" className="text-xs text-err">
          {error}
        </p>
      ) : null}
      <p id="connection-url-hint" className="text-xs text-muted-foreground">
        Parsed locally. The URL is not stored or written to debug logs.
      </p>
    </div>
  )
}
