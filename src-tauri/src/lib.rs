use app_storage::{
    BackupSnapshot, ConnectionProfile, EnvironmentStore, HistoryStore, MAX_BACKUP_BYTES,
    RecoveryOperation, RecoveryRecord, SubmissionRecord, TestRecord,
    export_backup as export_metadata_backup, restore_backup as restore_metadata_backup,
};
use environment_manager::{
    DestructiveAction, EnvironmentLedger, EnvironmentMode, EnvironmentRecord, EnvironmentState,
};
use judge_core::{Dialect, QueryLimits, QueryResult, SqlValue, Verdict};
use judge_postgres::{
    CancellationToken, ExternalConnectionInfo, ExternalMetadata, ExternalPostgresConfig,
    ExternalPostgresError, ExternalPostgresExplorer, ExternalTable, ExternalTlsMode, JudgeCase,
    ManagedEnvironment, ManagedPostgresRuntime, ManagedPostgresSession, PostgresSession,
    QueryExecutionError, judge_submission_with_watchdog_token,
};
use judge_sqlserver::{
    SqlServerConfig, SqlServerConnectionInfo, SqlServerError, SqlServerExplorer, SqlServerTlsMode,
};
use keyring::Entry;
use problem_pack::{PackData, PackError, PackImporter, ValidatedProblem};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_opener::OpenerExt;

const CREDENTIAL_SERVICE: &str = "com.sqljudge.connections.v1";
const MAX_EXTERNAL_SQL_BYTES: usize = 64 * 1024;
static SESSION_SECRETS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
enum VaultError {
    #[error("invalid secret reference")]
    InvalidReference,
    #[error("credential vault operation failed")]
    OperationFailed,
}

trait SecretVault {
    fn set_secret(&mut self, secret_ref: &str, secret: &str) -> Result<(), VaultError>;
    fn get_secret(&self, secret_ref: &str) -> Result<Option<String>, VaultError>;
    fn delete_secret(&mut self, secret_ref: &str) -> Result<(), VaultError>;
}

struct KeyringSecretVault;

struct SessionSecretVault;

fn session_secrets() -> &'static Mutex<HashMap<String, String>> {
    SESSION_SECRETS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl SecretVault for KeyringSecretVault {
    fn set_secret(&mut self, secret_ref: &str, secret: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        Entry::new(CREDENTIAL_SERVICE, secret_ref)
            .map_err(|_| VaultError::OperationFailed)?
            .set_password(secret)
            .map_err(|_| VaultError::OperationFailed)
    }

    fn get_secret(&self, secret_ref: &str) -> Result<Option<String>, VaultError> {
        validate_secret_ref(secret_ref)?;
        match Entry::new(CREDENTIAL_SERVICE, secret_ref)
            .map_err(|_| VaultError::OperationFailed)?
            .get_password()
        {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VaultError::OperationFailed),
        }
    }

    fn delete_secret(&mut self, secret_ref: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        Entry::new(CREDENTIAL_SERVICE, secret_ref)
            .map_err(|_| VaultError::OperationFailed)?
            .delete_credential()
            .or_else(|error| match error {
                keyring::Error::NoEntry => Ok(()),
                _ => Err(VaultError::OperationFailed),
            })
    }
}

impl SecretVault for SessionSecretVault {
    fn set_secret(&mut self, secret_ref: &str, secret: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        let mut keyring = KeyringSecretVault;
        match keyring.set_secret(secret_ref, secret) {
            Ok(()) => {
                session_secrets()
                    .lock()
                    .map_err(|_| VaultError::OperationFailed)?
                    .remove(secret_ref);
                Ok(())
            }
            Err(VaultError::OperationFailed) => {
                session_secrets()
                    .lock()
                    .map_err(|_| VaultError::OperationFailed)?
                    .insert(secret_ref.to_owned(), secret.to_owned());
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn get_secret(&self, secret_ref: &str) -> Result<Option<String>, VaultError> {
        validate_secret_ref(secret_ref)?;
        let keyring = KeyringSecretVault;
        match keyring.get_secret(secret_ref) {
            Ok(Some(secret)) => Ok(Some(secret)),
            Ok(None) | Err(VaultError::OperationFailed) => session_secrets()
                .lock()
                .map_err(|_| VaultError::OperationFailed)
                .map(|secrets| secrets.get(secret_ref).cloned()),
            Err(error) => Err(error),
        }
    }

    fn delete_secret(&mut self, secret_ref: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        let mut keyring = KeyringSecretVault;
        keyring.delete_secret(secret_ref)?;
        session_secrets()
            .lock()
            .map_err(|_| VaultError::OperationFailed)?
            .remove(secret_ref);
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
struct MemorySecretVault {
    secrets: HashMap<String, String>,
}

#[cfg(test)]
impl SecretVault for MemorySecretVault {
    fn set_secret(&mut self, secret_ref: &str, secret: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        self.secrets
            .insert(secret_ref.to_owned(), secret.to_owned());
        Ok(())
    }

    fn get_secret(&self, secret_ref: &str) -> Result<Option<String>, VaultError> {
        validate_secret_ref(secret_ref)?;
        Ok(self.secrets.get(secret_ref).cloned())
    }

    fn delete_secret(&mut self, secret_ref: &str) -> Result<(), VaultError> {
        validate_secret_ref(secret_ref)?;
        self.secrets.remove(secret_ref);
        Ok(())
    }
}

fn validate_secret_ref(secret_ref: &str) -> Result<(), VaultError> {
    let valid_prefix = "vault://sql-judge/";
    let suffix = secret_ref.strip_prefix(valid_prefix);
    if suffix.is_none_or(|suffix| {
        suffix.is_empty()
            || suffix.len() > 192
            || suffix.contains("..")
            || suffix.contains('/')
            || suffix.contains('\\')
            || suffix.chars().any(char::is_control)
    }) {
        return Err(VaultError::InvalidReference);
    }
    Ok(())
}
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JudgeRunRequest {
    problem_id: String,
    pack_id: String,
    pack_version: String,
    pack_digest: String,
    sql: String,
    mode: String,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportPackRequest {
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackImportResult {
    status: String,
    pack_id: String,
    version: String,
    digest: String,
    message: String,
}

static CANCELLATIONS: OnceLock<Mutex<HashMap<String, CancellationToken>>> = OnceLock::new();

fn cancellation_registry() -> &'static Mutex<HashMap<String, CancellationToken>> {
    CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct CancellationRegistration {
    id: Option<String>,
    token: CancellationToken,
}

impl Drop for CancellationRegistration {
    fn drop(&mut self) {
        if let Some(id) = &self.id
            && let Ok(mut registry) = cancellation_registry().lock()
        {
            let owns_entry = registry
                .get(id)
                .is_some_and(|token| token.same_instance(&self.token));
            if owns_entry {
                registry.remove(id);
            }
        }
    }
}

fn register_cancellation(request_id: Option<&str>) -> CancellationRegistration {
    let token = CancellationToken::default();
    let id = request_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
    if let Some(id) = &id
        && let Ok(mut registry) = cancellation_registry().lock()
    {
        registry.insert(id.clone(), token.clone());
    }
    CancellationRegistration { id, token }
}

fn cancel_judge_value(request_id: &str) -> bool {
    cancellation_registry()
        .lock()
        .ok()
        .and_then(|registry| registry.get(request_id).cloned())
        .is_some_and(|token| {
            token.cancel();
            true
        })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JudgeRunResult {
    status: String,
    verdict: Option<String>,
    columns: Vec<JudgeColumn>,
    rows: Vec<Vec<serde_json::Value>>,
    duration_ms: u64,
    message: String,
    /// Per-fixture outcome only: never expected rows, fixture SQL, or mismatch details.
    tests: Vec<JudgeTestResult>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct JudgeTestResult {
    id: String,
    verdict: String,
    elapsed_ms: u64,
    hidden: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct JudgeColumn {
    name: String,
    r#type: String,
}

struct ManagedEnvironmentState {
    database_path: PathBuf,
    store: EnvironmentStore,
    history: HistoryStore,
    ledger: EnvironmentLedger,
    runtime_root: PathBuf,
    runtime_instance_id: String,
    runtime: Option<ManagedPostgresRuntime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateEnvironmentRequest {
    env_id: String,
    pack_id: String,
    pack_version: String,
    pack_digest: String,
    namespace: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedPackSummary {
    pack_id: String,
    version: String,
    digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloneEnvironmentRequest {
    source_env_id: String,
    env_id: String,
    namespace: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetEnvironmentRequest {
    env_id: String,
    instance_id: String,
    namespace: String,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEnvironmentRequest {
    env_id: String,
    instance_id: String,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveConnectionProfileRequest {
    profile_id: String,
    display_name: String,
    engine: String,
    host: String,
    port: u16,
    database: String,
    username: String,
    tls_mode: String,
    #[serde(default)]
    secret: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ConnectionProfileSummary {
    profile_id: String,
    display_name: String,
    engine: String,
    host: String,
    port: u16,
    database: String,
    username: String,
    tls_mode: String,
    has_secret: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExternalProfileRequest {
    profile_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExternalSelectRequest {
    profile_id: String,
    sql: String,
    statement_timeout_ms: u32,
    max_rows: u32,
    max_bytes: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExternalCommandError {
    code: String,
    message: String,
}

fn validate_external_sql_request(sql: &str) -> Result<(), ExternalCommandError> {
    if sql.len() > MAX_EXTERNAL_SQL_BYTES {
        return Err(ExternalCommandError {
            code: "sql_too_large".to_owned(),
            message: "External SQL exceeds the supported input limit.".to_owned(),
        });
    }
    Ok(())
}

fn validate_external_query_limits(
    statement_timeout_ms: u32,
    max_rows: u32,
    max_bytes: u32,
) -> Result<QueryLimits, ExternalCommandError> {
    let limits = QueryLimits::new(statement_timeout_ms, max_rows, max_bytes);
    limits.validate().map_err(|_| ExternalCommandError {
        code: "limits_invalid".to_owned(),
        message: "External query limits are outside the supported bounds.".to_owned(),
    })?;
    Ok(limits)
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExternalConnectionInfoResponse {
    database: String,
    username: String,
    server_version: String,
    read_only: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExternalColumnResponse {
    name: String,
    data_type: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExternalTableResponse {
    schema: String,
    name: String,
    kind: String,
    columns: Vec<ExternalColumnResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExternalMetadataResponse {
    tables: Vec<ExternalTableResponse>,
    truncated: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExternalQueryResponse {
    columns: Vec<JudgeColumn>,
    rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedRuntimeStatus {
    available: bool,
    state: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedEnvironmentSummary {
    env_id: String,
    instance_id: String,
    pack_id: String,
    pack_version: String,
    pack_digest: String,
    namespace: String,
    mode: String,
    state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmissionHistorySummary {
    submission_id: String,
    pack_id: String,
    pack_version: String,
    pack_hash: String,
    engine_version: String,
    policy_version: String,
    verdict: String,
    created_at: String,
    problem_id: Option<String>,
    mode: String,
    tests: Vec<SubmissionTestSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmissionTestSummary {
    test_id: String,
    verdict: String,
    elapsed_ms: u64,
}

fn load_environment_state(path: &Path) -> Result<ManagedEnvironmentState, String> {
    let mut store = EnvironmentStore::open(path)
        .map_err(|_| "Managed environment state could not be opened.".to_owned())?;
    let history = HistoryStore::open(path)
        .map_err(|_| "Submission history could not be opened.".to_owned())?;
    let runtime_instance_id = store
        .get_or_create_runtime_instance_id()
        .map_err(|_| "Managed runtime identity could not be persisted.".to_owned())?;
    let mut records = store
        .list_environments()
        .map_err(|_| "Managed environment state could not be read.".to_owned())?;
    let recoveries = store
        .list_recoveries()
        .map_err(|_| "Managed environment recovery state could not be read.".to_owned())?;
    for record in &mut records {
        let has_matching_recovery = recoveries.iter().any(|recovery| {
            recovery.env_id == record.env_id
                && recovery.instance_id == record.instance_id
                && recovery.ownership_nonce == record.ownership_nonce
                && recovery.namespace == record.namespace
        });
        if matches!(
            record.state,
            EnvironmentState::Provisioning
                | EnvironmentState::Running
                | EnvironmentState::Rebuilding
                | EnvironmentState::Deleting
        ) && !has_matching_recovery
        {
            record.state = EnvironmentState::NeedsRecovery;
            store
                .save_environment(record)
                .map_err(|_| "Managed environment recovery state could not be saved.".to_owned())?;
        }
    }
    let ledger = EnvironmentLedger::restore(records)
        .map_err(|_| "Managed environment state failed policy validation.".to_owned())?;
    let runtime_root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("postgres-runtime");
    Ok(ManagedEnvironmentState {
        database_path: path.to_owned(),
        store,
        history,
        ledger,
        runtime_root,
        runtime_instance_id,
        runtime: None,
    })
}

fn load_environment_state_for_startup(path: &Path) -> Result<ManagedEnvironmentState, String> {
    let mut state = load_environment_state(path)?;
    let has_recovery = !state
        .store
        .list_recoveries()
        .map_err(|_| "Managed environment recovery state could not be read.".to_owned())?
        .is_empty();

    match ManagedPostgresRuntime::adopt(&state.runtime_root, &state.runtime_instance_id) {
        Ok(runtime) => {
            state.runtime = Some(runtime);
        }
        Err(QueryExecutionError::ConnectionError) => return Ok(state),
        Err(_) if has_recovery => return Ok(state),
        Err(_) => {
            return Err("Managed PostgreSQL runtime identity verification failed.".to_owned());
        }
    }

    if has_recovery {
        // Journaled operations reconcile before the state is exposed to commands.
        reconcile_environment_recovery(&mut state)?;
    }
    Ok(state)
}

fn managed_runtime_status_value(available: bool) -> ManagedRuntimeStatus {
    if available {
        ManagedRuntimeStatus {
            available: true,
            state: "running".to_owned(),
            message: "Managed PostgreSQL runtime is available.".to_owned(),
        }
    } else {
        ManagedRuntimeStatus {
            available: false,
            state: "unavailable".to_owned(),
            message: "Managed PostgreSQL runtime has not been started.".to_owned(),
        }
    }
}

fn ensure_managed_runtime(
    state: &mut ManagedEnvironmentState,
) -> Result<ManagedRuntimeStatus, String> {
    if state.runtime.is_none() {
        let runtime =
            ManagedPostgresRuntime::start(&state.runtime_root, &state.runtime_instance_id)
                .map_err(|_| "Managed PostgreSQL runtime could not be started.".to_owned())?;
        state.runtime = Some(runtime);
    }
    reconcile_environment_recovery(state)?;
    Ok(managed_runtime_status_value(true))
}

fn recovery_environment(
    recovery: &RecoveryRecord,
    namespace: &str,
) -> Result<ManagedEnvironment, String> {
    ManagedEnvironment::new(
        recovery.env_id.clone(),
        recovery.instance_id.clone(),
        namespace.to_owned(),
        recovery.ownership_nonce.clone(),
    )
    .map_err(|_| "Managed recovery identity failed policy validation.".to_owned())
}

fn drop_recovery_namespace(
    session: &mut PostgresSession,
    recovery: &RecoveryRecord,
    namespace: &str,
) -> Result<(), String> {
    let environment = recovery_environment(recovery, namespace)?;
    let exists = session
        .managed_namespace_exists(&environment)
        .map_err(|_| "Managed recovery namespace existence could not be verified.".to_owned())?;
    if !exists {
        return Ok(());
    }
    let owned = session
        .managed_namespace_is_owned(&environment)
        .map_err(|_| "Managed recovery ownership verification failed.".to_owned())?;
    if !owned {
        return Err("Managed recovery namespace ownership could not be verified.".to_owned());
    }
    session
        .drop_managed_namespace(&environment)
        .map_err(|_| "Managed recovery cleanup failed closed.".to_owned())?;
    Ok(())
}

fn reconcile_environment_recovery(state: &mut ManagedEnvironmentState) -> Result<(), String> {
    let recoveries = state
        .store
        .list_recoveries()
        .map_err(|_| "Managed recovery journal could not be read.".to_owned())?;
    if recoveries.is_empty() {
        return Ok(());
    }
    let connection = state
        .runtime
        .as_ref()
        .ok_or_else(|| "Managed PostgreSQL runtime is unavailable.".to_owned())?
        .connection_string();
    let mut session = PostgresSession::connect(&connection)
        .map_err(|_| "Managed recovery runtime connection failed.".to_owned())?;

    for recovery in recoveries {
        match recovery.operation {
            RecoveryOperation::Provision => {
                let environment = recovery_environment(&recovery, &recovery.namespace)?;
                let current = state.ledger.inspect(&recovery.env_id).ok().cloned();
                if let Some(current) = current.as_ref()
                    && (current.instance_id != recovery.instance_id
                        || current.namespace != recovery.namespace
                        || current.ownership_nonce != recovery.ownership_nonce)
                {
                    return Err(
                        "Managed provisioning recovery identity does not match the ledger."
                            .to_owned(),
                    );
                }
                let namespace_exists = session.managed_namespace_exists(&environment).map_err(
                    |_| {
                        "Managed provisioning recovery namespace existence could not be verified."
                            .to_owned()
                    },
                )?;
                if namespace_exists {
                    session
                        .managed_namespace_is_owned(&environment)
                        .map_err(|_| {
                            "Managed provisioning recovery ownership verification failed."
                                .to_owned()
                        })?;
                }
                let schema_persisted = if namespace_exists {
                    session
                        .managed_namespace_has_schema(&environment)
                        .map_err(|_| {
                            "Managed provisioning recovery ownership verification failed."
                                .to_owned()
                        })?
                } else {
                    false
                };
                if schema_persisted {
                    let Some(current) = current else {
                        drop_recovery_namespace(&mut session, &recovery, &recovery.namespace)?;
                        state.store.clear_recovery(&recovery.env_id).map_err(|_| {
                            "Managed provisioning recovery could not be cleared.".to_owned()
                        })?;
                        continue;
                    };
                    if current.state == EnvironmentState::Provisioning {
                        state.ledger.mark_ready(&recovery.env_id).map_err(|_| {
                            "Managed provisioning recovery could not enter the ready state."
                                .to_owned()
                        })?;
                    }
                    let recovered = state
                        .ledger
                        .inspect(&recovery.env_id)
                        .map_err(|_| {
                            "Managed provisioning recovery record disappeared.".to_owned()
                        })?
                        .clone();
                    state.store.save_environment(&recovered).map_err(|_| {
                        "Managed provisioning recovery state could not be saved.".to_owned()
                    })?;
                } else {
                    if namespace_exists {
                        drop_recovery_namespace(&mut session, &recovery, &recovery.namespace)?;
                    }
                    if current.is_some() {
                        state
                            .ledger
                            .remove_recovered(&recovery.env_id)
                            .map_err(|_| {
                                "Managed provisioning recovery could not remove the ledger record."
                                    .to_owned()
                            })?;
                        state
                            .store
                            .delete_environment(&recovery.env_id)
                            .map_err(|_| {
                                "Managed provisioning recovery could not remove storage.".to_owned()
                            })?;
                    }
                }
                state.store.clear_recovery(&recovery.env_id).map_err(|_| {
                    "Managed provisioning recovery could not be cleared.".to_owned()
                })?;
            }
            RecoveryOperation::Rebuild => {
                let current = state.ledger.inspect(&recovery.env_id).ok().cloned();
                match current {
                    Some(current) if current.namespace == recovery.namespace => {
                        if let Some(replacement) = recovery.replacement_namespace.as_deref() {
                            drop_recovery_namespace(&mut session, &recovery, replacement)?;
                        }
                        state
                            .ledger
                            .recover_rebuild(&recovery.env_id)
                            .map_err(|_| {
                                "Managed rebuild recovery could not restore the ledger.".to_owned()
                            })?;
                        let ready = state
                            .ledger
                            .inspect(&recovery.env_id)
                            .map_err(|_| "Managed rebuild recovery record disappeared.".to_owned())?
                            .clone();
                        state.store.save_environment(&ready).map_err(|_| {
                            "Managed rebuild recovery state could not be saved.".to_owned()
                        })?;
                    }
                    Some(current)
                        if recovery.replacement_namespace.as_deref()
                            == Some(current.namespace.as_str()) =>
                    {
                        drop_recovery_namespace(&mut session, &recovery, &recovery.namespace)?;
                        state
                            .ledger
                            .recover_rebuild(&recovery.env_id)
                            .map_err(|_| {
                                "Managed rebuild recovery could not restore the ledger.".to_owned()
                            })?;
                        let ready = state
                            .ledger
                            .inspect(&recovery.env_id)
                            .map_err(|_| "Managed rebuild recovery record disappeared.".to_owned())?
                            .clone();
                        state.store.save_environment(&ready).map_err(|_| {
                            "Managed rebuild recovery state could not be saved.".to_owned()
                        })?;
                    }
                    None => {
                        if let Some(replacement) = recovery.replacement_namespace.as_deref() {
                            drop_recovery_namespace(&mut session, &recovery, replacement)?;
                        }
                        drop_recovery_namespace(&mut session, &recovery, &recovery.namespace)?;
                    }
                    Some(_) => {
                        return Err(
                            "Managed rebuild recovery identity does not match the ledger."
                                .to_owned(),
                        );
                    }
                }
                state
                    .store
                    .clear_recovery(&recovery.env_id)
                    .map_err(|_| "Managed rebuild recovery could not be cleared.".to_owned())?;
            }
            RecoveryOperation::Delete => {
                drop_recovery_namespace(&mut session, &recovery, &recovery.namespace)?;
                if state.ledger.inspect(&recovery.env_id).is_ok() {
                    state
                        .ledger
                        .remove_recovered(&recovery.env_id)
                        .map_err(|_| {
                            "Managed delete recovery could not remove the ledger record.".to_owned()
                        })?;
                    state
                        .store
                        .delete_environment(&recovery.env_id)
                        .map_err(|_| {
                            "Managed delete recovery could not remove storage.".to_owned()
                        })?;
                }
                state
                    .store
                    .clear_recovery(&recovery.env_id)
                    .map_err(|_| "Managed delete recovery could not be cleared.".to_owned())?;
            }
        }
    }
    Ok(())
}

fn ownership_nonce(kind: &str, env_id: &str) -> String {
    format!(
        "nonce-{kind}-{env_id}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    )
}

fn environment_summary(record: &EnvironmentRecord) -> Result<ManagedEnvironmentSummary, String> {
    managed_environment_summary(record)
        .ok_or_else(|| "Only managed environments can be returned by this command.".to_owned())
}

fn provision_record(
    state: &mut ManagedEnvironmentState,
    record: EnvironmentRecord,
) -> Result<ManagedEnvironmentSummary, String> {
    state
        .ledger
        .create(record.clone())
        .map_err(|_| "Managed environment could not be reserved.".to_owned())?;
    provision_reserved_record(state, record)
}

fn provision_reserved_record(
    state: &mut ManagedEnvironmentState,
    record: EnvironmentRecord,
) -> Result<ManagedEnvironmentSummary, String> {
    let env_id = record.env_id.clone();
    let result = provision_reserved_record_inner(state, record);
    if result.is_err() {
        let _ = state.ledger.mark_error(&env_id);
        if let Ok(error_record) = state.ledger.inspect(&env_id).cloned() {
            let _ = state.store.save_environment(&error_record);
        }
    }
    result
}

fn provision_reserved_record_inner(
    state: &mut ManagedEnvironmentState,
    record: EnvironmentRecord,
) -> Result<ManagedEnvironmentSummary, String> {
    let recovery = RecoveryRecord {
        env_id: record.env_id.clone(),
        operation: RecoveryOperation::Provision,
        instance_id: record.instance_id.clone(),
        ownership_nonce: record.ownership_nonce.clone(),
        namespace: record.namespace.clone(),
        replacement_namespace: None,
    };
    state
        .store
        .save_recovery(&recovery)
        .map_err(|_| "Managed environment recovery intent could not be saved.".to_owned())?;
    state
        .store
        .save_environment(&record)
        .map_err(|_| "Managed environment reservation could not be saved.".to_owned())?;
    let connection = state
        .runtime
        .as_ref()
        .ok_or_else(|| "Managed PostgreSQL runtime is unavailable.".to_owned())?
        .connection_string();
    let schema_sql = managed_pack_schema(state, &record)?;

    let environment = ManagedEnvironment::new(
        record.env_id.clone(),
        record.instance_id.clone(),
        record.namespace.clone(),
        record.ownership_nonce.clone(),
    )
    .map_err(|_| "Managed environment identity failed policy validation.".to_owned())?;
    let mut session = PostgresSession::connect(&connection)
        .map_err(|_| "Managed PostgreSQL runtime connection failed.".to_owned())?;
    session
        .provision_managed_namespace_with_schema(&environment, "sqljudge_runner", &schema_sql)
        .map_err(|_| "Managed environment schema provisioning failed.".to_owned())?;
    state
        .ledger
        .mark_ready(&record.env_id)
        .map_err(|_| "Managed environment could not enter the ready state.".to_owned())?;
    let ready = state
        .ledger
        .inspect(&record.env_id)
        .map_err(|_| "Managed environment could not be read after provisioning.".to_owned())?
        .clone();
    state
        .store
        .save_environment(&ready)
        .map_err(|_| "Managed environment ready state could not be saved.".to_owned())?;
    state
        .store
        .clear_recovery(&record.env_id)
        .map_err(|_| "Managed environment recovery intent could not be cleared.".to_owned())?;
    environment_summary(&ready)
}

fn managed_pack_identity(
    state: &ManagedEnvironmentState,
    pack_id: &str,
) -> Result<(String, String), String> {
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    let pack = PackImporter::new(registry)
        .load_validated(pack_id)
        .map_err(|_| "Managed pack registry validation failed.".to_owned())?
        .ok_or_else(|| "The requested managed pack is not installed.".to_owned())?;
    Ok((pack.version().to_owned(), pack.digest().to_owned()))
}

fn managed_pack_schema(
    state: &ManagedEnvironmentState,
    environment: &EnvironmentRecord,
) -> Result<String, String> {
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    let pack = PackImporter::new(registry)
        .load_validated_exact(
            &environment.pack_id,
            &environment.pack_version,
            &environment.pack_digest,
        )
        .map_err(|_| "Managed pack registry validation failed.".to_owned())?
        .ok_or_else(|| "The exact managed pack is not installed.".to_owned())?;
    pack.schema_sql()
        .map_err(|_| "Managed pack schema could not be loaded.".to_owned())
}

fn import_pack_value(
    request: ImportPackRequest,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<PackImportResult, String> {
    let path = request.path.trim();
    if path.is_empty() {
        return Err("A local pack path is required.".to_owned());
    }
    let data = PackData::from_path(path).map_err(|_| "Pack source could not be loaded.")?;
    let pack = data
        .validate()
        .map_err(|_| "Pack failed structural validation.")?;
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)?;
    let connection = state
        .runtime
        .as_ref()
        .ok_or_else(|| "Managed PostgreSQL runtime is unavailable.".to_owned())?
        .connection_string();
    let mut session = ManagedPostgresSession::connect(&connection, &state.runtime_instance_id)
        .map_err(|_| "Managed PostgreSQL runtime ownership could not be verified.")?;
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    let published = PackImporter::new(registry)
        .import(&pack, &mut session, "sqljudge_runner")
        .map_err(|error| match error {
            PackError::RuntimeValidation { problem, test } => {
                format!("Pack runtime validation failed for {problem}/{test}.")
            }
            _ => "Pack runtime validation or publication failed.".to_owned(),
        })?;
    Ok(PackImportResult {
        status: if published.already_installed {
            "already_installed".to_owned()
        } else {
            "published".to_owned()
        },
        pack_id: pack.id().to_owned(),
        version: pack.version().to_owned(),
        digest: pack.digest().to_owned(),
        message: "Pack validated and published for managed use.".to_owned(),
    })
}

fn list_installed_packs_value(
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<Vec<problem_pack::PackCatalogEntry>, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    PackImporter::new(registry)
        .list_catalog()
        .map_err(|_| "Installed pack catalog validation failed.".to_owned())
}

fn export_backup_value(state: &Mutex<ManagedEnvironmentState>) -> Result<BackupSnapshot, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    let snapshot = export_metadata_backup(&state.database_path)
        .map_err(|_| "Metadata backup could not be exported.".to_owned())?;
    validate_backup_pack_registry(&snapshot, &state)?;
    let encoded = serde_json::to_vec(&snapshot)
        .map_err(|_| "Metadata backup could not be encoded.".to_owned())?;
    if encoded.len() > MAX_BACKUP_BYTES {
        return Err("Metadata backup exceeds the supported size limit.".to_owned());
    }
    Ok(snapshot)
}

fn restore_backup_value(
    snapshot: BackupSnapshot,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<(), String> {
    let encoded = serde_json::to_vec(&snapshot)
        .map_err(|_| "Metadata backup could not be encoded.".to_owned())?;
    if encoded.len() > MAX_BACKUP_BYTES {
        return Err("Metadata backup exceeds the supported size limit.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    validate_backup_pack_registry(&snapshot, &state)?;
    restore_metadata_backup(&state.database_path, &snapshot)
        .map_err(|_| "Metadata backup failed validation or restore.".to_owned())?;
    let records = state
        .store
        .list_environments()
        .map_err(|_| "Restored environment metadata could not be read.".to_owned())?;
    state.ledger = EnvironmentLedger::restore(records)
        .map_err(|_| "Restored environment metadata failed policy validation.".to_owned())?;
    Ok(())
}

fn validate_backup_pack_registry(
    snapshot: &BackupSnapshot,
    state: &ManagedEnvironmentState,
) -> Result<(), String> {
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    let importer = PackImporter::new(registry);
    for environment in snapshot
        .environments
        .iter()
        .filter(|environment| environment.mode == "managed")
    {
        let installed = importer
            .load_validated_exact(
                &environment.pack_id,
                &environment.pack_version,
                &environment.pack_digest,
            )
            .map_err(|_| "Metadata backup pack validation failed.".to_owned())?;
        if installed.is_none() {
            return Err("Metadata backup references a pack that is not imported.".to_owned());
        }
    }
    Ok(())
}

fn restore_backup_json_value(
    snapshot_json: &str,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<(), String> {
    if snapshot_json.len() > MAX_BACKUP_BYTES {
        return Err("Metadata backup exceeds the supported size limit.".to_owned());
    }
    let snapshot = serde_json::from_str::<BackupSnapshot>(snapshot_json)
        .map_err(|_| "Metadata backup could not be decoded.".to_owned())?;
    restore_backup_value(snapshot, state)
}

fn secret_ref_for_profile(profile_id: &str) -> String {
    format!("vault://sql-judge/{profile_id}")
}

fn connection_profile_summary(profile: ConnectionProfile) -> ConnectionProfileSummary {
    ConnectionProfileSummary {
        profile_id: profile.profile_id,
        display_name: profile.display_name,
        engine: profile.engine,
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        tls_mode: profile.tls_mode,
        has_secret: profile.secret_ref.is_some(),
    }
}

fn list_connection_profiles_value(
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<Vec<ConnectionProfileSummary>, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    state
        .store
        .list_connection_profiles()
        .map(|profiles| {
            profiles
                .into_iter()
                .map(connection_profile_summary)
                .collect()
        })
        .map_err(|_| "Connection profiles could not be read.".to_owned())
}

fn save_connection_profile_value<V: SecretVault>(
    request: SaveConnectionProfileRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &mut V,
) -> Result<ConnectionProfileSummary, String> {
    if request.secret.as_deref().is_some_and(str::is_empty) {
        return Err("The connection secret cannot be empty.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    let existing = state
        .store
        .get_connection_profile(&request.profile_id)
        .map_err(|_| "Connection profile could not be read.".to_owned())?;
    let secret_ref = request
        .secret
        .as_ref()
        .map(|_| secret_ref_for_profile(&request.profile_id))
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|profile| profile.secret_ref.clone())
        });
    let profile = ConnectionProfile {
        profile_id: request.profile_id,
        display_name: request.display_name,
        engine: request.engine,
        host: request.host,
        port: request.port,
        database: request.database,
        username: request.username,
        tls_mode: request.tls_mode,
        secret_ref,
    };
    let previous_secret = if request.secret.is_some() {
        existing
            .as_ref()
            .and_then(|profile| profile.secret_ref.as_deref())
            .map(|secret_ref| {
                vault
                    .get_secret(secret_ref)
                    .map_err(|_| "Credential vault is unavailable.".to_owned())
            })
            .transpose()?
    } else {
        None
    };
    if let (Some(secret), Some(secret_ref)) =
        (request.secret.as_deref(), profile.secret_ref.as_deref())
    {
        vault
            .set_secret(secret_ref, secret)
            .map_err(|_| "Credential vault is unavailable.".to_owned())?;
    }
    if let Err(error) = state.store.save_connection_profile(&profile) {
        if let Some(secret_ref) = profile.secret_ref.as_deref() {
            match previous_secret.as_ref() {
                Some(Some(previous)) => {
                    let _ = vault.set_secret(secret_ref, previous);
                }
                _ => {
                    let _ = vault.delete_secret(secret_ref);
                }
            }
        }
        return Err(match error {
            app_storage::StorageError::InvalidConnectionProfile(_) => {
                "The connection profile is invalid.".to_owned()
            }
            _ => "Connection profile could not be saved.".to_owned(),
        });
    }
    Ok(connection_profile_summary(profile))
}

fn delete_connection_profile_value<V: SecretVault>(
    profile_id: &str,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &mut V,
) -> Result<(), String> {
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    let profile = state
        .store
        .get_connection_profile(profile_id)
        .map_err(|_| "Connection profile could not be read.".to_owned())?;
    if let Some(profile) = profile
        && let Some(secret_ref) = profile.secret_ref.as_deref()
    {
        vault
            .delete_secret(secret_ref)
            .map_err(|_| "Credential vault is unavailable.".to_owned())?;
    }
    state
        .store
        .delete_connection_profile(profile_id)
        .map_err(|_| "Connection profile could not be deleted.".to_owned())
}

fn external_profile_config_value<V: SecretVault>(
    profile_id: &str,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<ExternalPostgresConfig, ExternalCommandError> {
    let state = state.lock().map_err(|_| ExternalCommandError {
        code: "external_database_error".to_owned(),
        message: "Managed environment state is busy.".to_owned(),
    })?;
    let profile = state
        .store
        .get_connection_profile(profile_id)
        .map_err(|_| ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "Connection profile could not be read.".to_owned(),
        })?
        .ok_or_else(|| ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "Connection profile was not found.".to_owned(),
        })?;
    if profile.engine != "postgresql" {
        return Err(ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "This operation requires a PostgreSQL profile.".to_owned(),
        });
    }
    let secret_ref = profile.secret_ref.ok_or_else(|| ExternalCommandError {
        code: "secret_unavailable".to_owned(),
        message: "No credential is stored for this connection profile.".to_owned(),
    })?;
    let password = vault
        .get_secret(&secret_ref)
        .map_err(|_| ExternalCommandError {
            code: "secret_unavailable".to_owned(),
            message: "The credential vault is unavailable.".to_owned(),
        })?
        .ok_or_else(|| ExternalCommandError {
            code: "secret_unavailable".to_owned(),
            message: "The connection credential is unavailable for this session.".to_owned(),
        })?;
    let tls_mode = match profile.tls_mode.as_str() {
        "verify_full" => ExternalTlsMode::VerifyFull,
        "verify_ca" => ExternalTlsMode::VerifyCa,
        "require" => ExternalTlsMode::Require,
        "disable" => ExternalTlsMode::Disable,
        _ => {
            return Err(ExternalCommandError {
                code: "profile_invalid".to_owned(),
                message: "The connection TLS policy is invalid.".to_owned(),
            });
        }
    };
    Ok(ExternalPostgresConfig {
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        password,
        tls_mode,
        allow_insecure_local: false,
    })
}

fn external_sqlserver_profile_config_value<V: SecretVault>(
    profile_id: &str,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<SqlServerConfig, ExternalCommandError> {
    let state = state.lock().map_err(|_| ExternalCommandError {
        code: "external_database_error".to_owned(),
        message: "Managed environment state is busy.".to_owned(),
    })?;
    let profile = state
        .store
        .get_connection_profile(profile_id)
        .map_err(|_| ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "Connection profile could not be read.".to_owned(),
        })?
        .ok_or_else(|| ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "Connection profile was not found.".to_owned(),
        })?;
    if profile.engine != "sqlserver" {
        return Err(ExternalCommandError {
            code: "profile_invalid".to_owned(),
            message: "This operation requires a SQL Server profile.".to_owned(),
        });
    }
    let secret_ref = profile.secret_ref.ok_or_else(|| ExternalCommandError {
        code: "secret_unavailable".to_owned(),
        message: "No credential is stored for this connection profile.".to_owned(),
    })?;
    let password = vault
        .get_secret(&secret_ref)
        .map_err(|_| ExternalCommandError {
            code: "secret_unavailable".to_owned(),
            message: "The credential vault is unavailable.".to_owned(),
        })?
        .ok_or_else(|| ExternalCommandError {
            code: "secret_unavailable".to_owned(),
            message: "The connection credential is unavailable for this session.".to_owned(),
        })?;
    let tls_mode = match profile.tls_mode.as_str() {
        "verify_full" => SqlServerTlsMode::VerifyFull,
        "verify_ca" => SqlServerTlsMode::VerifyCa,
        "require" => SqlServerTlsMode::Require,
        "disable" => SqlServerTlsMode::Disable,
        "trust_server_certificate" => SqlServerTlsMode::TrustServerCertificate,
        _ => {
            return Err(ExternalCommandError {
                code: "profile_invalid".to_owned(),
                message: "The connection TLS policy is invalid.".to_owned(),
            });
        }
    };
    Ok(SqlServerConfig {
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        password,
        tls_mode,
    })
}

fn map_external_error(error: ExternalPostgresError) -> ExternalCommandError {
    let (code, message) = match error {
        ExternalPostgresError::InvalidProfile(_) => {
            ("profile_invalid", "The connection profile is invalid.")
        }
        ExternalPostgresError::TlsPolicyRejected => (
            "tls_failed",
            "The selected TLS policy is not permitted for external connections.",
        ),
        ExternalPostgresError::TlsFailed => (
            "tls_failed",
            "TLS negotiation or certificate validation failed.",
        ),
        ExternalPostgresError::AuthenticationFailed => {
            ("authentication_failed", "PostgreSQL authentication failed.")
        }
        ExternalPostgresError::ServerUnreachable => (
            "server_unreachable",
            "The PostgreSQL server could not be reached.",
        ),
        ExternalPostgresError::QueryRejected => (
            "query_rejected",
            "Only a single read-only SELECT is permitted.",
        ),
        ExternalPostgresError::QueryTimeout => (
            "query_timeout",
            "The external query exceeded its time limit.",
        ),
        ExternalPostgresError::ResultLimitExceeded => (
            "result_limit_exceeded",
            "The external result exceeded the configured limit.",
        ),
        ExternalPostgresError::InvalidLimits(_) => {
            ("profile_invalid", "The requested query limits are invalid.")
        }
        ExternalPostgresError::UnsupportedType(_) => (
            "external_database_error",
            "The external result contains an unsupported PostgreSQL type.",
        ),
        ExternalPostgresError::DatabaseError => (
            "external_database_error",
            "The external PostgreSQL operation failed.",
        ),
    };
    ExternalCommandError {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn map_sqlserver_error(error: SqlServerError) -> ExternalCommandError {
    let (code, message) = match error {
        SqlServerError::InvalidProfile(_) => {
            ("profile_invalid", "The connection profile is invalid.")
        }
        SqlServerError::TlsPolicyRejected | SqlServerError::TlsFailed => (
            "tls_failed",
            "TLS negotiation or certificate validation failed.",
        ),
        SqlServerError::AuthenticationFailed => {
            ("authentication_failed", "SQL Server authentication failed.")
        }
        SqlServerError::ServerUnreachable => {
            ("server_unreachable", "The SQL Server could not be reached.")
        }
        SqlServerError::QueryRejected => (
            "query_rejected",
            "Only a single read-only T-SQL SELECT is permitted.",
        ),
        SqlServerError::QueryTimeout => (
            "query_timeout",
            "The external query exceeded its time limit.",
        ),
        SqlServerError::ResultLimitExceeded => (
            "result_limit_exceeded",
            "The external result exceeded the configured limit.",
        ),
        SqlServerError::InvalidLimits(_) => {
            ("profile_invalid", "The requested query limits are invalid.")
        }
        SqlServerError::UnsupportedType(_) => (
            "external_database_error",
            "The SQL Server result contains an unsupported type.",
        ),
        SqlServerError::DatabaseError => (
            "external_database_error",
            "The SQL Server operation failed.",
        ),
    };
    ExternalCommandError {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn test_external_connection_value<V: SecretVault>(
    request: ExternalProfileRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<ExternalConnectionInfoResponse, ExternalCommandError> {
    let config = external_profile_config_value(&request.profile_id, state, vault)?;
    let mut explorer = ExternalPostgresExplorer::connect(config).map_err(map_external_error)?;
    let info = explorer.test_connection().map_err(map_external_error)?;
    Ok(external_connection_info_response(info))
}

fn list_external_metadata_value<V: SecretVault>(
    request: ExternalProfileRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
    limits: QueryLimits,
) -> Result<ExternalMetadataResponse, ExternalCommandError> {
    let config = external_profile_config_value(&request.profile_id, state, vault)?;
    let metadata = with_verified_postgres_explorer(
        || ExternalPostgresExplorer::connect(config).map_err(map_external_error),
        |explorer| explorer.test_connection().map_err(map_external_error),
        |explorer| explorer.list_metadata(limits).map_err(map_external_error),
    )?;
    Ok(external_metadata_response(metadata))
}

fn execute_external_select_value<V: SecretVault>(
    request: ExternalSelectRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<ExternalQueryResponse, ExternalCommandError> {
    validate_external_sql_request(&request.sql)?;
    let limits = validate_external_query_limits(
        request.statement_timeout_ms,
        request.max_rows,
        request.max_bytes,
    )?;
    let config = external_profile_config_value(&request.profile_id, state, vault)?;
    let result = with_verified_postgres_explorer(
        || ExternalPostgresExplorer::connect(config).map_err(map_external_error),
        |explorer| explorer.test_connection().map_err(map_external_error),
        |explorer| {
            explorer
                .execute_select(&request.sql, limits)
                .map_err(map_external_error)
        },
    )?;
    let (columns, rows) = preview_response(Some(&result));
    Ok(ExternalQueryResponse { columns, rows })
}

fn require_postgres_read_only(info: &ExternalConnectionInfo) -> Result<(), ExternalCommandError> {
    if info.read_only {
        return Ok(());
    }
    Err(ExternalCommandError {
        code: "read_only_unverified".to_owned(),
        message: "The PostgreSQL profile is not verified as read-only.".to_owned(),
    })
}

fn with_verified_postgres_explorer<E, T, Connect, Probe, Operation>(
    connect: Connect,
    probe: Probe,
    operation: Operation,
) -> Result<T, ExternalCommandError>
where
    Connect: FnOnce() -> Result<E, ExternalCommandError>,
    Probe: FnOnce(&mut E) -> Result<ExternalConnectionInfo, ExternalCommandError>,
    Operation: FnOnce(&mut E) -> Result<T, ExternalCommandError>,
{
    let mut explorer = connect()?;
    let info = probe(&mut explorer)?;
    require_postgres_read_only(&info)?;
    operation(&mut explorer)
}

fn test_external_sqlserver_value<V: SecretVault>(
    request: ExternalProfileRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<ExternalConnectionInfoResponse, ExternalCommandError> {
    let config = external_sqlserver_profile_config_value(&request.profile_id, state, vault)?;
    let explorer = SqlServerExplorer::connect(config).map_err(map_sqlserver_error)?;
    let info = explorer.test_connection().map_err(map_sqlserver_error)?;
    Ok(ExternalConnectionInfoResponse {
        database: info.database,
        username: info.username,
        server_version: info.server_version,
        read_only: info.read_only,
    })
}

fn require_sqlserver_read_only(info: &SqlServerConnectionInfo) -> Result<(), ExternalCommandError> {
    if info.read_only {
        return Ok(());
    }
    Err(ExternalCommandError {
        code: "read_only_unverified".to_owned(),
        message: "The SQL Server profile is not verified as read-only.".to_owned(),
    })
}

fn with_verified_sqlserver_explorer<E, T, Connect, Probe, Operation>(
    connect: Connect,
    probe: Probe,
    operation: Operation,
) -> Result<T, ExternalCommandError>
where
    Connect: FnOnce() -> Result<E, ExternalCommandError>,
    Probe: FnOnce(&E) -> Result<SqlServerConnectionInfo, ExternalCommandError>,
    Operation: FnOnce(E) -> Result<T, ExternalCommandError>,
{
    let explorer = connect()?;
    let info = probe(&explorer)?;
    require_sqlserver_read_only(&info)?;
    operation(explorer)
}

fn list_external_sqlserver_metadata_value<V: SecretVault>(
    request: ExternalProfileRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
    limits: QueryLimits,
) -> Result<ExternalMetadataResponse, ExternalCommandError> {
    let config = external_sqlserver_profile_config_value(&request.profile_id, state, vault)?;
    let metadata = with_verified_sqlserver_explorer(
        || SqlServerExplorer::connect(config).map_err(map_sqlserver_error),
        |explorer| explorer.test_connection().map_err(map_sqlserver_error),
        |explorer| explorer.list_metadata(limits).map_err(map_sqlserver_error),
    )?;
    Ok(ExternalMetadataResponse {
        tables: metadata
            .tables
            .into_iter()
            .map(|table| ExternalTableResponse {
                schema: table.schema,
                name: table.name,
                kind: table.kind,
                columns: table
                    .columns
                    .into_iter()
                    .map(|column| ExternalColumnResponse {
                        name: column.name,
                        data_type: column.data_type,
                    })
                    .collect(),
            })
            .collect(),
        truncated: metadata.truncated,
    })
}

fn execute_external_sqlserver_select_value<V: SecretVault>(
    request: ExternalSelectRequest,
    state: &Mutex<ManagedEnvironmentState>,
    vault: &V,
) -> Result<ExternalQueryResponse, ExternalCommandError> {
    validate_external_sql_request(&request.sql)?;
    let limits = validate_external_query_limits(
        request.statement_timeout_ms,
        request.max_rows,
        request.max_bytes,
    )?;
    let config = external_sqlserver_profile_config_value(&request.profile_id, state, vault)?;
    let sql = request.sql;
    let result = with_verified_sqlserver_explorer(
        || SqlServerExplorer::connect(config).map_err(map_sqlserver_error),
        |explorer| explorer.test_connection().map_err(map_sqlserver_error),
        |explorer| {
            explorer
                .execute_select(&sql, limits)
                .map_err(map_sqlserver_error)
        },
    )?;
    let (columns, rows) = preview_response(Some(&result));
    Ok(ExternalQueryResponse { columns, rows })
}

fn external_connection_info_response(
    info: ExternalConnectionInfo,
) -> ExternalConnectionInfoResponse {
    ExternalConnectionInfoResponse {
        database: info.database,
        username: info.username,
        server_version: info.server_version,
        read_only: info.read_only,
    }
}

fn external_metadata_response(metadata: ExternalMetadata) -> ExternalMetadataResponse {
    ExternalMetadataResponse {
        tables: metadata
            .tables
            .into_iter()
            .map(external_table_response)
            .collect(),
        truncated: metadata.truncated,
    }
}

fn external_table_response(table: ExternalTable) -> ExternalTableResponse {
    ExternalTableResponse {
        schema: table.schema,
        name: table.name,
        kind: table.kind,
        columns: table
            .columns
            .into_iter()
            .map(|column| ExternalColumnResponse {
                name: column.name,
                data_type: column.data_type,
            })
            .collect(),
    }
}

fn create_managed_environment_value(
    request: CreateEnvironmentRequest,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<ManagedEnvironmentSummary, String> {
    if request.pack_id.trim().is_empty()
        || request.env_id.trim().is_empty()
        || request.namespace.trim().is_empty()
    {
        return Err("The requested managed pack or environment identity is invalid.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)?;
    let pack_version = request.pack_version.trim().to_owned();
    let pack_digest = request.pack_digest.trim().to_owned();
    if pack_version.is_empty() || pack_digest.is_empty() {
        return Err("An exact managed pack version and digest are required.".to_owned());
    }
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    if PackImporter::new(registry)
        .load_validated_exact(&request.pack_id, &pack_version, &pack_digest)
        .map_err(|_| "Managed pack registry validation failed.".to_owned())?
        .is_none()
    {
        return Err("The exact managed pack is not installed.".to_owned());
    }
    let record = EnvironmentRecord {
        env_id: request.env_id.clone(),
        instance_id: state.runtime_instance_id.clone(),
        pack_id: request.pack_id,
        pack_version,
        pack_digest,
        namespace: request.namespace,
        ownership_nonce: ownership_nonce("create", &request.env_id),
        mode: EnvironmentMode::Managed,
        state: EnvironmentState::Provisioning,
    };
    provision_record(&mut state, record)
}

fn clone_managed_environment_value(
    request: CloneEnvironmentRequest,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<ManagedEnvironmentSummary, String> {
    if request.source_env_id.trim().is_empty()
        || request.env_id.trim().is_empty()
        || request.namespace.trim().is_empty()
    {
        return Err("The requested managed environment identity is invalid.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)?;
    let replacement = EnvironmentRecord {
        env_id: request.env_id.clone(),
        instance_id: state.runtime_instance_id.clone(),
        pack_id: String::new(),
        pack_version: String::new(),
        pack_digest: String::new(),
        namespace: request.namespace,
        ownership_nonce: ownership_nonce("clone", &request.env_id),
        mode: EnvironmentMode::Managed,
        state: EnvironmentState::Provisioning,
    };
    let record = state
        .ledger
        .clone_environment(&request.source_env_id, replacement)
        .map_err(|_| "Managed environment could not be cloned.".to_owned())?;
    provision_reserved_record(&mut state, record)
}

fn lifecycle_failure(state: &mut ManagedEnvironmentState, env_id: &str, message: &str) -> String {
    if state.ledger.mark_error(env_id).is_ok()
        && let Ok(record) = state.ledger.inspect(env_id).cloned()
    {
        let _ = state.store.save_environment(&record);
    }
    message.to_owned()
}

fn reset_managed_environment_value(
    request: ResetEnvironmentRequest,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<ManagedEnvironmentSummary, String> {
    if request.env_id.trim().is_empty() || request.namespace.trim().is_empty() {
        return Err("The requested reset identity is invalid.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)?;
    let current = state
        .ledger
        .inspect(&request.env_id)
        .map_err(|_| "Managed environment was not found.".to_owned())?
        .clone();
    let schema_sql = managed_pack_schema(&state, &current)?;
    if current.namespace == request.namespace {
        return Err("Reset requires a replacement namespace.".to_owned());
    }
    let action = DestructiveAction::Reset {
        env_id: request.env_id.clone(),
        instance_id: request.instance_id,
        ownership_nonce: current.ownership_nonce.clone(),
        confirmed: request.confirmed,
    };
    state
        .ledger
        .begin_rebuild(&request.env_id, action.clone())
        .map_err(|_| "Managed environment reset authorization failed.".to_owned())?;
    let rebuilding = state
        .ledger
        .inspect(&request.env_id)
        .map_err(|_| "Managed environment reset state could not be read.".to_owned())?
        .clone();
    let recovery = RecoveryRecord {
        env_id: current.env_id.clone(),
        operation: RecoveryOperation::Rebuild,
        instance_id: current.instance_id.clone(),
        ownership_nonce: current.ownership_nonce.clone(),
        namespace: current.namespace.clone(),
        replacement_namespace: Some(request.namespace.clone()),
    };
    state.store.save_recovery(&recovery).map_err(|_| {
        lifecycle_failure(
            &mut state,
            &request.env_id,
            "Managed reset recovery intent could not be saved.",
        )
    })?;
    state
        .store
        .save_environment(&rebuilding)
        .map_err(|_| "Managed environment reset state could not be saved.".to_owned())?;

    let replacement = EnvironmentRecord {
        env_id: current.env_id.clone(),
        instance_id: current.instance_id.clone(),
        pack_id: current.pack_id.clone(),
        pack_version: current.pack_version.clone(),
        pack_digest: current.pack_digest.clone(),
        namespace: request.namespace,
        ownership_nonce: current.ownership_nonce.clone(),
        mode: EnvironmentMode::Managed,
        state: EnvironmentState::Ready,
    };
    let replacement_environment = ManagedEnvironment::new(
        replacement.env_id.clone(),
        replacement.instance_id.clone(),
        replacement.namespace.clone(),
        replacement.ownership_nonce.clone(),
    )
    .map_err(|_| {
        lifecycle_failure(
            &mut state,
            &request.env_id,
            "Managed replacement identity failed policy validation.",
        )
    })?;
    let old_environment = ManagedEnvironment::new(
        current.env_id.clone(),
        current.instance_id.clone(),
        current.namespace.clone(),
        current.ownership_nonce.clone(),
    )
    .map_err(|_| "Managed environment identity failed policy validation.".to_owned())?;
    let connection = state
        .runtime
        .as_ref()
        .ok_or_else(|| "Managed PostgreSQL runtime is unavailable.".to_owned())?
        .connection_string();
    let mut session = PostgresSession::connect(&connection)
        .map_err(|_| "Managed PostgreSQL runtime connection failed.".to_owned())?;
    session
        .provision_managed_namespace_with_schema(
            &replacement_environment,
            "sqljudge_runner",
            &schema_sql,
        )
        .map_err(|_| "Managed replacement schema provisioning failed.".to_owned())?;
    state
        .ledger
        .finish_rebuild(&request.env_id, replacement, action, |_| true)
        .map_err(|_| "Managed replacement could not enter the ready state.".to_owned())?;
    let ready = state
        .ledger
        .inspect(&request.env_id)
        .map_err(|_| "Managed replacement state could not be read.".to_owned())?
        .clone();
    state
        .store
        .save_environment(&ready)
        .map_err(|_| "Managed replacement state could not be saved.".to_owned())?;
    if session.drop_managed_namespace(&old_environment).is_err() {
        state
            .ledger
            .mark_error(&request.env_id)
            .map_err(|_| "Managed reset cleanup failed.".to_owned())?;
        let error_state = state
            .ledger
            .inspect(&request.env_id)
            .map_err(|_| "Managed reset cleanup state could not be read.".to_owned())?
            .clone();
        state
            .store
            .save_environment(&error_state)
            .map_err(|_| "Managed reset cleanup state could not be saved.".to_owned())?;
        return Err("Managed reset replacement succeeded but old cleanup failed.".to_owned());
    }
    state
        .store
        .clear_recovery(&request.env_id)
        .map_err(|_| "Managed reset recovery intent could not be cleared.".to_owned())?;
    environment_summary(&ready)
}

fn delete_managed_environment_value(
    request: DeleteEnvironmentRequest,
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<(), String> {
    if request.env_id.trim().is_empty() {
        return Err("The requested delete identity is invalid.".to_owned());
    }
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)?;
    let current = state
        .ledger
        .inspect(&request.env_id)
        .map_err(|_| "Managed environment was not found.".to_owned())?
        .clone();
    let action = DestructiveAction::Delete {
        env_id: request.env_id.clone(),
        instance_id: request.instance_id,
        ownership_nonce: current.ownership_nonce.clone(),
        confirmed: request.confirmed,
    };
    state
        .ledger
        .begin_delete(&request.env_id, action.clone())
        .map_err(|_| "Managed environment delete authorization failed.".to_owned())?;
    let deleting = state
        .ledger
        .inspect(&request.env_id)
        .map_err(|_| "Managed environment delete state could not be read.".to_owned())?
        .clone();
    let recovery = RecoveryRecord {
        env_id: current.env_id.clone(),
        operation: RecoveryOperation::Delete,
        instance_id: current.instance_id.clone(),
        ownership_nonce: current.ownership_nonce.clone(),
        namespace: current.namespace.clone(),
        replacement_namespace: None,
    };
    state
        .store
        .save_recovery(&recovery)
        .map_err(|_| "Managed delete recovery intent could not be saved.".to_owned())?;
    state
        .store
        .save_environment(&deleting)
        .map_err(|_| "Managed environment delete state could not be saved.".to_owned())?;
    let environment = ManagedEnvironment::new(
        current.env_id.clone(),
        current.instance_id.clone(),
        current.namespace.clone(),
        current.ownership_nonce.clone(),
    )
    .map_err(|_| "Managed environment identity failed policy validation.".to_owned())?;
    let connection = state
        .runtime
        .as_ref()
        .ok_or_else(|| "Managed PostgreSQL runtime is unavailable.".to_owned())?
        .connection_string();
    let mut session = PostgresSession::connect(&connection)
        .map_err(|_| "Managed PostgreSQL runtime connection failed.".to_owned())?;
    if session.drop_managed_namespace(&environment).is_err() {
        state
            .ledger
            .mark_error(&request.env_id)
            .map_err(|_| "Managed delete cleanup failed.".to_owned())?;
        let error_state = state
            .ledger
            .inspect(&request.env_id)
            .map_err(|_| "Managed delete cleanup state could not be read.".to_owned())?
            .clone();
        state
            .store
            .save_environment(&error_state)
            .map_err(|_| "Managed delete cleanup state could not be saved.".to_owned())?;
        return Err("Managed environment cleanup failed.".to_owned());
    }
    state
        .ledger
        .finish_delete(&request.env_id, action)
        .map_err(|_| "Managed environment could not be removed from the ledger.".to_owned())?;
    state
        .store
        .delete_environment(&request.env_id)
        .map_err(|_| "Managed environment could not be removed from storage.".to_owned())?;
    state
        .store
        .clear_recovery(&request.env_id)
        .map_err(|_| "Managed delete recovery intent could not be cleared.".to_owned())?;
    Ok(())
}

fn managed_environment_summary(record: &EnvironmentRecord) -> Option<ManagedEnvironmentSummary> {
    if record.mode != EnvironmentMode::Managed {
        return None;
    }
    Some(ManagedEnvironmentSummary {
        env_id: record.env_id.clone(),
        instance_id: record.instance_id.clone(),
        pack_id: record.pack_id.clone(),
        pack_version: record.pack_version.clone(),
        pack_digest: record.pack_digest.clone(),
        namespace: record.namespace.clone(),
        mode: match record.mode {
            EnvironmentMode::Managed => "managed".to_owned(),
            EnvironmentMode::ExternalExplorer => "external_explorer".to_owned(),
        },
        state: match record.state {
            EnvironmentState::Provisioning => "provisioning".to_owned(),
            EnvironmentState::Ready => "ready".to_owned(),
            EnvironmentState::Running => "running".to_owned(),
            EnvironmentState::Rebuilding => "rebuilding".to_owned(),
            EnvironmentState::Deleting => "deleting".to_owned(),
            EnvironmentState::NeedsRecovery => "needs_recovery".to_owned(),
            EnvironmentState::Error => "error".to_owned(),
        },
    })
}

fn policy_error(message: &str) -> JudgeRunResult {
    JudgeRunResult {
        status: "error".to_owned(),
        verdict: Some("PolicyViolation".to_owned()),
        columns: Vec::new(),
        rows: Vec::new(),
        duration_ms: 0,
        message: message.to_owned(),
        tests: Vec::new(),
    }
}

fn verdict_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Accepted => "Accepted",
        Verdict::WrongAnswer => "WrongAnswer",
        Verdict::SyntaxError => "SyntaxError",
        Verdict::RuntimeError => "RuntimeError",
        Verdict::TimeLimitExceeded => "TimeLimitExceeded",
        Verdict::ResultLimitExceeded => "ResultLimitExceeded",
        Verdict::PolicyViolation => "PolicyViolation",
        Verdict::Cancelled => "Cancelled",
        Verdict::JudgeError => "JudgeError",
    }
}

fn preview_response(
    preview: Option<&QueryResult>,
) -> (Vec<JudgeColumn>, Vec<Vec<serde_json::Value>>) {
    let Some(preview) = preview else {
        return (Vec::new(), Vec::new());
    };
    let columns = preview
        .columns
        .iter()
        .map(|column| JudgeColumn {
            name: column.name.clone(),
            r#type: format!("{:?}", column.logical_type),
        })
        .collect();
    let rows = preview
        .rows
        .iter()
        .map(|row| row.iter().map(sql_value_json).collect())
        .collect();
    (columns, rows)
}

fn sql_value_json(value: &SqlValue) -> serde_json::Value {
    const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;

    match value {
        SqlValue::Null => serde_json::Value::Null,
        SqlValue::Bool(value) => serde_json::Value::Bool(*value),
        SqlValue::Signed(value)
            if (-MAX_SAFE_JSON_INTEGER..=MAX_SAFE_JSON_INTEGER).contains(value) =>
        {
            serde_json::Value::Number(serde_json::Number::from(*value))
        }
        SqlValue::Signed(value) => serde_json::Value::String(value.to_string()),
        SqlValue::Unsigned(value) if *value <= MAX_SAFE_JSON_INTEGER as u64 => {
            serde_json::Value::Number(serde_json::Number::from(*value))
        }
        SqlValue::Unsigned(value) => serde_json::Value::String(value.to_string()),
        SqlValue::Decimal(value)
        | SqlValue::Text(value)
        | SqlValue::Date(value)
        | SqlValue::Timestamp(value)
        | SqlValue::Timestamptz(value)
        | SqlValue::Uuid(value)
        | SqlValue::Json(value) => serde_json::Value::String(value.clone()),
        SqlValue::Float { bits } => serde_json::Number::from_f64(f64::from_bits(*bits))
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(format!("0x{bits:016x}"))),
        SqlValue::Bytes(value) => {
            serde_json::Value::String(value.iter().map(|byte| format!("{byte:02x}")).collect())
        }
    }
}

/// Maps orchestrator outcomes (already in manifest order) to the IPC shape. Only the
/// test ID, verdict name, user-query elapsed time and the manifest `hidden` flag
/// cross the boundary; expected rows and fixture SQL stay native-only.
fn judge_test_results(
    problem: &ValidatedProblem,
    outcomes: &[judge_postgres::TestOutcome],
) -> Vec<JudgeTestResult> {
    outcomes
        .iter()
        .map(|outcome| JudgeTestResult {
            id: outcome.id.clone(),
            verdict: verdict_name(outcome.verdict).to_owned(),
            elapsed_ms: outcome.elapsed_ms,
            hidden: problem
                .tests
                .iter()
                .find(|test| test.id == outcome.id)
                .is_none_or(|test| test.hidden),
        })
        .collect()
}

fn judge_cases(problem: &ValidatedProblem, submit: bool) -> Vec<JudgeCase<'_>> {
    problem
        .tests
        .iter()
        .filter(|test| submit || !test.hidden)
        .map(|test| JudgeCase {
            id: &test.id,
            schema_sql: &problem.schema_sql,
            fixture_sql: &test.fixture_sql,
            reference_sql: &problem.reference_sql,
            output: problem.output.clone(),
            limits: problem.limits,
        })
        .collect()
}

fn submission_id() -> String {
    format!(
        "submission-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    )
}

fn created_at() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(
            |_| "0".to_owned(),
            |duration| duration.as_secs().to_string(),
        )
}

fn judge_request(
    request: JudgeRunRequest,
    state: &Mutex<ManagedEnvironmentState>,
    expected_mode: &str,
) -> JudgeRunResult {
    if request.problem_id.trim().is_empty()
        || request.pack_id.trim().is_empty()
        || request.pack_version.trim().is_empty()
        || request.pack_digest.trim().is_empty()
        || request.sql.len() > 65_536
        || !matches!(request.mode.as_str(), "run" | "submit")
    {
        return policy_error("The judge request is malformed.");
    }
    if request.mode != expected_mode {
        return policy_error("The requested problem or judge mode is not available.");
    }
    let cancellation = register_cancellation(request.request_id.as_deref());
    let cancellation_token = cancellation.token.clone();
    let started = Instant::now();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(_) => {
            return JudgeRunResult {
                status: "error".to_owned(),
                verdict: Some("JudgeError".to_owned()),
                columns: Vec::new(),
                rows: Vec::new(),
                duration_ms: 0,
                message: "Managed environment state is busy.".to_owned(),
                tests: Vec::new(),
            };
        }
    };
    let registry = state
        .runtime_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("packs");
    let pack = match PackImporter::new(registry).load_validated_exact(
        &request.pack_id,
        &request.pack_version,
        &request.pack_digest,
    ) {
        Ok(Some(pack)) => pack,
        Ok(None) | Err(_) => return policy_error("The exact requested pack is not installed."),
    };
    let problem = match pack.problem(&request.problem_id) {
        Ok(Some(problem)) if problem.dialect == Dialect::Postgresql => problem,
        Ok(Some(_)) => {
            return policy_error("The requested problem is not supported by the managed judge.");
        }
        Ok(None) | Err(_) => {
            return policy_error("The requested problem is not available in the exact pack.");
        }
    };
    if judge_cases(&problem, expected_mode == "submit").is_empty() {
        return policy_error("The requested problem has no executable judge tests.");
    }
    if ensure_managed_runtime(&mut state).is_err() {
        return JudgeRunResult {
            status: "unavailable".to_owned(),
            verdict: None,
            columns: Vec::new(),
            rows: Vec::new(),
            duration_ms: started.elapsed().as_millis() as u64,
            message: "Managed PostgreSQL runtime could not be started.".to_owned(),
            tests: Vec::new(),
        };
    }
    let connection_string = match state.runtime.as_ref() {
        Some(runtime) => runtime.connection_string(),
        None => {
            return JudgeRunResult {
                status: "unavailable".to_owned(),
                verdict: None,
                columns: Vec::new(),
                rows: Vec::new(),
                duration_ms: started.elapsed().as_millis() as u64,
                message: "Managed PostgreSQL runtime is unavailable.".to_owned(),
                tests: Vec::new(),
            };
        }
    };
    let mut session = match PostgresSession::connect(&connection_string) {
        Ok(session) => session,
        Err(_) => {
            return JudgeRunResult {
                status: "error".to_owned(),
                verdict: Some("JudgeError".to_owned()),
                columns: Vec::new(),
                rows: Vec::new(),
                duration_ms: started.elapsed().as_millis() as u64,
                message: "Managed PostgreSQL runtime connection failed.".to_owned(),
                tests: Vec::new(),
            };
        }
    };
    let report = judge_submission_with_watchdog_token(
        &mut session,
        "sqljudge_runner",
        &judge_cases(&problem, expected_mode == "submit"),
        &request.sql,
        &cancellation_token,
        std::time::Duration::from_secs(5),
    );
    let duration_ms = started.elapsed().as_millis() as u64;
    let verdict = verdict_name(report.verdict).to_owned();
    let (columns, rows) = preview_response(report.preview.as_ref());
    let test_results = judge_test_results(&problem, &report.tests);
    // ADR-024: sample runs are recorded too, tagged by mode; only IDs, verdicts and timings.
    let tests = report
        .tests
        .iter()
        .map(|test| TestRecord {
            test_id: test.id.clone(),
            verdict: verdict_name(test.verdict).to_owned(),
            elapsed_ms: test.elapsed_ms,
        })
        .collect::<Vec<_>>();
    let submission = SubmissionRecord {
        submission_id: submission_id(),
        pack_id: pack.id().to_owned(),
        pack_version: pack.version().to_owned(),
        pack_hash: pack.digest().to_owned(),
        engine_version: "postgresql-managed".to_owned(),
        policy_version: "policy-v1".to_owned(),
        verdict: verdict.clone(),
        created_at: created_at(),
        problem_id: Some(problem.id.clone()),
        mode: expected_mode.to_owned(),
    };
    if state.history.save_submission(&submission, &tests).is_err() {
        return JudgeRunResult {
            status: "error".to_owned(),
            verdict: Some("JudgeError".to_owned()),
            columns: Vec::new(),
            rows: Vec::new(),
            duration_ms,
            message: "Judge completed but history could not be saved.".to_owned(),
            tests: Vec::new(),
        };
    }
    JudgeRunResult {
        status: "completed".to_owned(),
        verdict: Some(verdict),
        columns,
        rows,
        duration_ms,
        message: format!("{} test case(s) evaluated.", report.tests.len()),
        tests: test_results,
    }
}

#[tauri::command]
fn run_sample(
    request: JudgeRunRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> JudgeRunResult {
    judge_request(request, state.inner(), "run")
}

#[tauri::command]
fn submit_solution(
    request: JudgeRunRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> JudgeRunResult {
    judge_request(request, state.inner(), "submit")
}

#[tauri::command]
fn cancel_judge(request_id: String) -> bool {
    cancel_judge_value(&request_id)
}

#[tauri::command]
fn list_connection_profiles(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Vec<ConnectionProfileSummary>, String> {
    list_connection_profiles_value(state.inner())
}

#[tauri::command]
fn save_connection_profile(
    request: SaveConnectionProfileRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ConnectionProfileSummary, String> {
    save_connection_profile_value(request, state.inner(), &mut SessionSecretVault)
}

#[tauri::command]
fn delete_connection_profile(
    profile_id: String,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<(), String> {
    delete_connection_profile_value(&profile_id, state.inner(), &mut SessionSecretVault)
}

#[tauri::command]
fn test_external_connection(
    request: ExternalProfileRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalConnectionInfoResponse, ExternalCommandError> {
    test_external_connection_value(request, state.inner(), &SessionSecretVault)
}

#[tauri::command]
fn list_external_metadata(
    request: ExternalProfileRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalMetadataResponse, ExternalCommandError> {
    list_external_metadata_value(
        request,
        state.inner(),
        &SessionSecretVault,
        QueryLimits::new(2_000, 1_000, 512 * 1024),
    )
}

#[tauri::command]
fn execute_external_select(
    request: ExternalSelectRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalQueryResponse, ExternalCommandError> {
    execute_external_select_value(request, state.inner(), &SessionSecretVault)
}

#[tauri::command]
fn test_external_sqlserver(
    request: ExternalProfileRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalConnectionInfoResponse, ExternalCommandError> {
    test_external_sqlserver_value(request, state.inner(), &SessionSecretVault)
}

#[tauri::command]
fn list_external_sqlserver_metadata(
    request: ExternalProfileRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalMetadataResponse, ExternalCommandError> {
    list_external_sqlserver_metadata_value(
        request,
        state.inner(),
        &SessionSecretVault,
        QueryLimits::new(2_000, 1_000, 512 * 1024),
    )
}

#[tauri::command]
fn execute_external_sqlserver_select(
    request: ExternalSelectRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ExternalQueryResponse, ExternalCommandError> {
    execute_external_sqlserver_select_value(request, state.inner(), &SessionSecretVault)
}

#[tauri::command]
fn list_managed_environments(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Vec<ManagedEnvironmentSummary>, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    Ok(state
        .ledger
        .list()
        .into_iter()
        .filter_map(managed_environment_summary)
        .collect())
}

#[tauri::command]
fn inspect_managed_environment(
    env_id: String,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Option<ManagedEnvironmentSummary>, String> {
    if env_id.trim().is_empty() {
        return Err("Environment ID is required.".to_owned());
    }
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    Ok(state
        .ledger
        .inspect(&env_id)
        .ok()
        .and_then(managed_environment_summary))
}

#[tauri::command]
fn list_submission_history(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Vec<SubmissionHistorySummary>, String> {
    list_submission_history_value(state.inner())
}

fn list_submission_history_value(
    state: &Mutex<ManagedEnvironmentState>,
) -> Result<Vec<SubmissionHistorySummary>, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    state
        .history
        .list_submissions()
        .map(|records| {
            records
                .into_iter()
                .map(|record| SubmissionHistorySummary {
                    submission_id: record.submission_id,
                    pack_id: record.pack_id,
                    pack_version: record.pack_version,
                    pack_hash: record.pack_hash,
                    engine_version: record.engine_version,
                    policy_version: record.policy_version,
                    verdict: record.verdict,
                    created_at: record.created_at,
                    problem_id: record.problem_id,
                    mode: record.mode,
                    tests: record
                        .tests
                        .into_iter()
                        .map(|test| SubmissionTestSummary {
                            test_id: test.test_id,
                            verdict: test.verdict,
                            elapsed_ms: test.elapsed_ms,
                        })
                        .collect(),
                })
                .collect()
        })
        .map_err(|_| "Submission history could not be read.".to_owned())
}

#[tauri::command]
fn export_backup(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<BackupSnapshot, String> {
    export_backup_value(state.inner())
}

#[tauri::command]
fn restore_backup(
    snapshot_json: String,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<(), String> {
    restore_backup_json_value(&snapshot_json, state.inner())
}

#[tauri::command]
fn create_managed_environment(
    request: CreateEnvironmentRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ManagedEnvironmentSummary, String> {
    create_managed_environment_value(request, state.inner())
}

#[tauri::command]
fn clone_managed_environment(
    request: CloneEnvironmentRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ManagedEnvironmentSummary, String> {
    clone_managed_environment_value(request, state.inner())
}

#[tauri::command]
fn reset_managed_environment(
    request: ResetEnvironmentRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ManagedEnvironmentSummary, String> {
    reset_managed_environment_value(request, state.inner())
}

#[tauri::command]
fn delete_managed_environment(
    request: DeleteEnvironmentRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<(), String> {
    delete_managed_environment_value(request, state.inner())
}

#[tauri::command]
fn import_pack(
    request: ImportPackRequest,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<PackImportResult, String> {
    import_pack_value(request, state.inner())
}

#[tauri::command]
fn inspect_managed_pack(
    pack_id: String,
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Option<ManagedPackSummary>, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    match managed_pack_identity(&state, &pack_id) {
        Ok((version, digest)) => Ok(Some(ManagedPackSummary {
            pack_id,
            version,
            digest,
        })),
        Err(error) if error == "The requested managed pack is not installed." => Ok(None),
        Err(error) => Err(error),
    }
}

#[tauri::command]
fn list_installed_packs(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<Vec<problem_pack::PackCatalogEntry>, String> {
    list_installed_packs_value(state.inner())
}

#[tauri::command]
fn managed_runtime_status(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ManagedRuntimeStatus, String> {
    let state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    Ok(managed_runtime_status_value(state.runtime.is_some()))
}

#[tauri::command]
fn start_managed_runtime(
    state: State<'_, Mutex<ManagedEnvironmentState>>,
) -> Result<ManagedRuntimeStatus, String> {
    let mut state = state
        .lock()
        .map_err(|_| "Managed environment state is busy.".to_owned())?;
    ensure_managed_runtime(&mut state)
}

fn external_navigation_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("external-navigation")
        .on_navigation(|webview, url| {
            let is_internal_host = matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost") | Some("::1")
            );

            let is_internal = url.scheme() == "tauri" || is_internal_host;

            if is_internal {
                return true;
            }

            let is_external_link = matches!(url.scheme(), "http" | "https" | "mailto" | "tel");

            if is_external_link {
                log::info!(
                    "opening external link in system browser: {}://{}",
                    url.scheme(),
                    url.host_str().unwrap_or("redacted")
                );
                let _ = webview.opener().open_url(url.as_str(), None::<&str>);
                return false;
            }

            true
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(external_navigation_plugin())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(Box::<dyn std::error::Error>::from)?;
            std::fs::create_dir_all(&data_dir)?;
            let state = load_environment_state_for_startup(&data_dir.join("sql-judge.sqlite"))
                .map_err(std::io::Error::other)?;
            app.manage(Mutex::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            run_sample,
            submit_solution,
            cancel_judge,
            list_connection_profiles,
            save_connection_profile,
            delete_connection_profile,
            test_external_connection,
            list_external_metadata,
            execute_external_select,
            test_external_sqlserver,
            list_external_sqlserver_metadata,
            execute_external_sqlserver_select,
            list_managed_environments,
            inspect_managed_environment,
            create_managed_environment,
            clone_managed_environment,
            reset_managed_environment,
            delete_managed_environment,
            import_pack,
            inspect_managed_pack,
            list_installed_packs,
            list_submission_history,
            export_backup,
            restore_backup,
            managed_runtime_status,
            start_managed_runtime
        ])
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                log::info!("main webview finished loading");
                let _ = webview.window().show();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        CloneEnvironmentRequest, CreateEnvironmentRequest, DeleteEnvironmentRequest,
        DestructiveAction, ExternalCommandError, ExternalConnectionInfo, ImportPackRequest,
        JudgeRunRequest, MAX_EXTERNAL_SQL_BYTES, ManagedEnvironment, MemorySecretVault,
        PostgresSession, RecoveryOperation, RecoveryRecord, ResetEnvironmentRequest,
        SaveConnectionProfileRequest, SecretVault, cancel_judge_value,
        clone_managed_environment_value, create_managed_environment_value,
        delete_connection_profile_value, delete_managed_environment_value, drop_recovery_namespace,
        ensure_managed_runtime, export_backup_value, external_profile_config_value,
        external_sqlserver_profile_config_value, import_pack_value, judge_cases, judge_request,
        list_connection_profiles_value, list_installed_packs_value, load_environment_state,
        load_environment_state_for_startup, managed_environment_summary, managed_pack_identity,
        managed_runtime_status_value, provision_record, reconcile_environment_recovery,
        register_cancellation, require_postgres_read_only, require_sqlserver_read_only,
        reset_managed_environment_value, restore_backup_json_value, restore_backup_value,
        save_connection_profile_value, sql_value_json, validate_external_sql_request,
        validate_secret_ref, verdict_name, with_verified_postgres_explorer,
        with_verified_sqlserver_explorer,
    };
    use environment_manager::{EnvironmentMode, EnvironmentRecord, EnvironmentState};
    use judge_core::SqlValue;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;

    fn invalid_external_select_request() -> super::ExternalSelectRequest {
        super::ExternalSelectRequest {
            profile_id: "missing-profile".to_owned(),
            sql: "SELECT 1".to_owned(),
            statement_timeout_ms: 0,
            max_rows: 0,
            max_bytes: 0,
        }
    }

    fn install_pack(state: &Mutex<super::ManagedEnvironmentState>, path: &Path) {
        import_pack_value(
            ImportPackRequest {
                path: path.to_string_lossy().into_owned(),
            },
            state,
        )
        .expect("install sample pack");
    }

    fn install_sample_pack(state: &Mutex<super::ManagedEnvironmentState>) {
        install_pack(
            state,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/packs/hr-analytics"),
        );
    }

    #[test]
    fn external_select_rejects_invalid_limits_before_profile_lookup() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-external-limits-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(load_environment_state(&root.join("state.sqlite")).expect("state"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.lock().expect("lock state");
            panic!("poison state after acquiring the mutex");
        }));
        assert!(result.is_err());

        let error = super::execute_external_select_value(
            invalid_external_select_request(),
            &state,
            &MemorySecretVault::default(),
        )
        .expect_err("invalid limits must be rejected before profile access");
        assert_eq!(error.code, "limits_invalid");
        let error = super::execute_external_sqlserver_select_value(
            invalid_external_select_request(),
            &state,
            &MemorySecretVault::default(),
        )
        .expect_err("invalid limits must be rejected before profile access");
        assert_eq!(error.code, "limits_invalid");
        let _ = std::fs::remove_dir_all(root);
    }

    fn sample_pack_identity() -> (String, String) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/packs/hr-analytics");
        let pack = problem_pack::PackData::from_path(path)
            .expect("load sample pack")
            .validate()
            .expect("validate sample pack");
        (pack.version().to_owned(), pack.digest().to_owned())
    }

    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).expect("create pack destination");
        for entry in std::fs::read_dir(source).expect("read pack source") {
            let entry = entry.expect("read pack entry");
            let target = destination.join(entry.file_name());
            if entry.file_type().expect("pack entry type").is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).expect("copy pack file");
            }
        }
    }

    #[test]
    fn native_judge_cancellation_registry_is_scoped_to_request_id() {
        let registration = register_cancellation(Some("request-1"));
        assert!(cancel_judge_value("request-1"));
        assert!(registration.token.is_cancelled());
        drop(registration);
        assert!(!cancel_judge_value("request-1"));
    }

    #[test]
    fn native_recovery_reconciles_interrupted_rebuild_without_touching_old_namespace() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let created = create_managed_environment_value(
            CreateEnvironmentRequest {
                env_id: "env-a".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version,
                pack_digest,
                namespace: "sj_env_a".to_owned(),
            },
            &state,
        )
        .expect("create environment");
        {
            let mut state_guard = state.lock().expect("lock native state");
            let ownership_nonce = state_guard
                .ledger
                .inspect("env-a")
                .expect("created record")
                .ownership_nonce
                .clone();
            state_guard
                .ledger
                .begin_rebuild(
                    "env-a",
                    DestructiveAction::Reset {
                        env_id: "env-a".to_owned(),
                        instance_id: created.instance_id.clone(),
                        ownership_nonce: ownership_nonce.clone(),
                        confirmed: true,
                    },
                )
                .expect("begin rebuild");
            let rebuilding = state_guard
                .ledger
                .inspect("env-a")
                .expect("rebuilding record")
                .clone();
            state_guard
                .store
                .save_environment(&rebuilding)
                .expect("save rebuilding state");
            state_guard
                .store
                .save_recovery(&RecoveryRecord {
                    env_id: "env-a".to_owned(),
                    operation: RecoveryOperation::Rebuild,
                    instance_id: created.instance_id,
                    ownership_nonce,
                    namespace: "sj_env_a".to_owned(),
                    replacement_namespace: Some("sj_env_next".to_owned()),
                })
                .expect("save recovery intent");
        }
        drop(state);

        let mut recovered = load_environment_state_for_startup(&path).expect("reload native state");
        assert!(recovered.runtime.is_none());
        ensure_managed_runtime(&mut recovered).expect("reconcile recovery");
        assert_eq!(
            recovered.ledger.inspect("env-a").expect("recovered").state,
            environment_manager::EnvironmentState::Ready
        );
        assert!(
            recovered
                .store
                .list_recoveries()
                .expect("list recoveries")
                .is_empty()
        );
        drop(recovered);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_recovery_preserves_provisioned_managed_environment_records() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-provision-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let runtime_instance_id = state
            .lock()
            .expect("lock native state")
            .runtime_instance_id
            .clone();

        for env_id in ["env_ready", "env_running", "env_error"] {
            create_managed_environment_value(
                CreateEnvironmentRequest {
                    env_id: env_id.to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: pack_version.clone(),
                    pack_digest: pack_digest.clone(),
                    namespace: format!("sj_{env_id}"),
                },
                &state,
            )
            .expect("create environment");
        }
        {
            let mut guard = state.lock().expect("lock native state");
            guard
                .ledger
                .mark_running("env_running")
                .expect("mark environment running");
            guard
                .ledger
                .mark_error("env_error")
                .expect("mark environment error");
            for env_id in ["env_ready", "env_running", "env_error"] {
                let record = guard
                    .ledger
                    .inspect(env_id)
                    .expect("read environment")
                    .clone();
                guard.store.save_environment(&record).expect("save state");
                guard
                    .store
                    .save_recovery(&RecoveryRecord {
                        env_id: env_id.to_owned(),
                        operation: RecoveryOperation::Provision,
                        instance_id: record.instance_id,
                        ownership_nonce: record.ownership_nonce,
                        namespace: record.namespace,
                        replacement_namespace: None,
                    })
                    .expect("save provision recovery");
            }
        }
        drop(state);
        let runtime = judge_postgres::ManagedPostgresRuntime::start(
            root.join("postgres-runtime"),
            &runtime_instance_id,
        )
        .expect("leave owned runtime running for startup adoption");
        std::mem::forget(runtime);
        let recovered =
            load_environment_state_for_startup(&path).expect("reload and reconcile native state");
        assert_eq!(
            recovered
                .ledger
                .inspect("env_ready")
                .expect("ready record")
                .state,
            environment_manager::EnvironmentState::Ready
        );
        assert_eq!(
            recovered
                .ledger
                .inspect("env_running")
                .expect("running record")
                .state,
            environment_manager::EnvironmentState::Running
        );
        assert_eq!(
            recovered
                .ledger
                .inspect("env_error")
                .expect("error record")
                .state,
            environment_manager::EnvironmentState::Error
        );
        assert!(
            recovered
                .store
                .list_recoveries()
                .expect("list recoveries")
                .is_empty()
        );
        drop(recovered);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_recovery_rejects_mismatched_provision_without_removing_record() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-provision-recovery-mismatch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut state = load_environment_state(&path).expect("load native state");
        let record = environment_manager::EnvironmentRecord {
            env_id: "env_mismatched".to_owned(),
            instance_id: "instance_mismatched".to_owned(),
            pack_id: "hr-analytics".to_owned(),
            pack_version: "1.0.0".to_owned(),
            pack_digest: "a".repeat(64),
            namespace: "sj_env_mismatched".to_owned(),
            ownership_nonce: "nonce_mismatched".to_owned(),
            mode: environment_manager::EnvironmentMode::Managed,
            state: environment_manager::EnvironmentState::Provisioning,
        };
        state
            .ledger
            .create(record.clone())
            .expect("reserve environment");
        state
            .store
            .save_environment(&record)
            .expect("save environment");
        state
            .store
            .save_recovery(&RecoveryRecord {
                env_id: record.env_id.clone(),
                operation: RecoveryOperation::Provision,
                instance_id: "wrong-instance".to_owned(),
                ownership_nonce: record.ownership_nonce.clone(),
                namespace: record.namespace.clone(),
                replacement_namespace: None,
            })
            .expect("save mismatched recovery");

        assert!(ensure_managed_runtime(&mut state).is_err());
        assert!(state.ledger.inspect(&record.env_id).is_ok());
        assert!(
            state
                .store
                .get_environment(&record.env_id)
                .expect("read environment")
                .is_some()
        );
        assert_eq!(
            state
                .store
                .list_recoveries()
                .expect("list recoveries")
                .len(),
            1
        );
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_recovery_cleanup_rejects_existing_mismatched_namespace() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-recovery-cleanup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut state = load_environment_state(&path).expect("load native state");
        ensure_managed_runtime(&mut state).expect("start managed runtime");
        let connection = state.runtime.as_ref().expect("runtime").connection_string();
        let mut session = PostgresSession::connect(&connection).expect("connect runtime");
        let original = ManagedEnvironment::new(
            "env-recovery-cleanup",
            state.runtime_instance_id.clone(),
            "sj_recovery_cleanup",
            "nonce-recovery-cleanup",
        )
        .expect("valid environment");
        session
            .provision_managed_namespace(&original, "sqljudge_runner")
            .expect("provision namespace");

        let mismatched = RecoveryRecord {
            env_id: "env-recovery-cleanup".to_owned(),
            operation: RecoveryOperation::Delete,
            instance_id: state.runtime_instance_id.clone(),
            ownership_nonce: "wrong-nonce".to_owned(),
            namespace: "sj_recovery_cleanup".to_owned(),
            replacement_namespace: None,
        };
        assert!(drop_recovery_namespace(&mut session, &mismatched, &mismatched.namespace).is_err());
        assert!(
            session
                .managed_namespace_is_owned(&original)
                .expect("inspect preserved namespace")
        );

        let absent = RecoveryRecord {
            namespace: "sj_absent_recovery_cleanup".to_owned(),
            ..mismatched
        };
        assert!(drop_recovery_namespace(&mut session, &absent, &absent.namespace).is_ok());
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_provision_recovery_retains_state_for_existing_unowned_namespace() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-provision-unknown-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut state = load_environment_state(&path).expect("load native state");
        ensure_managed_runtime(&mut state).expect("start managed runtime");
        let namespace = "sj_unknown_provision_recovery";
        let socket = state.runtime_root.join("socket");
        let socket = socket.to_str().expect("runtime socket path");
        assert!(
            Command::new("psql")
                .args([
                    "-h",
                    socket,
                    "-d",
                    "postgres",
                    "-v",
                    "ON_ERROR_STOP=1",
                    "-c",
                    &format!("CREATE SCHEMA {namespace}"),
                ])
                .status()
                .expect("create unowned namespace")
                .success()
        );
        let record = EnvironmentRecord {
            env_id: "env-unknown-provision".to_owned(),
            instance_id: state.runtime_instance_id.clone(),
            pack_id: "hr-analytics".to_owned(),
            pack_version: "1.0.0".to_owned(),
            pack_digest: "a".repeat(64),
            namespace: namespace.to_owned(),
            ownership_nonce: "nonce-unknown-provision".to_owned(),
            mode: EnvironmentMode::Managed,
            state: EnvironmentState::Provisioning,
        };
        state
            .ledger
            .create(record.clone())
            .expect("create ledger record");
        state
            .store
            .save_environment(&record)
            .expect("save ledger record");
        state
            .store
            .save_recovery(&RecoveryRecord {
                env_id: record.env_id.clone(),
                operation: RecoveryOperation::Provision,
                instance_id: record.instance_id.clone(),
                ownership_nonce: record.ownership_nonce.clone(),
                namespace: record.namespace.clone(),
                replacement_namespace: None,
            })
            .expect("save recovery journal");

        assert!(reconcile_environment_recovery(&mut state).is_err());
        assert!(state.ledger.inspect(&record.env_id).is_ok());
        assert!(
            state
                .store
                .get_environment(&record.env_id)
                .expect("stored record")
                .is_some()
        );
        assert_eq!(state.store.list_recoveries().expect("recoveries").len(), 1);
        let _ = Command::new("psql")
            .args([
                "-h",
                socket,
                "-d",
                "postgres",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                &format!("DROP SCHEMA {namespace}"),
            ])
            .status();
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_sqlserver_direct_access_requires_verified_read_only() {
        let verified = judge_sqlserver::SqlServerConnectionInfo {
            database: "practice".to_owned(),
            username: "reader".to_owned(),
            server_version: "test".to_owned(),
            read_only: true,
        };
        let writer = judge_sqlserver::SqlServerConnectionInfo {
            read_only: false,
            ..verified.clone()
        };
        assert!(require_sqlserver_read_only(&verified).is_ok());
        assert_eq!(
            require_sqlserver_read_only(&writer)
                .expect_err("writer profile must be denied")
                .code,
            "read_only_unverified"
        );

        let postgres_verified = ExternalConnectionInfo {
            database: "practice".to_owned(),
            username: "reader".to_owned(),
            server_version: "test".to_owned(),
            read_only: true,
        };
        let postgres_writer = ExternalConnectionInfo {
            read_only: false,
            ..postgres_verified.clone()
        };
        assert!(require_postgres_read_only(&postgres_verified).is_ok());
        assert_eq!(
            require_postgres_read_only(&postgres_writer)
                .expect_err("writer PostgreSQL profile must be denied")
                .code,
            "read_only_unverified"
        );
    }

    #[test]
    fn native_sqlserver_direct_operations_stop_before_operation_when_unverified() {
        let writer = judge_sqlserver::SqlServerConnectionInfo {
            database: "practice".to_owned(),
            username: "writer".to_owned(),
            server_version: "test".to_owned(),
            read_only: false,
        };
        let mut called = false;
        let result = with_verified_sqlserver_explorer(
            || Ok::<(), ExternalCommandError>(()),
            |_| Ok(writer),
            |_| {
                called = true;
                Ok::<_, ExternalCommandError>(())
            },
        );
        assert_eq!(
            result.expect_err("writer operation must be denied").code,
            "read_only_unverified"
        );
        assert!(!called);

        let postgres_writer = ExternalConnectionInfo {
            database: "practice".to_owned(),
            username: "writer".to_owned(),
            server_version: "test".to_owned(),
            read_only: false,
        };
        let mut postgres_called = false;
        let result = with_verified_postgres_explorer(
            || Ok::<(), ExternalCommandError>(()),
            |_| Ok(postgres_writer),
            |_| {
                postgres_called = true;
                Ok::<_, ExternalCommandError>(())
            },
        );
        assert_eq!(
            result
                .expect_err("writer PostgreSQL operation must be denied")
                .code,
            "read_only_unverified"
        );
        assert!(!postgres_called);
    }

    #[test]
    fn native_sqlserver_direct_operations_run_after_verified_probe() {
        let verified = judge_sqlserver::SqlServerConnectionInfo {
            database: "practice".to_owned(),
            username: "reader".to_owned(),
            server_version: "test".to_owned(),
            read_only: true,
        };
        let mut called = false;
        let result = with_verified_sqlserver_explorer(
            || Ok::<_, ExternalCommandError>(()),
            |_| Ok(verified),
            |_| {
                called = true;
                Ok::<_, ExternalCommandError>("metadata-or-select")
            },
        )
        .expect("verified operation");
        assert_eq!(result, "metadata-or-select");
        assert!(called);

        let postgres_verified = ExternalConnectionInfo {
            database: "practice".to_owned(),
            username: "reader".to_owned(),
            server_version: "test".to_owned(),
            read_only: true,
        };
        let mut postgres_called = false;
        let result = with_verified_postgres_explorer(
            || Ok::<_, ExternalCommandError>(()),
            |_| Ok(postgres_verified),
            |_| {
                postgres_called = true;
                Ok::<_, ExternalCommandError>("metadata-or-select")
            },
        )
        .expect("verified PostgreSQL operation");
        assert_eq!(result, "metadata-or-select");
        assert!(postgres_called);
    }

    #[test]
    fn native_external_sql_requests_are_bounded_before_connection() {
        assert!(validate_external_sql_request(&"x".repeat(MAX_EXTERNAL_SQL_BYTES)).is_ok());
        let oversized = "x".repeat(MAX_EXTERNAL_SQL_BYTES + 1);
        let error = validate_external_sql_request(&oversized).expect_err("oversized SQL");
        assert_eq!(error.code, "sql_too_large");
        assert!(!error.message.contains(&oversized));
    }

    #[test]
    fn native_provision_failure_persists_error_and_recovery_journal() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-provision-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut state = load_environment_state(&path).expect("load native state");
        let record = environment_manager::EnvironmentRecord {
            env_id: "env_provision_error".to_owned(),
            instance_id: "instance_provision_error".to_owned(),
            pack_id: "hr-analytics".to_owned(),
            pack_version: "1.0.0".to_owned(),
            pack_digest: "a".repeat(64),
            namespace: "sj_env_provision_error".to_owned(),
            ownership_nonce: "nonce_provision_error".to_owned(),
            mode: environment_manager::EnvironmentMode::Managed,
            state: environment_manager::EnvironmentState::Provisioning,
        };

        let error = provision_record(&mut state, record.clone()).expect_err("provision fails");

        assert_eq!(error, "Managed PostgreSQL runtime is unavailable.");
        assert_eq!(
            state
                .ledger
                .inspect(&record.env_id)
                .expect("ledger record")
                .state,
            environment_manager::EnvironmentState::Error
        );
        assert_eq!(
            state
                .store
                .get_environment(&record.env_id)
                .expect("stored record")
                .expect("stored environment")
                .state,
            environment_manager::EnvironmentState::Error
        );
        assert_eq!(state.store.list_recoveries().expect("recoveries").len(), 1);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_bridge_rejects_malformed_request_as_policy_error() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-malformed-judge-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create malformed request test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load test state"),
        );
        let result = judge_request(
            JudgeRunRequest {
                problem_id: "above-average".to_owned(),
                pack_id: String::new(),
                pack_version: String::new(),
                pack_digest: String::new(),
                sql: "SELECT 1".to_owned(),
                mode: "run".to_owned(),
                request_id: None,
            },
            &state,
            "run",
        );

        assert_eq!(result.status, "error");
        assert_eq!(result.verdict.as_deref(), Some("PolicyViolation"));
        assert!(result.message.contains("malformed"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_runtime_status_is_unavailable_before_explicit_start() {
        let status = managed_runtime_status_value(false);
        assert!(!status.available);
        assert_eq!(status.state, "unavailable");
    }

    #[test]
    fn clean_startup_does_not_start_managed_runtime() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-clean-startup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");

        let state = load_environment_state_for_startup(&root.join("state.sqlite"))
            .expect("load clean startup state");

        assert!(state.runtime.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn startup_with_stopped_runtime_and_pending_recovery_registers_state() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-startup-pending-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut initial = load_environment_state(&path).expect("load initial state");
        let recovery = RecoveryRecord {
            env_id: "env-pending-startup".to_owned(),
            operation: RecoveryOperation::Provision,
            instance_id: initial.runtime_instance_id.clone(),
            ownership_nonce: "nonce-pending-startup".to_owned(),
            namespace: "sj_pending_startup".to_owned(),
            replacement_namespace: None,
        };
        initial
            .store
            .save_recovery(&recovery)
            .expect("save pending recovery");
        drop(initial);
        std::fs::create_dir(root.join("postgres-runtime"))
            .expect("create unavailable runtime root");

        let state = load_environment_state_for_startup(&path)
            .expect("startup must register state while runtime is stopped");
        assert!(state.runtime.is_none());
        assert_eq!(state.store.list_recoveries().expect("recoveries").len(), 1);

        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn startup_adopts_running_owned_runtime_without_recovery_journal() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-startup-adopt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let initial = load_environment_state(&path).expect("load initial state");
        let instance_id = initial.runtime_instance_id.clone();
        drop(initial);

        let runtime = judge_postgres::ManagedPostgresRuntime::start(
            root.join("postgres-runtime"),
            &instance_id,
        )
        .expect("start owned runtime");
        std::mem::forget(runtime);

        let state = load_environment_state_for_startup(&path).expect("adopt running runtime");
        assert!(state.runtime.is_some());
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_secret_vault_roundtrips_without_exposing_secret_in_errors() {
        let mut vault = MemorySecretVault::default();
        vault
            .set_secret("vault://sql-judge/profile-a", "super-secret")
            .expect("store session secret");
        assert_eq!(
            vault
                .get_secret("vault://sql-judge/profile-a")
                .expect("read session secret")
                .as_deref(),
            Some("super-secret")
        );
        vault
            .delete_secret("vault://sql-judge/profile-a")
            .expect("delete session secret");
        assert!(
            vault
                .get_secret("vault://sql-judge/profile-a")
                .expect("read deleted session secret")
                .is_none()
        );
    }

    #[test]
    fn secret_reference_validation_rejects_credentials_and_path_traversal() {
        assert!(validate_secret_ref("vault://sql-judge/profile-a").is_ok());
        assert!(validate_secret_ref("super-secret").is_err());
        assert!(validate_secret_ref("vault://sql-judge/../profile-a").is_err());
        assert!(!format!("{:?}", validate_secret_ref("super-secret")).contains("super-secret"));
    }

    #[test]
    fn connection_profile_commands_persist_metadata_and_keep_secret_out_of_results() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-connection-command-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load native state"),
        );
        let mut vault = MemorySecretVault::default();
        let summary = save_connection_profile_value(
            SaveConnectionProfileRequest {
                profile_id: "profile-a".to_owned(),
                display_name: "Training database".to_owned(),
                engine: "postgresql".to_owned(),
                host: "db.example.test".to_owned(),
                port: 5432,
                database: "training".to_owned(),
                username: "explorer".to_owned(),
                tls_mode: "verify_full".to_owned(),
                secret: Some("super-secret".to_owned()),
            },
            &state,
            &mut vault,
        )
        .expect("save profile");
        assert!(summary.has_secret);
        assert!(!format!("{summary:?}").contains("super-secret"));
        let updated = save_connection_profile_value(
            SaveConnectionProfileRequest {
                profile_id: "profile-a".to_owned(),
                display_name: "Updated training database".to_owned(),
                engine: "postgresql".to_owned(),
                host: "db.example.test".to_owned(),
                port: 5432,
                database: "training".to_owned(),
                username: "explorer".to_owned(),
                tls_mode: "verify_full".to_owned(),
                secret: None,
            },
            &state,
            &mut vault,
        )
        .expect("update profile metadata");
        assert!(updated.has_secret);
        assert_eq!(
            list_connection_profiles_value(&state)
                .expect("list profiles")
                .len(),
            1
        );
        assert_eq!(
            vault
                .get_secret("vault://sql-judge/profile-a")
                .expect("read vault")
                .as_deref(),
            Some("super-secret")
        );
        delete_connection_profile_value("profile-a", &state, &mut vault).expect("delete profile");
        assert!(
            vault
                .get_secret("vault://sql-judge/profile-a")
                .expect("read deleted vault")
                .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_profile_config_keeps_secret_native_and_rejects_sql_server_routing() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-external-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load native state"),
        );
        let mut vault = MemorySecretVault::default();
        save_connection_profile_value(
            SaveConnectionProfileRequest {
                profile_id: "profile-a".to_owned(),
                display_name: "External PostgreSQL".to_owned(),
                engine: "postgresql".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: 5432,
                database: "practice".to_owned(),
                username: "reader".to_owned(),
                tls_mode: "verify_full".to_owned(),
                secret: Some("secret-canary".to_owned()),
            },
            &state,
            &mut vault,
        )
        .expect("save external profile");
        let config = external_profile_config_value("profile-a", &state, &vault)
            .expect("build external config");
        assert_eq!(config.password, "secret-canary");

        save_connection_profile_value(
            SaveConnectionProfileRequest {
                profile_id: "profile-sqlserver".to_owned(),
                display_name: "External SQL Server".to_owned(),
                engine: "sqlserver".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: 1433,
                database: "practice".to_owned(),
                username: "reader".to_owned(),
                tls_mode: "verify_full".to_owned(),
                secret: Some("sqlserver-secret".to_owned()),
            },
            &state,
            &mut vault,
        )
        .expect("save SQL Server profile");
        let sqlserver_config =
            external_sqlserver_profile_config_value("profile-sqlserver", &state, &vault)
                .expect("build SQL Server config");
        assert_eq!(sqlserver_config.password, "sqlserver-secret");
        let error = match external_profile_config_value("profile-sqlserver", &state, &vault) {
            Ok(_) => panic!("SQL Server must not route to PostgreSQL"),
            Err(error) => error,
        };
        assert_eq!(error.code, "profile_invalid");
        assert!(!format!("{error:?}").contains("secret-canary"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_verdict_names_are_stable() {
        assert_eq!(verdict_name(judge_core::Verdict::Accepted), "Accepted");
        assert_eq!(
            verdict_name(judge_core::Verdict::WrongAnswer),
            "WrongAnswer"
        );
    }

    #[test]
    fn native_judge_cases_follow_a_non_default_problem_manifest() {
        let problem = problem_pack::ValidatedProblem {
            id: "department-summary".to_owned(),
            dialect: judge_core::Dialect::Postgresql,
            schema_sql: "CREATE TABLE departments (id integer);".to_owned(),
            reference_sql: "SELECT id FROM departments".to_owned(),
            output: judge_core::OutputPolicy::new(
                judge_core::RowOrder::Ordered,
                true,
                true,
                vec![judge_core::Column::new(
                    "id",
                    judge_core::LogicalType::Integer,
                )],
            ),
            limits: judge_core::QueryLimits::new(2_000, 10, 64 * 1024),
            tests: vec![
                problem_pack::ValidatedTest {
                    id: "visible-example".to_owned(),
                    hidden: false,
                    fixture_sql: "INSERT INTO departments VALUES (1);".to_owned(),
                },
                problem_pack::ValidatedTest {
                    id: "hidden-boundary".to_owned(),
                    hidden: true,
                    fixture_sql: "INSERT INTO departments VALUES (2);".to_owned(),
                },
            ],
        };

        assert_eq!(
            judge_cases(&problem, false)
                .iter()
                .map(|case| case.id)
                .collect::<Vec<_>>(),
            vec!["visible-example"]
        );
        assert_eq!(
            judge_cases(&problem, true)
                .iter()
                .map(|case| case.id)
                .collect::<Vec<_>>(),
            vec!["visible-example", "hidden-boundary"]
        );
    }

    #[test]
    fn native_run_executes_sample_against_managed_runtime() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-run-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut native_state = load_environment_state(&path).expect("load native state");
        ensure_managed_runtime(&mut native_state).expect("start managed runtime");
        let state = Mutex::new(native_state);
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let result = judge_request(
            JudgeRunRequest {
                problem_id: "above-average".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: pack_version.clone(),
                pack_digest: pack_digest.clone(),
                sql: "SELECT e.id AS employee_id, e.name AS employee_name FROM employees AS e WHERE e.salary > (SELECT AVG(peer.salary) FROM employees AS peer WHERE peer.department_id = e.department_id)".to_owned(),
                mode: "run".to_owned(),
                request_id: None,
            },
            &state,
            "run",
        );

        assert_eq!(result.status, "completed");
        assert_eq!(result.verdict.as_deref(), Some("Accepted"));
        assert_eq!(
            result
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            vec!["employee_id", "employee_name"]
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            serde_json::to_value(&result.tests).expect("encode run tests"),
            serde_json::json!([{
                "id": "basic",
                "verdict": "Accepted",
                "elapsedMs": result.tests[0].elapsed_ms,
                "hidden": false
            }])
        );
        let submit = judge_request(
            JudgeRunRequest {
                problem_id: "above-average".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version,
                pack_digest,
                sql: "SELECT 1".to_owned(),
                mode: "submit".to_owned(),
                request_id: None,
            },
            &state,
            "submit",
        );
        assert_eq!(submit.status, "completed");
        assert_eq!(submit.verdict.as_deref(), Some("WrongAnswer"));
        assert_eq!(
            submit
                .tests
                .iter()
                .map(|test| (test.id.as_str(), test.verdict.as_str(), test.hidden))
                .collect::<Vec<_>>(),
            vec![
                ("basic", "WrongAnswer", false),
                ("ties-at-average", "WrongAnswer", true),
                ("null-salaries", "WrongAnswer", true),
                ("empty-department", "WrongAnswer", true),
            ]
        );
        let encoded = serde_json::to_value(&submit).expect("encode submit result");
        for test in encoded["tests"].as_array().expect("tests array") {
            let mut keys = test
                .as_object()
                .expect("test object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            keys.sort_unstable();
            assert_eq!(keys, vec!["elapsedMs", "hidden", "id", "verdict"]);
        }
        let history_ipc = serde_json::to_value(
            super::list_submission_history_value(&state).expect("history IPC"),
        )
        .expect("encode history");
        assert_eq!(history_ipc[0]["problemId"], "above-average");
        let state_guard = state.lock().expect("lock history state");
        let history = state_guard
            .history
            .list_submissions()
            .expect("list submission history");
        assert_eq!(history.len(), 2);
        let run = history
            .iter()
            .find(|record| record.mode == "run")
            .expect("sample run is recorded");
        let submitted = history
            .iter()
            .find(|record| record.mode == "submit")
            .expect("submission is recorded");
        for record in [run, submitted] {
            assert_eq!(record.pack_id, "hr-analytics");
            assert!(!record.pack_hash.is_empty());
            assert_eq!(record.problem_id.as_deref(), Some("above-average"));
        }
        assert_eq!(run.verdict, "Accepted");
        assert_eq!(submitted.verdict, "WrongAnswer");
        assert!(!run.tests.is_empty());
        assert!(submitted.tests.len() > run.tests.len());
        drop(state_guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_submit_reports_per_fixture_verdicts_for_design_starter() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-starter-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let mut native_state =
            load_environment_state(&root.join("state.sqlite")).expect("load native state");
        ensure_managed_runtime(&mut native_state).expect("start managed runtime");
        let state = Mutex::new(native_state);
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let catalog = list_installed_packs_value(&state).expect("list catalog");
        let problem = catalog[0]
            .problems
            .iter()
            .find(|problem| problem.id == "second-highest-salary")
            .expect("second highest salary problem");
        let starter = problem.starter_sql.clone().expect("starter SQL");
        assert!(starter.contains("DENSE_RANK()") && starter.lines().count() == 14);
        let request = |problem_id: &str, sql: &str, mode: &str| JudgeRunRequest {
            problem_id: problem_id.to_owned(),
            pack_id: "hr-analytics".to_owned(),
            pack_version: pack_version.clone(),
            pack_digest: pack_digest.clone(),
            sql: sql.to_owned(),
            mode: mode.to_owned(),
            request_id: None,
        };

        let run = judge_request(
            request("second-highest-salary", &starter, "run"),
            &state,
            "run",
        );
        assert_eq!(run.status, "completed");
        assert_eq!(
            run.tests
                .iter()
                .map(|test| (test.id.as_str(), test.hidden))
                .collect::<Vec<_>>(),
            vec![("basic", false)]
        );

        let submit = judge_request(
            request("second-highest-salary", &starter, "submit"),
            &state,
            "submit",
        );
        assert_eq!(submit.verdict.as_deref(), Some("WrongAnswer"));
        assert_eq!(
            submit
                .tests
                .iter()
                .map(|test| (test.id.as_str(), test.verdict.as_str(), test.hidden))
                .collect::<Vec<_>>(),
            vec![
                ("basic", "WrongAnswer", false),
                ("salary-ties", "WrongAnswer", true),
                ("null-salaries", "WrongAnswer", true),
                ("empty-department", "Accepted", true),
            ]
        );

        let forged = judge_request(
            request("not-in-this-pack", "SELECT 1", "submit"),
            &state,
            "submit",
        );
        assert_eq!(forged.verdict.as_deref(), Some("PolicyViolation"));
        assert!(forged.tests.is_empty());
        let history = super::list_submission_history_value(&state).expect("history");
        assert_eq!(history.len(), 2, "forged problem ids are never recorded");
        assert!(
            history
                .iter()
                .all(|record| record.problem_id.as_deref() == Some("second-highest-salary"))
        );
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_imports_zip_pack_after_runtime_validation() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut native_state = load_environment_state(&path).expect("load native state");
        ensure_managed_runtime(&mut native_state).expect("start managed runtime");
        let state = Mutex::new(native_state);
        let result = import_pack_value(
            ImportPackRequest {
                path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../examples/hr-analytics.sqljudge.zip")
                    .to_string_lossy()
                    .into_owned(),
            },
            &state,
        )
        .expect("import zip pack");

        assert_eq!(result.status, "published");
        assert_eq!(result.pack_id, "hr-analytics");
        assert_eq!(result.version, "1.2.0");
        assert!(!result.digest.is_empty());
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_rejects_a_managed_environment_without_an_imported_pack() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-unimported-pack-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "unimported-env".to_owned(),
                    instance_id: "unimported-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_digest: "a".repeat(64),
                    namespace: "sj_unimported".to_owned(),
                    ownership_nonce: "unimported-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save environment");
        }
        assert_eq!(
            export_backup_value(&state),
            Err("Metadata backup references a pack that is not imported.".to_owned())
        );
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_roundtrip_excludes_secrets() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let sample_identity = {
            let guard = state.lock().expect("lock native state");
            managed_pack_identity(&guard, "hr-analytics").expect("sample pack identity")
        };
        let mut vault = MemorySecretVault::default();
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "backup-env".to_owned(),
                    instance_id: "backup-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: sample_identity.0.clone(),
                    pack_digest: sample_identity.1.clone(),
                    namespace: "sj_backup".to_owned(),
                    ownership_nonce: "backup-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save environment");
            guard
                .history
                .save_submission(
                    &app_storage::SubmissionRecord {
                        submission_id: "backup-submission".to_owned(),
                        pack_id: "hr-analytics".to_owned(),
                        pack_version: "1.0.0".to_owned(),
                        pack_hash: "sha256:backup".to_owned(),
                        engine_version: "postgresql-managed".to_owned(),
                        policy_version: "policy-v1".to_owned(),
                        verdict: "Accepted".to_owned(),
                        created_at: "2026-10-12T00:00:00Z".to_owned(),
                        problem_id: Some("above-average".to_owned()),
                        mode: "submit".to_owned(),
                    },
                    &[app_storage::TestRecord {
                        test_id: "basic".to_owned(),
                        verdict: "Accepted".to_owned(),
                        elapsed_ms: 4,
                    }],
                )
                .expect("save submission");
        }
        save_connection_profile_value(
            SaveConnectionProfileRequest {
                profile_id: "backup-canary".to_owned(),
                display_name: "Backup canary profile".to_owned(),
                engine: "postgresql".to_owned(),
                host: "db.example.test".to_owned(),
                port: 5432,
                database: "training".to_owned(),
                username: "explorer".to_owned(),
                tls_mode: "verify_full".to_owned(),
                secret: Some("backup-secret-canary".to_owned()),
            },
            &state,
            &mut vault,
        )
        .expect("save backup canary profile");
        assert_eq!(
            vault
                .get_secret("vault://sql-judge/backup-canary")
                .expect("read backup canary")
                .as_deref(),
            Some("backup-secret-canary")
        );

        let snapshot = export_backup_value(&state).expect("export backup");
        let encoded = serde_json::to_string(&snapshot).expect("encode backup");
        assert!(!encoded.contains("backup-secret-canary"));
        assert!(!encoded.contains("password"));
        assert!(!encoded.contains("secret_ref"));
        assert!(!encoded.contains("vault://sql-judge/"));
        assert!(!encoded.contains("SELECT"));
        restore_backup_value(snapshot, &state).expect("restore backup");
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_malformed_backup_preserves_metadata_and_restart_state() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-invalid-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let sample_identity = {
            let guard = state.lock().expect("lock native state");
            managed_pack_identity(&guard, "hr-analytics").expect("sample pack identity")
        };
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "existing-env".to_owned(),
                    instance_id: "existing-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: sample_identity.0.clone(),
                    pack_digest: sample_identity.1.clone(),
                    namespace: "sj_existing".to_owned(),
                    ownership_nonce: "existing-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save environment");
            guard
                .history
                .save_submission(
                    &app_storage::SubmissionRecord {
                        submission_id: "existing-submission".to_owned(),
                        pack_id: "hr-analytics".to_owned(),
                        pack_version: "1.0.0".to_owned(),
                        pack_hash: "sha256:existing".to_owned(),
                        engine_version: "postgresql-managed".to_owned(),
                        policy_version: "policy-v1".to_owned(),
                        verdict: "Accepted".to_owned(),
                        created_at: "2026-10-12T00:00:00Z".to_owned(),
                        problem_id: Some("above-average".to_owned()),
                        mode: "submit".to_owned(),
                    },
                    &[app_storage::TestRecord {
                        test_id: "basic".to_owned(),
                        verdict: "Accepted".to_owned(),
                        elapsed_ms: 4,
                    }],
                )
                .expect("save submission");
        }

        let mut snapshot = export_backup_value(&state).expect("export backup");
        snapshot.environments[0].namespace = "public".to_owned();
        assert!(restore_backup_value(snapshot, &state).is_err());
        {
            let guard = state.lock().expect("relock state");
            assert_eq!(
                guard
                    .store
                    .get_environment("existing-env")
                    .expect("read existing environment")
                    .expect("existing environment remains")
                    .namespace,
                "sj_existing"
            );
            assert_eq!(
                guard
                    .history
                    .list_submissions()
                    .expect("read existing history")
                    .len(),
                1
            );
        }
        drop(state);

        let restarted = load_environment_state(&path).expect("restart from preserved state");
        assert_eq!(restarted.ledger.list().len(), 1);
        assert_eq!(
            restarted
                .store
                .get_environment("existing-env")
                .expect("read restarted environment")
                .expect("restarted environment remains")
                .namespace,
            "sj_existing"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_rejects_omitted_managed_environment_without_replacement() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-omitted-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let sample_identity = {
            let guard = state.lock().expect("lock native state");
            managed_pack_identity(&guard, "hr-analytics").expect("sample pack identity")
        };
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "existing-env".to_owned(),
                    instance_id: "existing-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: sample_identity.0.clone(),
                    pack_digest: sample_identity.1.clone(),
                    namespace: "sj_existing".to_owned(),
                    ownership_nonce: "existing-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save existing environment");
        }
        let snapshot = export_backup_value(&state).expect("export backup");
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "new-env".to_owned(),
                    instance_id: "new-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_digest: "a".repeat(64),
                    namespace: "sj_new".to_owned(),
                    ownership_nonce: "new-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save new environment");
        }
        assert!(restore_backup_value(snapshot, &state).is_err());
        let guard = state.lock().expect("relock state");
        assert!(
            guard
                .store
                .get_environment("existing-env")
                .expect("read existing environment")
                .is_some()
        );
        assert!(
            guard
                .store
                .get_environment("new-env")
                .expect("read new environment")
                .is_some()
        );
        drop(guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_rejects_changed_managed_identity_without_replacement() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        install_sample_pack(&state);
        let sample_identity = {
            let guard = state.lock().expect("lock native state");
            managed_pack_identity(&guard, "hr-analytics").expect("sample pack identity")
        };
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "existing-env".to_owned(),
                    instance_id: "existing-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: sample_identity.0.clone(),
                    pack_digest: sample_identity.1.clone(),
                    namespace: "sj_existing".to_owned(),
                    ownership_nonce: "existing-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save existing environment");
        }
        let mut snapshot = export_backup_value(&state).expect("export backup");
        snapshot.environments[0].pack_id = "different-pack".to_owned();
        assert!(restore_backup_value(snapshot, &state).is_err());
        let guard = state.lock().expect("relock state");
        assert_eq!(
            guard
                .store
                .get_environment("existing-env")
                .expect("read existing environment")
                .expect("existing environment remains")
                .ownership_nonce,
            "existing-nonce"
        );
        drop(guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_rejects_pending_recovery_without_erasing_journal() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "existing-env".to_owned(),
                    instance_id: "existing-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_digest: "a".repeat(64),
                    namespace: "sj_existing".to_owned(),
                    ownership_nonce: "existing-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save existing environment");
            guard
                .store
                .save_recovery(&app_storage::RecoveryRecord {
                    env_id: "existing-env".to_owned(),
                    operation: app_storage::RecoveryOperation::Rebuild,
                    instance_id: "existing-instance".to_owned(),
                    ownership_nonce: "existing-nonce".to_owned(),
                    namespace: "sj_existing".to_owned(),
                    replacement_namespace: Some("sj_replacement".to_owned()),
                })
                .expect("save recovery journal");
        }
        assert!(export_backup_value(&state).is_err());
        let snapshot = app_storage::BackupSnapshot {
            version: 1,
            environments: Vec::new(),
            submissions: Vec::new(),
        };
        assert!(restore_backup_value(snapshot, &state).is_err());
        let guard = state.lock().expect("relock state");
        assert_eq!(
            guard
                .store
                .list_recoveries()
                .expect("list recoveries")
                .len(),
            1
        );
        assert!(
            guard
                .store
                .get_environment("existing-env")
                .expect("read existing environment")
                .is_some()
        );
        drop(guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_export_rejects_oversized_metadata_before_returning_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-export-size-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let mut history = app_storage::HistoryStore::open(&path).expect("open history");
        history
            .save_submission(
                &app_storage::SubmissionRecord {
                    submission_id: "oversized-submission".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_hash: "x".repeat(4 * 1024 * 1024),
                    engine_version: "postgresql-managed".to_owned(),
                    policy_version: "policy-v1".to_owned(),
                    verdict: "Accepted".to_owned(),
                    created_at: "2026-10-12T00:00:00Z".to_owned(),
                    problem_id: Some("above-average".to_owned()),
                    mode: "submit".to_owned(),
                },
                &[],
            )
            .expect("save oversized submission");
        drop(history);
        assert!(app_storage::export_backup(&path).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn submission_history_lists_problem_mode_and_tests_in_camel_case() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-history-shape-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load native state"),
        );
        state
            .lock()
            .expect("lock history state")
            .history
            .save_submission(
                &app_storage::SubmissionRecord {
                    submission_id: "run-1".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_hash: "a".repeat(64),
                    engine_version: "postgresql-managed".to_owned(),
                    policy_version: "policy-v1".to_owned(),
                    verdict: "WrongAnswer".to_owned(),
                    created_at: "1760000000".to_owned(),
                    problem_id: Some("above-average".to_owned()),
                    mode: "run".to_owned(),
                },
                &[app_storage::TestRecord {
                    test_id: "visible-example".to_owned(),
                    verdict: "WrongAnswer".to_owned(),
                    elapsed_ms: 6,
                }],
            )
            .expect("save sample run");

        let history = super::list_submission_history_value(&state).expect("list history");

        assert_eq!(
            serde_json::to_value(&history).expect("encode history"),
            serde_json::json!([{
                "submissionId": "run-1",
                "packId": "hr-analytics",
                "packVersion": "1.0.0",
                "packHash": "a".repeat(64),
                "engineVersion": "postgresql-managed",
                "policyVersion": "policy-v1",
                "verdict": "WrongAnswer",
                "createdAt": "1760000000",
                "problemId": "above-average",
                "mode": "run",
                "tests": [{
                    "testId": "visible-example",
                    "verdict": "WrongAnswer",
                    "elapsedMs": 6
                }]
            }])
        );
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_oversized_restore_payload_is_rejected_before_deserialization() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-backup-oversized-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let path = root.join("state.sqlite");
        let state = Mutex::new(load_environment_state(&path).expect("load native state"));
        {
            let mut guard = state.lock().expect("lock state");
            guard
                .store
                .save_environment(&environment_manager::EnvironmentRecord {
                    env_id: "oversized-existing".to_owned(),
                    instance_id: "oversized-instance".to_owned(),
                    pack_id: "hr-analytics".to_owned(),
                    pack_version: "1.0.0".to_owned(),
                    pack_digest: "a".repeat(64),
                    namespace: "sj_oversized_existing".to_owned(),
                    ownership_nonce: "oversized-nonce".to_owned(),
                    mode: environment_manager::EnvironmentMode::Managed,
                    state: environment_manager::EnvironmentState::Ready,
                })
                .expect("save environment");
        }

        let payload = format!("{{\"padding\":\"{}", "x".repeat(4 * 1024 * 1024));
        assert_eq!(
            restore_backup_json_value(&payload, &state),
            Err("Metadata backup exceeds the supported size limit.".to_owned())
        );
        let guard = state.lock().expect("relock state");
        assert_eq!(
            guard
                .store
                .get_environment("oversized-existing")
                .expect("read existing environment")
                .expect("existing environment remains")
                .namespace,
            "sj_oversized_existing"
        );
        drop(guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_create_and_clone_provision_isolated_ready_environments() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-environments-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load native state"),
        );
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let first = create_managed_environment_value(
            CreateEnvironmentRequest {
                env_id: "env-a".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version,
                pack_digest,
                namespace: "sj_env_a".to_owned(),
            },
            &state,
        )
        .expect("create first environment");
        let second = clone_managed_environment_value(
            CloneEnvironmentRequest {
                source_env_id: "env-a".to_owned(),
                env_id: "env-b".to_owned(),
                namespace: "sj_env_b".to_owned(),
            },
            &state,
        )
        .expect("clone second environment");

        assert_eq!(first.state, "ready");
        assert_eq!(second.state, "ready");
        assert_ne!(first.namespace, second.namespace);
        let reset = reset_managed_environment_value(
            ResetEnvironmentRequest {
                env_id: first.env_id.clone(),
                instance_id: first.instance_id.clone(),
                namespace: "sj_env_reset".to_owned(),
                confirmed: true,
            },
            &state,
        )
        .expect("reset first environment");
        assert_eq!(reset.state, "ready");
        delete_managed_environment_value(
            DeleteEnvironmentRequest {
                env_id: second.env_id.clone(),
                instance_id: second.instance_id.clone(),
                confirmed: true,
            },
            &state,
        )
        .expect("delete second environment");
        let state_guard = state.lock().expect("lock native state");
        assert_eq!(state_guard.ledger.list().len(), 1);
        assert_eq!(
            state_guard
                .ledger
                .inspect(&first.env_id)
                .expect("reset environment")
                .namespace,
            "sj_env_reset"
        );
        drop(state_guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_create_provisions_an_imported_non_default_pack() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-non-default-pack-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let pack = root.join("non-default-pack");
        copy_tree(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/packs/hr-analytics"),
            &pack,
        );
        let manifest_path = pack.join("pack.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["id"] = serde_json::Value::String("analytics-sql-101".to_owned());
        std::fs::write(
            manifest_path,
            serde_json::to_vec(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");

        let state =
            Mutex::new(load_environment_state(&root.join("state.sqlite")).expect("load state"));
        install_pack(&state, &pack);
        let catalog = list_installed_packs_value(&state).expect("list installed catalog");
        assert!(
            catalog
                .iter()
                .any(|entry| entry.pack_id == "analytics-sql-101")
        );
        let (pack_version, pack_digest) = {
            let guard = state.lock().expect("lock state");
            managed_pack_identity(&guard, "analytics-sql-101").expect("pack identity")
        };
        let summary = create_managed_environment_value(
            CreateEnvironmentRequest {
                env_id: "env-analytics".to_owned(),
                pack_id: "analytics-sql-101".to_owned(),
                pack_version,
                pack_digest,
                namespace: "sj_env_analytics".to_owned(),
            },
            &state,
        )
        .expect("create imported non-default environment");
        assert_eq!(summary.pack_id, "analytics-sql-101");
        assert_eq!(summary.state, "ready");
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_reset_failure_persists_error_without_replacing_old_environment() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-reset-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load native state"),
        );
        install_sample_pack(&state);
        let (pack_version, pack_digest) = sample_pack_identity();
        let created = create_managed_environment_value(
            CreateEnvironmentRequest {
                env_id: "env-a".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version,
                pack_digest,
                namespace: "sj_env_a".to_owned(),
            },
            &state,
        )
        .expect("create environment");
        let result = reset_managed_environment_value(
            ResetEnvironmentRequest {
                env_id: created.env_id.clone(),
                instance_id: created.instance_id,
                namespace: "public".to_owned(),
                confirmed: true,
            },
            &state,
        );

        assert!(result.is_err());
        let state_guard = state.lock().expect("lock native state");
        assert_eq!(
            state_guard
                .ledger
                .inspect("env-a")
                .expect("environment")
                .state,
            environment_manager::EnvironmentState::Error
        );
        assert_eq!(
            state_guard
                .ledger
                .inspect("env-a")
                .expect("environment")
                .namespace,
            "sj_env_a"
        );
        drop(state_guard);
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_bridge_rejects_oversized_sql_before_runtime_access() {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-tauri-oversized-judge-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create oversized request test root");
        let state = Mutex::new(
            load_environment_state(&root.join("state.sqlite")).expect("load test state"),
        );
        let result = judge_request(
            JudgeRunRequest {
                problem_id: "above-average".to_owned(),
                pack_id: String::new(),
                pack_version: String::new(),
                pack_digest: String::new(),
                sql: "x".repeat(65_537),
                mode: "submit".to_owned(),
                request_id: None,
            },
            &state,
            "submit",
        );

        assert_eq!(result.status, "error");
        assert_eq!(result.verdict.as_deref(), Some("PolicyViolation"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn judge_result_json_preserves_boolean_and_integer_precision() {
        let values = [
            SqlValue::Bool(true),
            SqlValue::Signed(9_007_199_254_740_991),
            SqlValue::Signed(9_007_199_254_740_992),
            SqlValue::Signed(-9_007_199_254_740_991),
            SqlValue::Signed(-9_007_199_254_740_992),
            SqlValue::Unsigned(9_007_199_254_740_991),
            SqlValue::Unsigned(9_007_199_254_740_992),
        ];

        let json_values = values.iter().map(sql_value_json).collect::<Vec<_>>();

        assert_eq!(
            json_values,
            vec![
                serde_json::json!(true),
                serde_json::json!(9_007_199_254_740_991_i64),
                serde_json::Value::String("9007199254740992".to_owned()),
                serde_json::json!(-9_007_199_254_740_991_i64),
                serde_json::Value::String("-9007199254740992".to_owned()),
                serde_json::json!(9_007_199_254_740_991_u64),
                serde_json::Value::String("9007199254740992".to_owned()),
            ]
        );
    }

    #[test]
    fn native_environment_state_restores_persisted_records() {
        use app_storage::EnvironmentStore;
        use environment_manager::{EnvironmentMode, EnvironmentRecord, EnvironmentState};

        let path = std::env::temp_dir().join(format!(
            "sql-judge-tauri-environment-state-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut store = EnvironmentStore::open(&path).expect("open environment store");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-a".to_owned(),
                instance_id: "instance-a".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_a".to_owned(),
                ownership_nonce: "nonce-a".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::NeedsRecovery,
            })
            .expect("save environment");

        let state = load_environment_state(&path).expect("restore native state");
        assert_eq!(
            state
                .ledger
                .inspect("env-a")
                .expect("restored environment")
                .state,
            EnvironmentState::NeedsRecovery
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn native_environment_state_marks_interrupted_operations_for_recovery() {
        use app_storage::EnvironmentStore;
        use environment_manager::{EnvironmentMode, EnvironmentRecord, EnvironmentState};

        let path = std::env::temp_dir().join(format!(
            "sql-judge-tauri-environment-recovery-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut store = EnvironmentStore::open(&path).expect("open environment store");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-rebuild".to_owned(),
                instance_id: "instance-rebuild".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_rebuild".to_owned(),
                ownership_nonce: "nonce-rebuild".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::Rebuilding,
            })
            .expect("save interrupted environment");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-running".to_owned(),
                instance_id: "instance-running".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_running".to_owned(),
                ownership_nonce: "nonce-running".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::Running,
            })
            .expect("save running environment");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-provision-journaled".to_owned(),
                instance_id: "instance-provision-journaled".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_provision_journaled".to_owned(),
                ownership_nonce: "nonce-provision-journaled".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::Provisioning,
            })
            .expect("save journaled provisioning environment");
        store
            .save_recovery(&app_storage::RecoveryRecord {
                env_id: "env-provision-journaled".to_owned(),
                operation: app_storage::RecoveryOperation::Provision,
                instance_id: "instance-provision-journaled".to_owned(),
                ownership_nonce: "nonce-provision-journaled".to_owned(),
                namespace: "sj_env_provision_journaled".to_owned(),
                replacement_namespace: None,
            })
            .expect("save provisioning recovery journal");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-provision-unjournaled".to_owned(),
                instance_id: "instance-provision-unjournaled".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_provision_unjournaled".to_owned(),
                ownership_nonce: "nonce-provision-unjournaled".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::Provisioning,
            })
            .expect("save unjournaled provisioning environment");
        store
            .save_environment(&EnvironmentRecord {
                env_id: "env-provision-mismatched".to_owned(),
                instance_id: "instance-provision-mismatched".to_owned(),
                pack_id: "hr-analytics".to_owned(),
                pack_version: "1.0.0".to_owned(),
                pack_digest: "a".repeat(64),
                namespace: "sj_env_provision_mismatched".to_owned(),
                ownership_nonce: "nonce-provision-mismatched".to_owned(),
                mode: EnvironmentMode::Managed,
                state: EnvironmentState::Provisioning,
            })
            .expect("save mismatched provisioning environment");
        store
            .save_recovery(&app_storage::RecoveryRecord {
                env_id: "env-provision-mismatched".to_owned(),
                operation: app_storage::RecoveryOperation::Provision,
                instance_id: "wrong-instance".to_owned(),
                ownership_nonce: "nonce-provision-mismatched".to_owned(),
                namespace: "sj_env_provision_mismatched".to_owned(),
                replacement_namespace: None,
            })
            .expect("save mismatched recovery journal");

        let state = load_environment_state(&path).expect("restore native state");
        assert_eq!(
            state
                .ledger
                .inspect("env-rebuild")
                .expect("restored environment")
                .state,
            EnvironmentState::NeedsRecovery
        );
        let persisted = EnvironmentStore::open(&path)
            .expect("reopen environment store")
            .get_environment("env-rebuild")
            .expect("read recovered environment")
            .expect("environment exists");
        assert_eq!(persisted.state, EnvironmentState::NeedsRecovery);
        let running = EnvironmentStore::open(&path)
            .expect("reopen environment store")
            .get_environment("env-running")
            .expect("read recovered running environment")
            .expect("running environment exists");
        assert_eq!(running.state, EnvironmentState::NeedsRecovery);
        assert_eq!(
            state
                .ledger
                .inspect("env-provision-journaled")
                .expect("journaled provisioning environment")
                .state,
            EnvironmentState::Provisioning
        );
        assert_eq!(
            EnvironmentStore::open(&path)
                .expect("reopen environment store")
                .get_environment("env-provision-journaled")
                .expect("read journaled provisioning environment")
                .expect("journaled provisioning environment exists")
                .state,
            EnvironmentState::Provisioning
        );
        assert_eq!(
            state
                .ledger
                .inspect("env-provision-unjournaled")
                .expect("unjournaled provisioning environment")
                .state,
            EnvironmentState::NeedsRecovery
        );
        assert_eq!(
            state
                .ledger
                .inspect("env-provision-mismatched")
                .expect("mismatched provisioning environment")
                .state,
            EnvironmentState::NeedsRecovery
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn managed_environment_views_exclude_external_records() {
        use environment_manager::{EnvironmentMode, EnvironmentRecord, EnvironmentState};

        let external = EnvironmentRecord {
            env_id: "external-a".to_owned(),
            instance_id: "instance-external".to_owned(),
            pack_id: "pack-a".to_owned(),
            pack_version: String::new(),
            pack_digest: String::new(),
            namespace: "sj_external".to_owned(),
            ownership_nonce: "not-owned".to_owned(),
            mode: EnvironmentMode::ExternalExplorer,
            state: EnvironmentState::Ready,
        };
        assert!(managed_environment_summary(&external).is_none());
    }
}
