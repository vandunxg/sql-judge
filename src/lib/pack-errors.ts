const SAFE_PACK_ERRORS = new Set([
  "A local pack path is required.",
  "Pack import is only available in the desktop app.",
  "Pack source could not be loaded.",
  "Pack failed structural validation.",
  "Managed environment state is busy.",
  "Managed PostgreSQL runtime could not be started.",
  "Managed PostgreSQL runtime is unavailable.",
  "Managed PostgreSQL runtime ownership could not be verified.",
  "Pack runtime validation or publication failed.",
  "Another pack is already installed with this ID and version.",
])

const FALLBACK_PACK_ERROR =
  "Pack import failed closed. No unvalidated pack was published."
const RUNTIME_START_ERROR = "Managed PostgreSQL runtime could not be started."
const RUNTIME_START_GUIDANCE =
  "Managed PostgreSQL runtime could not be started. Install PostgreSQL tools (initdb and pg_ctl) and ensure they are available on PATH."
const SAFE_RUNTIME_VALIDATION_ERROR =
  /^Pack runtime validation failed for [a-z][a-z0-9-]*\/[a-z][a-z0-9-]*\.$/u

export function isPackImportDisabled(desktopAvailable: boolean, busy: boolean) {
  return !desktopAvailable || busy
}

export function readPackImportError(reason: unknown): string {
  const message = nativeMessage(reason)
  if (message === RUNTIME_START_ERROR) {
    return RUNTIME_START_GUIDANCE
  }
  if (SAFE_RUNTIME_VALIDATION_ERROR.test(message)) {
    return message
  }
  return SAFE_PACK_ERRORS.has(message) ? message : FALLBACK_PACK_ERROR
}

const SAFE_PACK_DELETE_ERRORS = new Set([
  "Deleting a pack requires explicit confirmation.",
  "Managed environment state is busy.",
  "The exact pack is not installed.",
  "Installed pack catalog could not be read.",
  "A pack environment is busy or needs recovery; the pack was kept.",
  "A pack environment could not be deleted; the pack was kept.",
  "The pack could not be removed from the catalog.",
  "Managed PostgreSQL runtime is unavailable.",
])

export function readPackDeleteError(reason: unknown): string {
  const message = nativeMessage(reason)
  if (message === RUNTIME_START_ERROR) {
    return RUNTIME_START_GUIDANCE
  }
  return SAFE_PACK_DELETE_ERRORS.has(message)
    ? message
    : "The pack was not deleted. Try again."
}

function nativeMessage(reason: unknown): string {
  if (reason instanceof Error) {
    return reason.message
  }
  if (typeof reason === "string") {
    return reason
  }
  if (typeof reason === "object" && reason !== null && "message" in reason) {
    const message = (reason as { message?: unknown }).message
    return typeof message === "string" ? message : ""
  }
  return ""
}
