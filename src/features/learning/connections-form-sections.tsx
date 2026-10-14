import type { ReactNode } from "react"
import { ShieldCheck } from "lucide-react"

import { Input } from "@/components/ui/input"
import { supportedTlsModes } from "@/lib/connection-url"

import { ENGINE_LABELS, tlsModeLabel, type Engine, type ProfileDraft, type TlsMode } from "./connections-profile"
import { ConnectionsSecretField } from "./connections-secret-field"
import { Field, fieldInputClass, nativeSelectClass } from "./connections-ui"

type SectionProps = {
  draft: ProfileDraft
  onChange: (patch: Partial<ProfileDraft>) => void
}

const monoInputClass = `${fieldInputClass} font-mono text-[13.5px] md:text-[13.5px]`
const narrowGrid = "grid grid-cols-[repeat(auto-fit,minmax(min(140px,100%),1fr))] gap-3.5"
const wideGrid = "grid grid-cols-[repeat(auto-fit,minmax(min(200px,100%),1fr))] gap-3.5"

export function FormSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex min-w-0 flex-col gap-4 border-t px-6 py-[22px]">
      <h3 className="text-sm font-semibold">{title}</h3>
      {children}
    </section>
  )
}

export function ServerSection({
  draft,
  onChange,
  onEngineChange,
  isNew,
  urlImport,
}: SectionProps & { onEngineChange: (engine: Engine) => void; isNew: boolean; urlImport: ReactNode }) {
  const text = (key: "host" | "database" | "displayName" | "profileId") => ({
    value: draft[key],
    onChange: (event: { target: { value: string } }) => onChange({ [key]: event.target.value }),
  })
  return (
    <FormSection title="Server">
      <div role="radiogroup" aria-label="Engine" className="flex flex-col gap-1.5">
        <span className="text-[13.5px] font-medium">Engine</span>
        <div className="inline-flex gap-0.5 self-start rounded-[10px] bg-muted p-[3px]">
          {(Object.keys(ENGINE_LABELS) as Engine[]).map((engine) => (
            <label
              key={engine}
              className="inline-flex h-[30px] cursor-pointer items-center rounded-[7px] px-3.5 text-[13px] font-medium text-muted-foreground has-checked:bg-card has-checked:text-foreground has-checked:shadow-xs has-focus-visible:ring-3 has-focus-visible:ring-ring/50 has-disabled:cursor-not-allowed"
            >
              <input
                type="radio"
                name="engine"
                value={engine}
                className="sr-only"
                checked={draft.engine === engine}
                onChange={() => onEngineChange(engine)}
              />
              {ENGINE_LABELS[engine]}
            </label>
          ))}
        </div>
      </div>
      <div className={narrowGrid}>
        <Field label="Host" htmlFor="host" className="col-span-2 max-sm:col-span-1">
          <Input id="host" required className={monoInputClass} spellCheck={false} {...text("host")} />
        </Field>
        <Field label="Port" htmlFor="port">
          <Input
            id="port"
            required
            type="number"
            min={1}
            max={65535}
            className={monoInputClass}
            value={draft.port}
            onChange={(event) => onChange({ port: Number(event.target.value) })}
          />
        </Field>
      </div>
      <div className={wideGrid}>
        <Field label="Database" htmlFor="database">
          <Input id="database" required className={monoInputClass} spellCheck={false} {...text("database")} />
        </Field>
        <Field label="Display name" htmlFor="display-name">
          <Input id="display-name" required className={fieldInputClass} {...text("displayName")} />
        </Field>
        {isNew ? (
          <Field label="Profile ID" htmlFor="profile-id">
            <Input id="profile-id" required className={monoInputClass} spellCheck={false} {...text("profileId")} />
          </Field>
        ) : null}
      </div>
      {urlImport}
    </FormSection>
  )
}

export function SignInSection({
  draft,
  onChange,
  hasSavedSecret,
}: SectionProps & { hasSavedSecret: boolean }) {
  return (
    <FormSection title="Sign in">
      <div className={wideGrid}>
        <Field label="Username" htmlFor="username">
          <Input
            id="username"
            required
            autoComplete="off"
            spellCheck={false}
            className={monoInputClass}
            value={draft.username}
            onChange={(event) => onChange({ username: event.target.value })}
          />
        </Field>
        <ConnectionsSecretField
          secret={draft.secret ?? ""}
          hasSavedSecret={hasSavedSecret}
          onChange={(secret) => onChange({ secret })}
        />
      </div>
    </FormSection>
  )
}

export function SecuritySection({ draft, onChange }: SectionProps) {
  return (
    <FormSection title="Security">
      <div className={wideGrid}>
        <Field label="TLS" htmlFor="tls-mode">
          <select
            id="tls-mode"
            className={nativeSelectClass}
            value={draft.tlsMode}
            onChange={(event) => onChange({ tlsMode: event.target.value as TlsMode })}
          >
            {supportedTlsModes(draft.engine).map((mode) => (
              <option key={mode} value={mode}>
                {tlsModeLabel(mode)}
              </option>
            ))}
          </select>
        </Field>
        <div className="flex min-w-0 flex-col gap-1.5">
          <span className="text-[13.5px] font-medium">Access</span>
          <p className="flex h-9 items-center gap-2 rounded-lg border border-input bg-muted px-3 text-[13.5px]">
            <ShieldCheck className="size-3.5 text-ok" aria-hidden="true" />
            Read-only, always
          </p>
        </div>
      </div>
    </FormSection>
  )
}
