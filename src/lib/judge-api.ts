import { isTauri, trackedInvoke } from "@/lib/tauri"
import { open } from "@tauri-apps/plugin-dialog"
import { MAX_BACKUP_BYTES } from "@/lib/backup-errors"

export type JudgeVerdict =
  | "Accepted"
  | "WrongAnswer"
  | "SyntaxError"
  | "RuntimeError"
  | "TimeLimitExceeded"
  | "ResultLimitExceeded"
  | "PolicyViolation"
  | "Cancelled"
  | "JudgeError"

export type ProblemDifficulty = "easy" | "medium" | "hard"
export type ProblemDialect = "postgresql" | "sqlserver"

export type CatalogColumn = {
  name: string
  type: string
  nullable: boolean
}

export type CatalogTable = {
  name: string
  columns: CatalogColumn[]
}

export type CatalogSampleValue = string | number | boolean | null

export type JudgeRunRequest = {
  problemId: string
  packId: string
  packVersion: string
  packDigest: string
  sql: string
  mode: "run" | "submit"
  requestId?: string
}

export type JudgeTestResult = {
  id: string
  verdict: JudgeVerdict
  elapsedMs: number
  hidden: boolean
}

export type JudgeRunResult = {
  status: "completed" | "unavailable" | "error"
  verdict?: JudgeVerdict
  columns?: Array<{ name: string; type: string }>
  rows?: Array<Array<string | number | boolean | null>>
  tests?: JudgeTestResult[]
  durationMs: number
  message: string
}

export function nativeRequest<T>(request: T) {
  return { request }
}

export type ManagedEnvironmentSummary = {
  envId: string
  instanceId: string
  packId: string
  packVersion: string
  packDigest: string
  namespace: string
  mode: "managed" | "external_explorer" | "external_sandbox"
  state:
    | "provisioning"
    | "ready"
    | "running"
    | "rebuilding"
    | "deleting"
    | "needs_recovery"
    | "error"
}

export type ManagedRuntimeStatus = {
  available: boolean
  state: "running" | "unavailable"
  message: string
}

export type ConnectionProfileSummary = {
  profileId: string
  displayName: string
  engine: "postgresql" | "sqlserver"
  host: string
  port: number
  database: string
  username: string
  tlsMode: "require" | "verify_ca" | "verify_full"
  hasSecret: boolean
}

export type SaveConnectionProfileRequest = Omit<
  ConnectionProfileSummary,
  "hasSecret"
> & {
  secret?: string
}

export type ExternalConnectionInfo = {
  database: string
  username: string
  serverVersion: string
  readOnly: boolean
}

export type ExternalMetadata = {
  tables: Array<{
    schema: string
    name: string
    kind: string
    columns: Array<{ name: string; dataType: string }>
  }>
  truncated: boolean
}

export type ExternalSelectRequest = {
  profileId: string
  sql: string
  statementTimeoutMs: number
  maxRows: number
  maxBytes: number
}

export type ExternalQueryResult = {
  columns: Array<{ name: string; type: string }>
  rows: Array<Array<string | number | boolean | null>>
}

export type CreateManagedEnvironmentRequest = {
  envId: string
  packId: string
  packVersion: string
  packDigest: string
  namespace: string
}

export type ExternalSandboxProbe = {
  database: string
  databaseOid: number
  systemIdentifier: string
}

export type CreateExternalSandboxRequest = {
  envId: string
  profileId: string
  packId: string
  packVersion: string
  packDigest: string
  confirmed: boolean
}

export type ManagedPackSummary = {
  packId: string
  version: string
  digest: string
}

export type CatalogSampleTable = {
  name: string
  rows: Array<Record<string, CatalogSampleValue>>
}

export type InstalledProblem = {
  id: string
  title: string
  difficulty: ProblemDifficulty
  dialect: ProblemDialect
  topics: string[]
  statement: string
  schema: CatalogTable[]
  sampleRows: Array<Record<string, CatalogSampleValue>>
  starterSql: string | null
  outputColumns: Array<{ name: string; type: string }>
  rowOrder: "ordered" | "unordered"
  timeLimitMs: number
  fixtureCount: number
  sampleTables: CatalogSampleTable[]
}

export type InstalledPack = {
  packId: string
  version: string
  title: string
  dialect: ProblemDialect
  digest: string
  problems: InstalledProblem[]
}

export type CloneManagedEnvironmentRequest = {
  sourceEnvId: string
  envId: string
  namespace: string
}

export type ResetManagedEnvironmentRequest = {
  envId: string
  instanceId: string
  namespace: string
  confirmed: boolean
}

export type DeleteManagedEnvironmentRequest = {
  envId: string
  instanceId: string
  confirmed: boolean
}

export type SubmissionTestSummary = {
  testId: string
  verdict: string
  elapsedMs: number
}

export type SubmissionHistorySummary = {
  submissionId: string
  packId: string
  packVersion: string
  packHash: string
  problemId: string | null
  engineVersion: string
  policyVersion: string
  verdict: string
  createdAt: string
  /** Records saved before run tracking existed are submissions. */
  mode: "run" | "submit"
  tests: SubmissionTestSummary[]
}

export type PackImportResult = {
  status: "published" | "already_installed"
  packId: string
  version: string
  digest: string
  message: string
  /** The pack sandbox Run uses; `pending` means the first Run creates it. */
  sandbox: "ready" | "pending"
}

export type BackupSnapshot = {
  version: 2
  environments: Array<{
    envId: string
    instanceId: string
    packId: string
    packVersion: string
    packDigest: string
    namespace: string
    ownershipNonce: string
    mode: string
    state: string
  }>
  submissions: Array<{
    submissionId: string
    packId: string
    packVersion: string
    packHash: string
    engineVersion: string
    policyVersion: string
    verdict: string
    createdAt: string
    /** Omitted for submissions recorded before problem tracking existed. */
    problemId?: string
    /** Omitted means "submit"; only sample runs carry it. */
    mode?: "run"
    tests: Array<{
      testId: string
      verdict: string
      elapsedMs: number
    }>
  }>
}

export async function runJudge(
  request: JudgeRunRequest
): Promise<JudgeRunResult> {
  if (!isTauri()) {
    return {
      status: "unavailable",
      durationMs: 0,
      message:
        "The native PostgreSQL runner is not connected in browser preview mode.",
    }
  }

  return trackedInvoke<JudgeRunResult>(
    request.mode === "run" ? "run_sample" : "submit_solution",
    nativeRequest(request)
  )
}

export async function cancelJudge(requestId: string): Promise<boolean> {
  if (!isTauri()) {
    return false
  }

  return trackedInvoke<boolean>("cancel_judge", { requestId })
}

export async function listManagedEnvironments(
  profileId: string | null = null
): Promise<ManagedEnvironmentSummary[]> {
  if (!isTauri()) {
    return []
  }

  return trackedInvoke<ManagedEnvironmentSummary[]>(
    "list_managed_environments",
    {
      profileId,
    }
  )
}

export async function inspectManagedEnvironment(
  envId: string
): Promise<ManagedEnvironmentSummary | null> {
  if (!isTauri()) {
    return null
  }

  return trackedInvoke<ManagedEnvironmentSummary | null>(
    "inspect_managed_environment",
    { envId }
  )
}

export async function createManagedEnvironment(
  request: CreateManagedEnvironmentRequest
): Promise<ManagedEnvironmentSummary> {
  if (!isTauri()) {
    throw new Error(
      "Managed environments are unavailable in browser preview mode."
    )
  }

  return trackedInvoke<ManagedEnvironmentSummary>(
    "create_managed_environment",
    nativeRequest(request)
  )
}

export async function probeExternalSandboxProfile(
  profileId: string
): Promise<ExternalSandboxProbe> {
  if (!isTauri()) {
    throw new Error(
      "External sandbox verification is only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalSandboxProbe>(
    "probe_external_sandbox_profile",
    nativeRequest({ profileId })
  )
}

export async function createExternalSandbox(
  request: CreateExternalSandboxRequest
): Promise<ManagedEnvironmentSummary> {
  if (!isTauri()) {
    throw new Error(
      "External sandbox creation is only available in the desktop app."
    )
  }

  if (!request.confirmed) {
    throw new Error("Explicit external sandbox confirmation is required.")
  }

  return trackedInvoke<ManagedEnvironmentSummary>(
    "create_external_sandbox",
    nativeRequest(request)
  )
}

export async function cloneManagedEnvironment(
  request: CloneManagedEnvironmentRequest
): Promise<ManagedEnvironmentSummary> {
  if (!isTauri()) {
    throw new Error(
      "Managed environments are unavailable in browser preview mode."
    )
  }

  return trackedInvoke<ManagedEnvironmentSummary>(
    "clone_managed_environment",
    nativeRequest(request)
  )
}

export async function resetManagedEnvironment(
  request: ResetManagedEnvironmentRequest
): Promise<ManagedEnvironmentSummary> {
  if (!isTauri()) {
    throw new Error(
      "Managed environments are unavailable in browser preview mode."
    )
  }

  if (!request.confirmed) {
    throw new Error("Explicit reset confirmation is required.")
  }

  return trackedInvoke<ManagedEnvironmentSummary>(
    "reset_managed_environment",
    nativeRequest(request)
  )
}

export async function deleteManagedEnvironment(
  request: DeleteManagedEnvironmentRequest
): Promise<void> {
  if (!isTauri()) {
    throw new Error(
      "Managed environments are unavailable in browser preview mode."
    )
  }

  if (!request.confirmed) {
    throw new Error("Explicit delete confirmation is required.")
  }

  await trackedInvoke<void>(
    "delete_managed_environment",
    nativeRequest(request)
  )
}

export async function listSubmissionHistory(): Promise<
  SubmissionHistorySummary[]
> {
  if (!isTauri()) {
    return []
  }

  return trackedInvoke<SubmissionHistorySummary[]>("list_submission_history")
}

export async function importPack(path: string): Promise<PackImportResult> {
  if (!isTauri()) {
    throw new Error("Pack import is only available in the desktop app.")
  }

  return trackedInvoke<PackImportResult>("import_pack", nativeRequest({ path }))
}

export async function inspectManagedPack(
  packId: string
): Promise<ManagedPackSummary | null> {
  if (!isTauri()) {
    return null
  }
  return trackedInvoke<ManagedPackSummary | null>("inspect_managed_pack", {
    packId,
  })
}

export async function listInstalledPacks(): Promise<InstalledPack[]> {
  if (!isTauri()) {
    return []
  }
  return trackedInvoke<InstalledPack[]>("list_installed_packs")
}

export type DeletePackRequest = {
  packId: string
  version: string
  digest: string
  confirmed: boolean
}

export async function deletePack(request: DeletePackRequest): Promise<void> {
  if (!isTauri()) {
    throw new Error("Pack deletion is only available in the desktop app.")
  }
  if (!request.confirmed) {
    throw new Error("Deleting a pack requires explicit confirmation.")
  }
  await trackedInvoke<void>("delete_pack", nativeRequest(request))
}

export async function pickPackSource(
  kind: "archive" | "directory"
): Promise<string | null> {
  if (!isTauri()) {
    throw new Error("Pack selection is only available in the desktop app.")
  }

  const selected = await open(
    kind === "directory"
      ? { directory: true, multiple: false }
      : {
          directory: false,
          multiple: false,
          filters: [
            { name: "SQL Judge pack", extensions: ["sqljudge.zip", "zip"] },
          ],
        }
  )
  return typeof selected === "string" ? selected : null
}

export async function exportBackup(): Promise<BackupSnapshot> {
  if (!isTauri()) {
    throw new Error("Metadata backup is only available in the desktop app.")
  }

  return trackedInvoke<BackupSnapshot>("export_backup")
}

export function serializeBackupSnapshot(snapshot: BackupSnapshot): string {
  const serialized = JSON.stringify(snapshot)
  const byteLength = new TextEncoder().encode(serialized).byteLength
  if (byteLength > MAX_BACKUP_BYTES) {
    throw new Error("Metadata backup exceeds the supported size limit.")
  }
  return serialized
}

export async function restoreBackup(snapshot: BackupSnapshot): Promise<void> {
  if (!isTauri()) {
    throw new Error("Metadata restore is only available in the desktop app.")
  }

  await trackedInvoke<void>("restore_backup", {
    snapshotJson: serializeBackupSnapshot(snapshot),
  })
}

export async function getManagedRuntimeStatus(): Promise<ManagedRuntimeStatus> {
  if (!isTauri()) {
    return {
      available: false,
      state: "unavailable",
      message:
        "The managed PostgreSQL runtime is unavailable in browser preview mode.",
    }
  }

  return trackedInvoke<ManagedRuntimeStatus>("managed_runtime_status")
}

export async function startManagedRuntime(): Promise<ManagedRuntimeStatus> {
  if (!isTauri()) {
    return {
      available: false,
      state: "unavailable",
      message:
        "The managed PostgreSQL runtime can only start in the desktop app.",
    }
  }

  return trackedInvoke<ManagedRuntimeStatus>("start_managed_runtime")
}

export async function listConnectionProfiles(): Promise<
  ConnectionProfileSummary[]
> {
  if (!isTauri()) {
    return []
  }

  return trackedInvoke<ConnectionProfileSummary[]>("list_connection_profiles")
}

export async function saveConnectionProfile(
  request: SaveConnectionProfileRequest
): Promise<ConnectionProfileSummary> {
  if (!isTauri()) {
    throw new Error(
      "Connection profiles are only available in the desktop app."
    )
  }

  return trackedInvoke<ConnectionProfileSummary>(
    "save_connection_profile",
    nativeRequest(request)
  )
}

export async function deleteConnectionProfile(
  profileId: string
): Promise<void> {
  if (!isTauri()) {
    throw new Error(
      "Connection profiles are only available in the desktop app."
    )
  }

  await trackedInvoke<void>("delete_connection_profile", { profileId })
}

export async function testExternalConnection(
  profileId: string
): Promise<ExternalConnectionInfo> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalConnectionInfo>(
    "test_external_connection",
    nativeRequest({ profileId })
  )
}

export async function listExternalMetadata(
  profileId: string
): Promise<ExternalMetadata> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalMetadata>(
    "list_external_metadata",
    nativeRequest({ profileId })
  )
}

export async function executeExternalSelect(
  request: ExternalSelectRequest
): Promise<ExternalQueryResult> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalQueryResult>(
    "execute_external_select",
    nativeRequest(request)
  )
}

export async function testExternalSqlServer(
  profileId: string
): Promise<ExternalConnectionInfo> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalConnectionInfo>(
    "test_external_sqlserver",
    nativeRequest({ profileId })
  )
}

export async function listExternalSqlServerMetadata(
  profileId: string
): Promise<ExternalMetadata> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalMetadata>(
    "list_external_sqlserver_metadata",
    nativeRequest({ profileId })
  )
}

export async function executeExternalSqlServerSelect(
  request: ExternalSelectRequest
): Promise<ExternalQueryResult> {
  if (!isTauri()) {
    throw new Error(
      "External connections are only available in the desktop app."
    )
  }

  return trackedInvoke<ExternalQueryResult>(
    "execute_external_sqlserver_select",
    nativeRequest(request)
  )
}
