import type { ReactNode } from "react"
import { ShieldCheck } from "lucide-react"

import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { supportedTlsModes } from "@/lib/connection-url"

import {
  ENGINE_LABELS,
  tlsModeLabel,
  type Engine,
  type ProfileDraft,
  type TlsMode,
} from "./connections-profile"
import { ConnectionsSecretField } from "./connections-secret-field"
import { Field, fieldInputClass } from "./connections-ui"

type SectionProps = {
  draft: ProfileDraft
  onChange: (patch: Partial<ProfileDraft>) => void
}

const monoInputClass = `${fieldInputClass} font-mono text-[13.5px] md:text-[13.5px]`
const narrowGrid =
  "grid grid-cols-[repeat(auto-fit,minmax(min(140px,100%),1fr))] gap-3.5"
const wideGrid =
  "grid grid-cols-[repeat(auto-fit,minmax(min(200px,100%),1fr))] gap-3.5"

export function FormSection({
  title,
  children,
}: {
  title: string
  children: ReactNode
}) {
  return (
    <section
      aria-label={title}
      className="flex min-w-0 flex-col gap-4 border-t px-6 py-[22px]"
    >
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
}: SectionProps & {
  onEngineChange: (engine: Engine) => void
  isNew: boolean
  urlImport: ReactNode
}) {
  const text = (key: "host" | "database" | "displayName" | "profileId") => ({
    value: draft[key],
    onChange: (event: { target: { value: string } }) =>
      onChange({ [key]: event.target.value }),
  })
  return (
    <FormSection title="Server">
      <div
        role="radiogroup"
        aria-label="Engine"
        className="flex flex-col gap-1.5"
      >
        <span className="text-[13.5px] font-medium">Engine</span>
        <ToggleGroup
          value={[draft.engine]}
          onValueChange={(value) =>
            value[0] && onEngineChange(value[0] as Engine)
          }
          className="inline-flex gap-0.5 self-start rounded-[10px] bg-muted p-[3px]"
          aria-label="Engine"
        >
          {(Object.keys(ENGINE_LABELS) as Engine[]).map((engine) => (
            <ToggleGroupItem
              key={engine}
              value={engine}
              className="h-[30px] rounded-[7px] px-3.5 text-[13px] font-medium text-muted-foreground data-pressed:bg-card data-pressed:text-foreground data-pressed:shadow-xs"
            >
              {ENGINE_LABELS[engine]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      <div className={narrowGrid}>
        <Field
          label="Host"
          htmlFor="host"
          className="col-span-2 max-sm:col-span-1"
        >
          <Input
            id="host"
            required
            className={monoInputClass}
            spellCheck={false}
            {...text("host")}
          />
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
          <Input
            id="database"
            required
            className={monoInputClass}
            spellCheck={false}
            {...text("database")}
          />
        </Field>
        <Field label="Display name" htmlFor="display-name">
          <Input
            id="display-name"
            required
            className={fieldInputClass}
            {...text("displayName")}
          />
        </Field>
        {isNew ? (
          <Field label="Profile ID" htmlFor="profile-id">
            <Input
              id="profile-id"
              required
              className={monoInputClass}
              spellCheck={false}
              {...text("profileId")}
            />
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
          <Select
            value={draft.tlsMode}
            onValueChange={(value) =>
              value && onChange({ tlsMode: value as TlsMode })
            }
          >
            <SelectTrigger id="tls-mode" className="h-9 w-full bg-card">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {supportedTlsModes(draft.engine).map((mode) => (
                <SelectItem key={mode} value={mode}>
                  {tlsModeLabel(mode)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
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
