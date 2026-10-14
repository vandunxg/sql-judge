import {
  normalizeTlsMode,
  suggestConnectionProfile,
  type ParsedConnectionProfile,
} from "@/lib/connection-url"
import type { ConnectionProfileSummary, SaveConnectionProfileRequest } from "@/lib/judge-api"

export type ProfileDraft = SaveConnectionProfileRequest
export type Engine = ProfileDraft["engine"]
export type TlsMode = ProfileDraft["tlsMode"]

const DEFAULT_PORTS: Record<Engine, number> = { postgresql: 5432, sqlserver: 1433 }

export const ENGINE_LABELS: Record<Engine, string> = {
  postgresql: "PostgreSQL",
  sqlserver: "SQL Server",
}

export const EMPTY_DRAFT: ProfileDraft = {
  profileId: "",
  displayName: "",
  engine: "postgresql",
  host: "",
  port: DEFAULT_PORTS.postgresql,
  database: "",
  username: "",
  tlsMode: "verify_full",
  secret: "",
}

export const SAMPLE_QUERIES: Record<Engine, string> = {
  postgresql: "SELECT * FROM public.sample LIMIT 50",
  sqlserver: "SELECT TOP (50) * FROM dbo.sample",
}

export function draftFromProfile(profile: ConnectionProfileSummary): ProfileDraft {
  return {
    profileId: profile.profileId,
    displayName: profile.displayName,
    engine: profile.engine,
    host: profile.host,
    port: profile.port,
    database: profile.database,
    username: profile.username,
    tlsMode: normalizeTlsMode(profile.engine, profile.tlsMode),
    secret: "",
  }
}

export function saveRequestFromDraft({ secret, ...draft }: ProfileDraft): SaveConnectionProfileRequest {
  return secret?.trim() ? { ...draft, secret } : draft
}

export function draftForEngine(draft: ProfileDraft, engine: Engine): ProfileDraft {
  return {
    ...draft,
    engine,
    port: DEFAULT_PORTS[engine],
    tlsMode: normalizeTlsMode(engine, draft.tlsMode),
  }
}

const TLS_LABELS: Record<TlsMode, [string, string]> = {
  verify_full: ["Verify certificate and host", "TLS verified"],
  verify_ca: ["Verify certificate authority", "TLS CA verified"],
  require: ["Require TLS (no certificate check)", "TLS not verified"],
}

export function tlsModeLabel(mode: TlsMode): string {
  return TLS_LABELS[mode][0]
}

export function tlsShortLabel(mode: TlsMode): string {
  return TLS_LABELS[mode][1]
}

export function draftFromUrl(draft: ProfileDraft, parsed: ParsedConnectionProfile): ProfileDraft {
  const suggestions = suggestConnectionProfile(parsed)
  return {
    ...draft,
    ...parsed,
    profileId: draft.profileId || suggestions.profileId,
    displayName: draft.displayName || suggestions.displayName,
  }
}
