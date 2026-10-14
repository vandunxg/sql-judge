use environment_manager::{EnvironmentMode, EnvironmentRecord, EnvironmentState};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid history record: {0}")]
    InvalidRecord(&'static str),
    #[error("invalid environment record: {0}")]
    InvalidEnvironment(&'static str),
    #[error("invalid connection profile: {0}")]
    InvalidConnectionProfile(&'static str),
    #[error("metadata backup exceeds the supported size limit")]
    BackupTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionRecord {
    pub submission_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_hash: String,
    pub engine_version: String,
    pub policy_version: String,
    pub verdict: String,
    pub created_at: String,
    pub problem_id: Option<String>,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRecord {
    pub test_id: String,
    pub verdict: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOperation {
    Provision,
    Rebuild,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryRecord {
    pub env_id: String,
    pub operation: RecoveryOperation,
    pub instance_id: String,
    pub ownership_nonce: String,
    pub namespace: String,
    pub replacement_namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionSummary {
    pub submission_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_hash: String,
    pub engine_version: String,
    pub policy_version: String,
    pub verdict: String,
    pub created_at: String,
    pub problem_id: Option<String>,
    pub mode: String,
    pub tests: Vec<TestRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub profile_id: String,
    pub display_name: String,
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub tls_mode: String,
    pub secret_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupSnapshot {
    pub version: u32,
    pub environments: Vec<BackupEnvironment>,
    pub submissions: Vec<BackupSubmission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackupEnvironment {
    pub env_id: String,
    pub instance_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_digest: String,
    pub namespace: String,
    pub ownership_nonce: String,
    pub mode: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackupSubmission {
    pub submission_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_hash: String,
    pub engine_version: String,
    pub policy_version: String,
    pub verdict: String,
    pub created_at: String,
    #[serde(default)]
    pub problem_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub tests: Vec<BackupTest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackupTest {
    pub test_id: String,
    pub verdict: String,
    pub elapsed_ms: u64,
}

const BACKUP_VERSION: u32 = 3;
const LEGACY_BACKUP_VERSION: u32 = 2;
const SUBMISSION_COLUMNS: &str = "submission_id, pack_id, pack_version, pack_hash, engine_version, policy_version, verdict, created_at, problem_id, mode";
pub const MAX_BACKUP_BYTES: usize = 4 * 1024 * 1024;

struct BackupSizeGuard {
    bytes: usize,
}

impl BackupSizeGuard {
    fn new() -> Self {
        Self { bytes: 0 }
    }

    fn can_add(&self, additional: usize) -> bool {
        self.bytes
            .checked_add(additional)
            .is_some_and(|total| total <= MAX_BACKUP_BYTES)
    }

    fn add_bytes(&mut self, additional: usize) -> Result<(), StorageError> {
        if !self.can_add(additional) {
            return Err(StorageError::BackupTooLarge);
        }
        self.bytes += additional;
        Ok(())
    }

    fn add_serialized<T: Serialize>(&mut self, value: &T) -> Result<(), StorageError> {
        let mut writer = CountingWriter { bytes: self.bytes };
        serde_json::to_writer(&mut writer, value).map_err(|_| StorageError::BackupTooLarge)?;
        self.bytes = writer.bytes;
        Ok(())
    }
}

struct CountingWriter {
    bytes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes.saturating_add(buffer.len()) > MAX_BACKUP_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "metadata backup size limit exceeded",
            ));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_size<T: Serialize>(value: &T) -> Result<usize, StorageError> {
    let mut writer = CountingWriter { bytes: 0 };
    serde_json::to_writer(&mut writer, value).map_err(|_| StorageError::BackupTooLarge)?;
    Ok(writer.bytes)
}

pub struct HistoryStore {
    connection: Connection,
}

pub struct EnvironmentStore {
    connection: Connection,
}

impl EnvironmentStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS environment_ledger (
                 env_id TEXT PRIMARY KEY NOT NULL,
                 instance_id TEXT NOT NULL,
                 pack_id TEXT NOT NULL,
                 pack_version TEXT NOT NULL DEFAULT '',
                 pack_digest TEXT NOT NULL DEFAULT '',
                 namespace TEXT NOT NULL UNIQUE,
                 ownership_nonce TEXT NOT NULL,
                 mode TEXT NOT NULL CHECK (mode IN ('managed', 'external_explorer')),
                 state TEXT NOT NULL CHECK (state IN (
                     'provisioning', 'ready', 'running', 'rebuilding',
                     'deleting', 'needs_recovery', 'error'
                 ))
             );
             CREATE INDEX IF NOT EXISTS idx_environment_pack ON environment_ledger(pack_id);
             CREATE TABLE IF NOT EXISTS runtime_metadata (
                 metadata_key TEXT PRIMARY KEY NOT NULL,
                 metadata_value TEXT NOT NULL
             );
              CREATE TABLE IF NOT EXISTS environment_recovery (
                 env_id TEXT PRIMARY KEY NOT NULL,
                 operation TEXT NOT NULL CHECK (operation IN ('provision', 'rebuild', 'delete')),
                 instance_id TEXT NOT NULL,
                 ownership_nonce TEXT NOT NULL,
                 namespace TEXT NOT NULL,
                  replacement_namespace TEXT
              );
              CREATE TABLE IF NOT EXISTS connection_profiles (
                  profile_id TEXT PRIMARY KEY NOT NULL,
                  display_name TEXT NOT NULL,
                  engine TEXT NOT NULL CHECK (engine IN ('postgresql', 'sqlserver')),
                  host TEXT NOT NULL,
                  port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
                  database_name TEXT NOT NULL,
                  username TEXT NOT NULL,
                  tls_mode TEXT NOT NULL CHECK (
                      tls_mode IN ('disable', 'require', 'verify_ca', 'verify_full')
                  ),
                  secret_ref TEXT
              );",
        )?;
        migrate_environment_identity_columns(&connection)?;
        Ok(Self { connection })
    }

    pub fn get_or_create_runtime_instance_id(&mut self) -> Result<String, StorageError> {
        let generated = format!(
            "runtime-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO runtime_metadata (metadata_key, metadata_value)
             VALUES ('runtime_instance_id', ?1)",
            [&generated],
        )?;
        let instance_id = transaction.query_row(
            "SELECT metadata_value FROM runtime_metadata
             WHERE metadata_key = 'runtime_instance_id'",
            [],
            |row| row.get::<_, String>(0),
        )?;
        transaction.commit()?;
        if instance_id.trim().is_empty() {
            return Err(StorageError::InvalidEnvironment("runtime instance id"));
        }
        Ok(instance_id)
    }

    pub fn save_environment(
        &mut self,
        environment: &EnvironmentRecord,
    ) -> Result<(), StorageError> {
        validate_environment(environment)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO environment_ledger (
                 env_id, instance_id, pack_id, pack_version, pack_digest, namespace,
                 ownership_nonce, mode, state
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(env_id) DO UPDATE SET
                 instance_id = excluded.instance_id,
                 pack_id = excluded.pack_id,
                 pack_version = excluded.pack_version,
                 pack_digest = excluded.pack_digest,
                 namespace = excluded.namespace,
                 ownership_nonce = excluded.ownership_nonce,
                 mode = excluded.mode,
                 state = excluded.state",
            params![
                environment.env_id,
                environment.instance_id,
                environment.pack_id,
                environment.pack_version,
                environment.pack_digest,
                environment.namespace,
                environment.ownership_nonce,
                environment_mode_name(environment.mode),
                environment_state_name(environment.state),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_environment(&self, env_id: &str) -> Result<Option<EnvironmentRecord>, StorageError> {
        if env_id.trim().is_empty() {
            return Err(StorageError::InvalidEnvironment(
                "environment id must be non-empty",
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT env_id, instance_id, pack_id, pack_version, pack_digest,
                    namespace, ownership_nonce, mode, state
             FROM environment_ledger WHERE env_id = ?1",
        )?;
        let row = statement
            .query_row([env_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .optional()?;

        row.map(decode_environment).transpose()
    }

    pub fn delete_environment(&mut self, env_id: &str) -> Result<(), StorageError> {
        if env_id.trim().is_empty() {
            return Err(StorageError::InvalidEnvironment(
                "environment id must be non-empty",
            ));
        }
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM environment_ledger WHERE env_id = ?1", [env_id])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_environments(&self) -> Result<Vec<EnvironmentRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT env_id, instance_id, pack_id, pack_version, pack_digest,
                    namespace, ownership_nonce, mode, state
             FROM environment_ledger ORDER BY env_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;

        let mut environments = Vec::new();
        for row in rows {
            environments.push(decode_environment(row?)?);
        }
        Ok(environments)
    }

    pub fn for_each_environment<F>(&self, mut visit: F) -> Result<(), StorageError>
    where
        F: FnMut(EnvironmentRecord) -> Result<(), StorageError>,
    {
        let mut statement = self.connection.prepare(
            "SELECT env_id, instance_id, pack_id, pack_version, pack_digest,
                    namespace, ownership_nonce, mode, state
             FROM environment_ledger ORDER BY env_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;
        for row in rows {
            visit(decode_environment(row?)?)?;
        }
        Ok(())
    }

    pub fn save_recovery(&mut self, recovery: &RecoveryRecord) -> Result<(), StorageError> {
        validate_recovery(recovery)?;
        self.connection.execute(
            "INSERT INTO environment_recovery (
                 env_id, operation, instance_id, ownership_nonce, namespace, replacement_namespace
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(env_id) DO UPDATE SET
                 operation = excluded.operation,
                 instance_id = excluded.instance_id,
                 ownership_nonce = excluded.ownership_nonce,
                 namespace = excluded.namespace,
                 replacement_namespace = excluded.replacement_namespace",
            params![
                recovery.env_id,
                recovery_operation_name(recovery.operation),
                recovery.instance_id,
                recovery.ownership_nonce,
                recovery.namespace,
                recovery.replacement_namespace,
            ],
        )?;
        Ok(())
    }

    pub fn list_recoveries(&self) -> Result<Vec<RecoveryRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT env_id, operation, instance_id, ownership_nonce, namespace, replacement_namespace
             FROM environment_recovery ORDER BY env_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.map(|row| decode_recovery(row?)).collect()
    }

    pub fn clear_recovery(&mut self, env_id: &str) -> Result<(), StorageError> {
        if env_id.trim().is_empty() {
            return Err(StorageError::InvalidEnvironment("environment id"));
        }
        self.connection.execute(
            "DELETE FROM environment_recovery WHERE env_id = ?1",
            [env_id],
        )?;
        Ok(())
    }

    pub fn save_connection_profile(
        &mut self,
        profile: &ConnectionProfile,
    ) -> Result<(), StorageError> {
        validate_connection_profile(profile)?;
        self.connection.execute(
            "INSERT INTO connection_profiles (
                 profile_id, display_name, engine, host, port, database_name,
                 username, tls_mode, secret_ref
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(profile_id) DO UPDATE SET
                 display_name = excluded.display_name,
                 engine = excluded.engine,
                 host = excluded.host,
                 port = excluded.port,
                 database_name = excluded.database_name,
                 username = excluded.username,
                 tls_mode = excluded.tls_mode,
                 secret_ref = excluded.secret_ref",
            params![
                profile.profile_id,
                profile.display_name,
                profile.engine,
                profile.host,
                profile.port,
                profile.database,
                profile.username,
                profile.tls_mode,
                profile.secret_ref,
            ],
        )?;
        Ok(())
    }

    pub fn get_connection_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<ConnectionProfile>, StorageError> {
        if profile_id.trim().is_empty() {
            return Err(StorageError::InvalidConnectionProfile("profile id"));
        }
        let mut statement = self.connection.prepare(
            "SELECT profile_id, display_name, engine, host, port, database_name,
                    username, tls_mode, secret_ref
             FROM connection_profiles WHERE profile_id = ?1",
        )?;
        let profile = statement
            .query_row([profile_id], |row| {
                Ok(ConnectionProfile {
                    profile_id: row.get(0)?,
                    display_name: row.get(1)?,
                    engine: row.get(2)?,
                    host: row.get(3)?,
                    port: row.get(4)?,
                    database: row.get(5)?,
                    username: row.get(6)?,
                    tls_mode: row.get(7)?,
                    secret_ref: row.get(8)?,
                })
            })
            .optional()?;
        profile
            .map(|profile| {
                validate_connection_profile(&profile)?;
                Ok(profile)
            })
            .transpose()
    }

    pub fn list_connection_profiles(&self) -> Result<Vec<ConnectionProfile>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT profile_id, display_name, engine, host, port, database_name,
                    username, tls_mode, secret_ref
             FROM connection_profiles ORDER BY profile_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ConnectionProfile {
                profile_id: row.get(0)?,
                display_name: row.get(1)?,
                engine: row.get(2)?,
                host: row.get(3)?,
                port: row.get(4)?,
                database: row.get(5)?,
                username: row.get(6)?,
                tls_mode: row.get(7)?,
                secret_ref: row.get(8)?,
            })
        })?;
        rows.map(|row| {
            let profile = row?;
            validate_connection_profile(&profile)?;
            Ok(profile)
        })
        .collect()
    }

    pub fn delete_connection_profile(&mut self, profile_id: &str) -> Result<(), StorageError> {
        if profile_id.trim().is_empty() {
            return Err(StorageError::InvalidConnectionProfile("profile id"));
        }
        self.connection.execute(
            "DELETE FROM connection_profiles WHERE profile_id = ?1",
            [profile_id],
        )?;
        Ok(())
    }
}

fn validate_connection_profile(profile: &ConnectionProfile) -> Result<(), StorageError> {
    for (value, field) in [
        (&profile.profile_id, "profile id"),
        (&profile.display_name, "display name"),
        (&profile.engine, "engine"),
        (&profile.host, "host"),
        (&profile.database, "database"),
        (&profile.username, "username"),
        (&profile.tls_mode, "tls mode"),
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(StorageError::InvalidConnectionProfile(field));
        }
    }
    if !profile
        .profile_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(StorageError::InvalidConnectionProfile("profile id"));
    }
    if !matches!(profile.engine.as_str(), "postgresql" | "sqlserver") {
        return Err(StorageError::InvalidConnectionProfile("engine"));
    }
    if !matches!(
        profile.tls_mode.as_str(),
        "require" | "verify_ca" | "verify_full"
    ) {
        return Err(StorageError::InvalidConnectionProfile("tls mode"));
    }
    if profile.port == 0 {
        return Err(StorageError::InvalidConnectionProfile("port"));
    }
    if profile.secret_ref.as_deref().is_some_and(|secret_ref| {
        secret_ref.trim().is_empty()
            || secret_ref.len() > 256
            || secret_ref.chars().any(char::is_control)
    }) {
        return Err(StorageError::InvalidConnectionProfile("secret ref"));
    }
    Ok(())
}

fn validate_environment(environment: &EnvironmentRecord) -> Result<(), StorageError> {
    [
        (&environment.env_id, "environment id"),
        (&environment.instance_id, "instance id"),
        (&environment.pack_id, "pack id"),
        (&environment.namespace, "namespace"),
        (&environment.ownership_nonce, "ownership nonce"),
    ]
    .iter()
    .find(|(value, _)| value.trim().is_empty())
    .map_or_else(
        || {
            if !is_owned_namespace(&environment.namespace) {
                Err(StorageError::InvalidEnvironment("namespace"))
            } else if environment.mode == EnvironmentMode::Managed
                && (!is_pack_version(&environment.pack_version)
                    || !is_pack_digest(&environment.pack_digest))
            {
                Err(StorageError::InvalidEnvironment("pack identity"))
            } else {
                Ok(())
            }
        },
        |(_, field)| Err(StorageError::InvalidEnvironment(field)),
    )
}

fn is_pack_version(version: &str) -> bool {
    !version.trim().is_empty() && version.len() <= 64
}

fn is_pack_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_backup_environment_state(environment: &EnvironmentRecord) -> Result<(), StorageError> {
    if environment.mode == EnvironmentMode::Managed
        && !matches!(
            environment.state,
            EnvironmentState::Ready | EnvironmentState::Error
        )
    {
        return Err(StorageError::InvalidEnvironment("environment state"));
    }
    Ok(())
}

fn is_owned_namespace(namespace: &str) -> bool {
    namespace.len() <= 63
        && namespace.starts_with("sj_")
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn validate_recovery(recovery: &RecoveryRecord) -> Result<(), StorageError> {
    if recovery.env_id.trim().is_empty() {
        return Err(StorageError::InvalidEnvironment("environment id"));
    }
    if recovery.instance_id.trim().is_empty() {
        return Err(StorageError::InvalidEnvironment("instance id"));
    }
    if recovery.ownership_nonce.trim().is_empty() {
        return Err(StorageError::InvalidEnvironment("ownership nonce"));
    }
    if !is_owned_namespace(&recovery.namespace) {
        return Err(StorageError::InvalidEnvironment("namespace"));
    }
    if recovery
        .replacement_namespace
        .as_deref()
        .is_some_and(|namespace| !is_owned_namespace(namespace))
    {
        return Err(StorageError::InvalidEnvironment("replacement namespace"));
    }
    Ok(())
}

fn recovery_operation_name(operation: RecoveryOperation) -> &'static str {
    match operation {
        RecoveryOperation::Provision => "provision",
        RecoveryOperation::Rebuild => "rebuild",
        RecoveryOperation::Delete => "delete",
    }
}

fn decode_recovery(
    (env_id, operation, instance_id, ownership_nonce, namespace, replacement_namespace): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    ),
) -> Result<RecoveryRecord, StorageError> {
    let operation = match operation.as_str() {
        "provision" => RecoveryOperation::Provision,
        "rebuild" => RecoveryOperation::Rebuild,
        "delete" => RecoveryOperation::Delete,
        _ => {
            return Err(StorageError::InvalidEnvironment(
                "unknown recovery operation",
            ));
        }
    };
    let recovery = RecoveryRecord {
        env_id,
        operation,
        instance_id,
        ownership_nonce,
        namespace,
        replacement_namespace,
    };
    validate_recovery(&recovery)?;
    Ok(recovery)
}

fn environment_mode_name(mode: EnvironmentMode) -> &'static str {
    match mode {
        EnvironmentMode::Managed => "managed",
        EnvironmentMode::ExternalExplorer => "external_explorer",
    }
}

fn environment_state_name(state: EnvironmentState) -> &'static str {
    match state {
        EnvironmentState::Provisioning => "provisioning",
        EnvironmentState::Ready => "ready",
        EnvironmentState::Running => "running",
        EnvironmentState::Rebuilding => "rebuilding",
        EnvironmentState::Deleting => "deleting",
        EnvironmentState::NeedsRecovery => "needs_recovery",
        EnvironmentState::Error => "error",
    }
}

fn decode_environment(
    (
        env_id,
        instance_id,
        pack_id,
        pack_version,
        pack_digest,
        namespace,
        ownership_nonce,
        mode,
        state,
    ): (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ),
) -> Result<EnvironmentRecord, StorageError> {
    let mode = match mode.as_str() {
        "managed" => EnvironmentMode::Managed,
        "external_explorer" => EnvironmentMode::ExternalExplorer,
        _ => return Err(StorageError::InvalidEnvironment("unknown environment mode")),
    };
    let state = match state.as_str() {
        "provisioning" => EnvironmentState::Provisioning,
        "ready" => EnvironmentState::Ready,
        "running" => EnvironmentState::Running,
        "rebuilding" => EnvironmentState::Rebuilding,
        "deleting" => EnvironmentState::Deleting,
        "needs_recovery" => EnvironmentState::NeedsRecovery,
        "error" => EnvironmentState::Error,
        _ => {
            return Err(StorageError::InvalidEnvironment(
                "unknown environment state",
            ));
        }
    };
    Ok(EnvironmentRecord {
        env_id,
        instance_id,
        pack_id,
        pack_version,
        pack_digest,
        namespace,
        ownership_nonce,
        mode,
        state,
    })
}

fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let columns = statement
        .query_map([table], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

fn migrate_environment_identity_columns(connection: &Connection) -> Result<(), StorageError> {
    let columns = table_columns(connection, "environment_ledger")?;
    if !columns.iter().any(|column| column == "pack_version") {
        connection.execute(
            "ALTER TABLE environment_ledger ADD COLUMN pack_version TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "pack_digest") {
        connection.execute(
            "ALTER TABLE environment_ledger ADD COLUMN pack_digest TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

impl HistoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                 version INTEGER PRIMARY KEY CHECK (version = 1)
             );
             INSERT OR IGNORE INTO schema_version(version) VALUES (1);
             CREATE TABLE IF NOT EXISTS submissions (
                 submission_id TEXT PRIMARY KEY NOT NULL,
                 pack_id TEXT NOT NULL,
                 pack_version TEXT NOT NULL,
                 pack_hash TEXT NOT NULL,
                 engine_version TEXT NOT NULL,
                 policy_version TEXT NOT NULL,
                 verdict TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS submission_tests (
                 submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
                 test_id TEXT NOT NULL,
                 verdict TEXT NOT NULL,
                 elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
                 PRIMARY KEY (submission_id, test_id)
             );
             CREATE INDEX IF NOT EXISTS idx_submissions_created_at ON submissions(created_at);",
        )?;
        migrate_submission_tracking_columns(&transaction)?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn save_submission(
        &mut self,
        submission: &SubmissionRecord,
        tests: &[TestRecord],
    ) -> Result<(), StorageError> {
        validate_submission(submission)?;
        tests.iter().try_for_each(validate_test)?;
        let transaction = self.connection.transaction()?;
        insert_submission(&transaction, submission, tests)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_submissions(&self) -> Result<Vec<SubmissionSummary>, StorageError> {
        let mut tests = std::collections::BTreeMap::<String, Vec<TestRecord>>::new();
        let mut statement = self.connection.prepare(
            "SELECT submission_id, test_id, verdict, elapsed_ms
             FROM submission_tests ORDER BY submission_id, test_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, decode_test(row, 1)?))
        })?;
        for row in rows {
            let (submission_id, test) = row?;
            tests.entry(submission_id).or_default().push(test);
        }

        let mut submissions = Vec::new();
        self.for_each_submission(|submission| {
            let tests = tests.remove(&submission.submission_id).unwrap_or_default();
            submissions.push(SubmissionSummary {
                submission_id: submission.submission_id,
                pack_id: submission.pack_id,
                pack_version: submission.pack_version,
                pack_hash: submission.pack_hash,
                engine_version: submission.engine_version,
                policy_version: submission.policy_version,
                verdict: submission.verdict,
                created_at: submission.created_at,
                problem_id: submission.problem_id,
                mode: submission.mode,
                tests,
            });
            Ok(())
        })?;
        Ok(submissions)
    }

    pub fn for_each_submission<F>(&self, mut visit: F) -> Result<(), StorageError>
    where
        F: FnMut(SubmissionRecord) -> Result<(), StorageError>,
    {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SUBMISSION_COLUMNS}
             FROM submissions ORDER BY created_at DESC, submission_id DESC"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(SubmissionRecord {
                submission_id: row.get(0)?,
                pack_id: row.get(1)?,
                pack_version: row.get(2)?,
                pack_hash: row.get(3)?,
                engine_version: row.get(4)?,
                policy_version: row.get(5)?,
                verdict: row.get(6)?,
                created_at: row.get(7)?,
                problem_id: row.get(8)?,
                mode: row.get(9)?,
            })
        })?;
        for row in rows {
            visit(row?)?;
        }
        Ok(())
    }

    pub fn test_count(&self, submission_id: &str) -> Result<usize, StorageError> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM submission_tests WHERE submission_id = ?1",
            [submission_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn list_submission_tests(
        &self,
        submission_id: &str,
    ) -> Result<Vec<TestRecord>, StorageError> {
        if submission_id.trim().is_empty() {
            return Err(StorageError::InvalidRecord("submission id"));
        }
        let mut statement = self.connection.prepare(
            "SELECT test_id, verdict, elapsed_ms
             FROM submission_tests WHERE submission_id = ?1 ORDER BY test_id",
        )?;
        let rows = statement.query_map([submission_id], |row| decode_test(row, 0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn for_each_submission_test<F>(
        &self,
        submission_id: &str,
        mut visit: F,
    ) -> Result<(), StorageError>
    where
        F: FnMut(TestRecord) -> Result<(), StorageError>,
    {
        if submission_id.trim().is_empty() {
            return Err(StorageError::InvalidRecord("submission id"));
        }
        let mut statement = self.connection.prepare(
            "SELECT test_id, verdict, elapsed_ms
             FROM submission_tests WHERE submission_id = ?1 ORDER BY test_id",
        )?;
        let rows = statement.query_map([submission_id], |row| decode_test(row, 0))?;
        for row in rows {
            visit(row?)?;
        }
        Ok(())
    }
}

pub fn export_backup(path: impl AsRef<Path>) -> Result<BackupSnapshot, StorageError> {
    let environments_store = EnvironmentStore::open(&path)?;
    if !environments_store.list_recoveries()?.is_empty() {
        return Err(StorageError::InvalidEnvironment(
            "pending environment recovery",
        ));
    }
    let history = HistoryStore::open(path)?;
    let mut size = BackupSizeGuard::new();
    size.add_bytes(b"{\"version\":1,\"environments\":[".len())?;

    let mut environments = Vec::new();
    let mut first_environment = true;
    environments_store.for_each_environment(|environment| {
        validate_backup_environment_state(&environment)?;
        let backup = BackupEnvironment {
            env_id: environment.env_id,
            instance_id: environment.instance_id,
            pack_id: environment.pack_id,
            pack_version: environment.pack_version,
            pack_digest: environment.pack_digest,
            namespace: environment.namespace,
            ownership_nonce: environment.ownership_nonce,
            mode: environment_mode_name(environment.mode).to_owned(),
            state: environment_state_name(environment.state).to_owned(),
        };
        if !first_environment {
            size.add_bytes(1)?;
        }
        size.add_serialized(&backup)?;
        first_environment = false;
        environments.push(backup);
        Ok(())
    })?;
    size.add_bytes(b"],\"submissions\":[".len())?;

    let mut submissions = Vec::new();
    let mut first_submission = true;
    history.for_each_submission(|submission| {
        let base = BackupSubmission {
            submission_id: submission.submission_id.clone(),
            pack_id: submission.pack_id.clone(),
            pack_version: submission.pack_version.clone(),
            pack_hash: submission.pack_hash.clone(),
            engine_version: submission.engine_version.clone(),
            policy_version: submission.policy_version.clone(),
            verdict: submission.verdict.clone(),
            created_at: submission.created_at.clone(),
            problem_id: submission.problem_id.clone(),
            mode: Some(submission.mode.clone()),
            tests: Vec::new(),
        };
        let base_size = serialized_size(&base)?;
        let mut tests = Vec::new();
        let mut submission_size = base_size;
        let mut first_test = true;
        history.for_each_submission_test(&submission.submission_id, |test| {
            let backup = BackupTest {
                test_id: test.test_id,
                verdict: test.verdict,
                elapsed_ms: test.elapsed_ms,
            };
            let test_size = serialized_size(&backup)?;
            if first_test {
                submission_size = base_size
                    .checked_sub(2)
                    .ok_or(StorageError::BackupTooLarge)?;
            } else {
                submission_size = submission_size
                    .checked_add(1)
                    .ok_or(StorageError::BackupTooLarge)?;
            }
            submission_size = submission_size
                .checked_add(test_size)
                .ok_or(StorageError::BackupTooLarge)?;
            let separator = usize::from(!first_submission);
            if !size.can_add(separator.saturating_add(submission_size)) {
                return Err(StorageError::BackupTooLarge);
            }
            first_test = false;
            tests.push(backup);
            Ok(())
        })?;
        if !size.can_add(usize::from(!first_submission).saturating_add(submission_size)) {
            return Err(StorageError::BackupTooLarge);
        }
        if !first_submission {
            size.add_bytes(1)?;
        }
        size.add_bytes(submission_size)?;
        first_submission = false;
        submissions.push(BackupSubmission {
            submission_id: base.submission_id,
            pack_id: base.pack_id,
            pack_version: base.pack_version,
            pack_hash: base.pack_hash,
            engine_version: base.engine_version,
            policy_version: base.policy_version,
            verdict: base.verdict,
            created_at: base.created_at,
            problem_id: base.problem_id,
            mode: base.mode,
            tests,
        });
        Ok(())
    })?;
    size.add_bytes(b"]}".len())?;

    Ok(BackupSnapshot {
        version: BACKUP_VERSION,
        environments,
        submissions,
    })
}

pub fn restore_backup(
    path: impl AsRef<Path>,
    snapshot: &BackupSnapshot,
) -> Result<(), StorageError> {
    if !matches!(snapshot.version, LEGACY_BACKUP_VERSION | BACKUP_VERSION) {
        return Err(StorageError::InvalidRecord("backup version"));
    }

    let mut environments = Vec::with_capacity(snapshot.environments.len());
    let mut environment_ids = std::collections::BTreeSet::new();
    let mut namespaces = std::collections::BTreeSet::new();
    for backup in &snapshot.environments {
        let record = decode_environment((
            backup.env_id.clone(),
            backup.instance_id.clone(),
            backup.pack_id.clone(),
            backup.pack_version.clone(),
            backup.pack_digest.clone(),
            backup.namespace.clone(),
            backup.ownership_nonce.clone(),
            backup.mode.clone(),
            backup.state.clone(),
        ))?;
        validate_environment(&record)?;
        validate_backup_environment_state(&record)?;
        if !environment_ids.insert(record.env_id.clone())
            || !namespaces.insert(record.namespace.clone())
        {
            return Err(StorageError::InvalidEnvironment(
                "duplicate backup environment",
            ));
        }
        environments.push(record);
    }

    let mut submissions = Vec::with_capacity(snapshot.submissions.len());
    let mut submission_ids = std::collections::BTreeSet::new();
    for backup in &snapshot.submissions {
        // Version 2 predates problem/run tracking, so its rows can only be untracked submissions.
        let (problem_id, mode) = match (snapshot.version, &backup.problem_id, &backup.mode) {
            (LEGACY_BACKUP_VERSION, None, None) => (None, "submit".to_owned()),
            (BACKUP_VERSION, problem_id, Some(mode)) => (problem_id.clone(), mode.clone()),
            _ => return Err(StorageError::InvalidRecord("backup submission tracking")),
        };
        let submission = SubmissionRecord {
            submission_id: backup.submission_id.clone(),
            pack_id: backup.pack_id.clone(),
            pack_version: backup.pack_version.clone(),
            pack_hash: backup.pack_hash.clone(),
            engine_version: backup.engine_version.clone(),
            policy_version: backup.policy_version.clone(),
            verdict: backup.verdict.clone(),
            created_at: backup.created_at.clone(),
            problem_id,
            mode,
        };
        validate_submission(&submission)?;
        if !submission_ids.insert(submission.submission_id.clone()) {
            return Err(StorageError::InvalidRecord("duplicate backup submission"));
        }
        let tests = backup
            .tests
            .iter()
            .map(|test| {
                let test = TestRecord {
                    test_id: test.test_id.clone(),
                    verdict: test.verdict.clone(),
                    elapsed_ms: test.elapsed_ms,
                };
                validate_test(&test)?;
                Ok(test)
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        submissions.push((submission, tests));
    }

    let current_store = EnvironmentStore::open(&path)?;
    let current_environments = current_store.list_environments()?;
    if !current_store.list_recoveries()?.is_empty() {
        return Err(StorageError::InvalidEnvironment(
            "pending environment recovery",
        ));
    }
    let current_managed = current_environments
        .iter()
        .filter(|environment| environment.mode == EnvironmentMode::Managed)
        .collect::<Vec<_>>();
    let backup_managed = environments
        .iter()
        .filter(|environment| environment.mode == EnvironmentMode::Managed)
        .collect::<Vec<_>>();
    if current_managed.len() != backup_managed.len()
        || current_managed.iter().any(|current| {
            backup_managed
                .iter()
                .find(|backup| backup.env_id == current.env_id)
                .is_none_or(|backup| {
                    backup.instance_id != current.instance_id
                        || backup.pack_id != current.pack_id
                        || backup.pack_version != current.pack_version
                        || backup.pack_digest != current.pack_digest
                        || backup.namespace != current.namespace
                        || backup.ownership_nonce != current.ownership_nonce
                })
        })
    {
        return Err(StorageError::InvalidEnvironment(
            "managed environment identity mismatch",
        ));
    }

    drop(HistoryStore::open(&path)?);
    let connection = Connection::open(path)?;
    let transaction = connection.unchecked_transaction()?;
    transaction.execute_batch(
        "DELETE FROM submission_tests;
         DELETE FROM submissions;
         DELETE FROM environment_ledger WHERE mode = 'managed';",
    )?;
    for environment in environments
        .into_iter()
        .filter(|environment| environment.mode == EnvironmentMode::Managed)
    {
        transaction.execute(
            "INSERT INTO environment_ledger (
                 env_id, instance_id, pack_id, pack_version, pack_digest, namespace,
                 ownership_nonce, mode, state
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                environment.env_id,
                environment.instance_id,
                environment.pack_id,
                environment.pack_version,
                environment.pack_digest,
                environment.namespace,
                environment.ownership_nonce,
                environment_mode_name(environment.mode),
                environment_state_name(environment.state),
            ],
        )?;
    }
    for (submission, tests) in submissions {
        insert_submission(&transaction, &submission, &tests)?;
    }
    transaction.commit()?;
    Ok(())
}

fn validate_submission(submission: &SubmissionRecord) -> Result<(), StorageError> {
    let fields = [
        (&submission.submission_id, "submission id"),
        (&submission.pack_id, "pack id"),
        (&submission.pack_version, "pack version"),
        (&submission.pack_hash, "pack hash"),
        (&submission.engine_version, "engine version"),
        (&submission.policy_version, "policy version"),
        (&submission.verdict, "verdict"),
        (&submission.created_at, "created at"),
    ];
    if let Some((_, field)) = fields.iter().find(|(value, _)| value.trim().is_empty()) {
        return Err(StorageError::InvalidRecord(field));
    }
    if !matches!(submission.mode.as_str(), "run" | "submit") {
        return Err(StorageError::InvalidRecord("mode"));
    }
    if submission
        .problem_id
        .as_deref()
        .is_some_and(|problem_id| !is_problem_id(problem_id))
    {
        return Err(StorageError::InvalidRecord("problem id"));
    }
    Ok(())
}

fn is_problem_id(problem_id: &str) -> bool {
    !problem_id.is_empty()
        && problem_id.len() <= 128
        && problem_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_test(test: &TestRecord) -> Result<(), StorageError> {
    if test.test_id.trim().is_empty() || test.verdict.trim().is_empty() {
        return Err(StorageError::InvalidRecord("test fields must be non-empty"));
    }
    Ok(())
}

fn decode_test(row: &Row<'_>, offset: usize) -> rusqlite::Result<TestRecord> {
    Ok(TestRecord {
        test_id: row.get(offset)?,
        verdict: row.get(offset + 1)?,
        elapsed_ms: row.get::<_, i64>(offset + 2)? as u64,
    })
}

fn insert_submission(
    connection: &Connection,
    submission: &SubmissionRecord,
    tests: &[TestRecord],
) -> Result<(), StorageError> {
    connection.execute(
        &format!(
            "INSERT INTO submissions ({SUBMISSION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
        ),
        params![
            submission.submission_id,
            submission.pack_id,
            submission.pack_version,
            submission.pack_hash,
            submission.engine_version,
            submission.policy_version,
            submission.verdict,
            submission.created_at,
            submission.problem_id,
            submission.mode,
        ],
    )?;
    for test in tests {
        let elapsed_ms = i64::try_from(test.elapsed_ms)
            .map_err(|_| StorageError::InvalidRecord("elapsed time is too large"))?;
        connection.execute(
            "INSERT INTO submission_tests (submission_id, test_id, verdict, elapsed_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                submission.submission_id,
                test.test_id,
                test.verdict,
                elapsed_ms
            ],
        )?;
    }
    Ok(())
}

/// Adds problem/run tracking to `submissions` created before it existed. Runs inside the
/// caller's IMMEDIATE transaction so concurrent openers cannot both add the same column.
fn migrate_submission_tracking_columns(connection: &Connection) -> Result<(), StorageError> {
    let columns = table_columns(connection, "submissions")?;
    if !columns.iter().any(|column| column == "problem_id") {
        connection.execute("ALTER TABLE submissions ADD COLUMN problem_id TEXT", [])?;
    }
    if !columns.iter().any(|column| column == "mode") {
        connection.execute(
            "ALTER TABLE submissions ADD COLUMN mode TEXT NOT NULL DEFAULT 'submit'
                 CHECK (mode IN ('run', 'submit'))",
            [],
        )?;
    }
    Ok(())
}
