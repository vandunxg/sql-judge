import type { FormEvent } from "react"
import { cn } from "cn"
import { ArrowRight } from "lucide-react"

import { buttonVariants } from "@/components/ui/button"
import type { ConnectionProfileSummary } from "@/lib/judge-api"

import { ConnectionsFormFooter } from "./connections-form-footer"
import {
  SecuritySection,
  ServerSection,
  SignInSection,
} from "./connections-form-sections"
import {
  ENGINE_LABELS,
  draftForEngine,
  draftFromUrl,
  type ProfileDraft,
} from "./connections-profile"
import { smallButtonClass, surfaceCardClass } from "./connections-ui"
import { ConnectionsUrlImport } from "./connections-url-import"
import { explorerHref } from "./explorer-route"

type ProfileFormProps = {
  draft: ProfileDraft
  saved: ConnectionProfileSummary | undefined
  testing: boolean
  disabled: boolean
  onChange: (next: ProfileDraft) => void
  onSave: () => void
  onDelete: () => void
  onTest: () => void
  modal?: boolean
}

function subtitle(saved: ConnectionProfileSummary | undefined) {
  if (!saved) {
    return "Explorer profiles are always read-only."
  }
  return `${ENGINE_LABELS[saved.engine]} · read-only profile`
}

export function ConnectionsProfileForm({
  draft,
  saved,
  testing,
  disabled,
  onChange,
  onSave,
  onDelete,
  onTest,
  modal = false,
}: ProfileFormProps) {
  const patch = (next: Partial<ProfileDraft>) => onChange({ ...draft, ...next })

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    onSave()
  }

  return (
    <section
      aria-labelledby={!modal ? "profile-form-heading" : undefined}
      className={
        modal
          ? "min-w-0"
          : `${surfaceCardClass} min-w-0 flex-[3_1_560px] overflow-hidden`
      }
    >
      {!modal ? (
        <div className="flex flex-wrap items-center justify-between gap-3 px-6 py-5">
          <div className="flex min-w-0 flex-col gap-0.5">
            <h2
              id="profile-form-heading"
              className="truncate text-[17px] font-semibold"
            >
              {saved?.displayName ?? "New profile"}
            </h2>
            <span className="truncate text-[13px] text-muted-foreground">
              {subtitle(saved)}
            </span>
          </div>
          {saved ? (
            <a
              href={explorerHref(saved.profileId)}
              className={cn(
                buttonVariants({ variant: "outline" }),
                smallButtonClass,
                "bg-card shadow-xs"
              )}
            >
              Open in Explorer <ArrowRight />
            </a>
          ) : null}
        </div>
      ) : null}
      <form onSubmit={submit}>
        <fieldset disabled={disabled} className="min-w-0">
          <ServerSection
            draft={draft}
            onChange={patch}
            onEngineChange={(engine) => onChange(draftForEngine(draft, engine))}
            isNew={!saved}
            urlImport={
              <ConnectionsUrlImport
                disabled={disabled}
                onParsed={(parsed) => onChange(draftFromUrl(draft, parsed))}
              />
            }
          />
          <SignInSection
            key={saved?.profileId ?? "new"}
            draft={draft}
            onChange={patch}
            hasSavedSecret={saved?.hasSecret ?? false}
          />
          <SecuritySection draft={draft} onChange={patch} />
          <ConnectionsFormFooter
            testing={testing}
            canTest={Boolean(saved)}
            canDelete={Boolean(saved)}
            disabled={disabled}
            onDelete={onDelete}
            onTest={onTest}
            isNew={!saved}
          />
        </fieldset>
      </form>
    </section>
  )
}
