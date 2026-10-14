import { useState } from "react"
import { LockKeyhole } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

import { Field, fieldInputClass } from "./connections-ui"

// The saved secret is never read back: the UI only knows whether the vault holds one.
export function ConnectionsSecretField({
  secret,
  hasSavedSecret,
  onChange,
}: {
  secret: string
  hasSavedSecret: boolean
  onChange: (secret: string) => void
}) {
  const [replacing, setReplacing] = useState(false)

  if (hasSavedSecret && !replacing && !secret) {
    return (
      <div className="flex min-w-0 flex-col gap-1.5">
        <span className="text-[13.5px] font-medium" id="secret-label">
          Password
        </span>
        <div
          aria-labelledby="secret-label"
          role="group"
          className="flex h-9 items-center justify-between gap-2 rounded-lg border border-input bg-muted pr-1 pl-3"
        >
          <span className="inline-flex items-center gap-2 text-[13px] text-muted-foreground">
            <LockKeyhole className="size-3.5" aria-hidden="true" /> Saved in keychain
          </span>
          <Button
            type="button"
            variant="ghost"
            className="h-[26px] px-2 text-[13px]"
            onClick={() => setReplacing(true)}
          >
            Replace
          </Button>
        </div>
      </div>
    )
  }

  return (
    <Field label={hasSavedSecret ? "New password or token" : "Password or token"} htmlFor="secret">
      <div className="flex gap-2">
        <Input
          id="secret"
          type="password"
          autoComplete="new-password"
          autoFocus={replacing}
          className={fieldInputClass}
          value={secret}
          onChange={(event) => onChange(event.target.value)}
          aria-describedby="secret-hint"
        />
        {hasSavedSecret ? (
          <Button
            type="button"
            variant="ghost"
            onClick={() => {
              onChange("")
              setReplacing(false)
            }}
          >
            Keep saved
          </Button>
        ) : null}
      </div>
      <span id="secret-hint" className="text-xs text-muted-foreground">
        Sent directly to the system keychain on save. Never shown again.
      </span>
    </Field>
  )
}
