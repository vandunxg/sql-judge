use fallible_iterator::FallibleIterator;
use judge_core::{Column, LogicalType, QueryLimits, QueryResult, SqlValue};
use postgres::{
    Client, Config, GenericClient, NoTls, Row,
    config::{Host, SslMode},
    types::{FromSql, ToSql, Type},
};
use postgres_native_tls::MakeTlsConnector;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

mod orchestrator;
mod runtime;

pub use orchestrator::{
    JudgeCase, SubmissionReport, TestOutcome, judge_submission,
    judge_submission_in_namespace_with_watchdog_token, judge_submission_with_cancel,
    judge_submission_with_token, judge_submission_with_watchdog,
    judge_submission_with_watchdog_token,
};
pub use runtime::ManagedPostgresRuntime;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueryExecutionError {
    #[error("query violates the read-only SQL policy")]
    PolicyViolation,
    #[error("query limits are invalid: {0}")]
    InvalidLimits(String),
    #[error("query has invalid SQL syntax")]
    SyntaxError,
    #[error("query exceeded its statement deadline")]
    TimeLimitExceeded,
    #[error("query was cancelled")]
    Cancelled,
    #[error("query exceeded the result limit")]
    ResultLimitExceeded,
    #[error("database execution failed")]
    RuntimeError,
    #[error("unsupported PostgreSQL result type: {0}")]
    UnsupportedType(String),
    #[error("could not connect to the managed PostgreSQL instance")]
    ConnectionError,
    #[error("managed namespace ownership marker does not match")]
    OwnershipVerificationFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTlsMode {
    VerifyFull,
    VerifyCa,
    Require,
    Disable,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ExternalPostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub tls_mode: ExternalTlsMode,
    pub allow_insecure_local: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExternalPostgresError {
    #[error("external connection profile is invalid: {0}")]
    InvalidProfile(&'static str),
    #[error("external TLS policy rejected")]
    TlsPolicyRejected,
    #[error("external TLS negotiation failed")]
    TlsFailed,
    #[error("external authentication failed")]
    AuthenticationFailed,
    #[error("external PostgreSQL server is unreachable")]
    ServerUnreachable,
    #[error("external query limits are invalid: {0}")]
    InvalidLimits(String),
    #[error("external SELECT query was rejected")]
    QueryRejected,
    #[error("external query timed out")]
    QueryTimeout,
    #[error("external query exceeded its result limit")]
    ResultLimitExceeded,
    #[error("external PostgreSQL operation failed")]
    DatabaseError,
    #[error("external PostgreSQL returned an unsupported type: {0}")]
    UnsupportedType(String),
    #[error("external sandbox ownership verification failed")]
    OwnershipVerificationFailed,
}

pub struct ExternalPostgresExplorer {
    client: Client,
}

/// A writable external target is intentionally separate from the read-only explorer.
/// Its caller must supply the exact `sj_` namespace that may be created or removed.
pub struct ExternalSandboxConfig {
    connection: ExternalPostgresConfig,
    profile_id: String,
    environment_id: String,
    namespace: String,
    ownership_nonce: String,
}

impl ExternalSandboxConfig {
    pub fn new(
        connection: ExternalPostgresConfig,
        profile_id: impl Into<String>,
        environment_id: impl Into<String>,
        namespace: impl Into<String>,
        ownership_nonce: impl Into<String>,
    ) -> Result<Self, ExternalPostgresError> {
        validate_external_config(&connection)?;
        let config = Self {
            connection,
            profile_id: profile_id.into(),
            environment_id: environment_id.into(),
            namespace: namespace.into(),
            ownership_nonce: ownership_nonce.into(),
        };
        for value in [
            &config.profile_id,
            &config.environment_id,
            &config.ownership_nonce,
        ] {
            if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
                return Err(ExternalPostgresError::InvalidProfile("sandbox identity"));
            }
        }
        if !is_safe_namespace(&config.namespace) {
            return Err(ExternalPostgresError::InvalidProfile("namespace"));
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalSandboxTargetIdentity {
    pub database: String,
    pub database_oid: u32,
    pub system_identifier: String,
}

pub struct ExternalPostgresSandbox {
    client: Client,
    config: ExternalSandboxConfig,
    target: ExternalSandboxTargetIdentity,
}

impl ExternalPostgresSandbox {
    pub fn connect(config: ExternalSandboxConfig) -> Result<Self, ExternalPostgresError> {
        let mut client = connect_external_client(&config.connection)?;
        // This privileged catalog function is a required capability: without a stable server
        // identity, an external target cannot safely be bound to a destructive sandbox marker.
        let target = client
            .query_one(
                "SELECT current_database(),
                        (SELECT oid FROM pg_catalog.pg_database WHERE datname = current_database())::text,
                        (pg_catalog.pg_control_system()).system_identifier::text",
                &[],
            )
            .map_err(map_external_connection_error)
            .and_then(|row| {
                let database_oid = row
                    .get::<_, String>(1)
                    .parse::<u32>()
                    .map_err(|_| ExternalPostgresError::DatabaseError)?;
                Ok(ExternalSandboxTargetIdentity {
                    database: row.get(0),
                    database_oid,
                    system_identifier: row.get(2),
                })
            })?;
        Ok(Self {
            client,
            config,
            target,
        })
    }

    pub fn target_identity(&self) -> &ExternalSandboxTargetIdentity {
        &self.target
    }

    pub fn provision(&mut self) -> Result<(), ExternalPostgresError> {
        let schema = quote_identifier(&self.config.namespace);
        let marker = format!("{schema}.\"__sql_judge_external_sandbox\"");
        let mut transaction = self
            .client
            .transaction()
            .map_err(map_external_connection_error)?;
        let result = (|| {
            let schema_exists = transaction
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = $1)",
                    &[&self.config.namespace],
                )
                .map_err(map_external_connection_error)?
                .get::<_, bool>(0);
            if schema_exists {
                return Err(ExternalPostgresError::OwnershipVerificationFailed);
            }
            transaction
                .batch_execute(&format!(
                    "CREATE SCHEMA {schema};
                     REVOKE ALL ON SCHEMA {schema} FROM PUBLIC;
                     CREATE TABLE {marker} (
                       marker_key text PRIMARY KEY CHECK (marker_key = 'external-sandbox'),
                       environment_id text NOT NULL,
                       profile_id text NOT NULL,
                       ownership_nonce text NOT NULL,
                       database_name text NOT NULL,
                       database_oid text NOT NULL,
                       system_identifier text NOT NULL
                     );
                     REVOKE ALL ON TABLE {marker} FROM PUBLIC"
                ))
                .map_err(map_external_connection_error)?;
            transaction
                .execute(
                    &format!(
                        "INSERT INTO {marker}
                         (marker_key, environment_id, profile_id, ownership_nonce, database_name, database_oid, system_identifier)
                         VALUES ('external-sandbox', $1, $2, $3, $4, $5, $6)"
                    ),
                    &[
                        &self.config.environment_id,
                        &self.config.profile_id,
                        &self.config.ownership_nonce,
                        &self.target.database,
                        &self.target.database_oid.to_string(),
                        &self.target.system_identifier,
                    ],
                )
                .map_err(map_external_connection_error)?;
            Ok(())
        })();
        match result {
            Ok(()) => transaction
                .commit()
                .map_err(map_external_connection_error)?,
            Err(error) => {
                let _ = transaction.rollback();
                return Err(error);
            }
        }
        if self.verify_ownership()? {
            Ok(())
        } else {
            Err(ExternalPostgresError::OwnershipVerificationFailed)
        }
    }

    pub fn verify_ownership(&mut self) -> Result<bool, ExternalPostgresError> {
        self.marker_is_owned()
    }

    pub fn delete_owned_schema(&mut self) -> Result<(), ExternalPostgresError> {
        let schema = quote_identifier(&self.config.namespace);
        let mut transaction = self
            .client
            .transaction()
            .map_err(map_external_connection_error)?;
        if !marker_is_owned(&mut transaction, &self.config, &self.target)? {
            let _ = transaction.rollback();
            return Err(ExternalPostgresError::OwnershipVerificationFailed);
        }
        transaction
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .map_err(map_external_connection_error)?;
        transaction.commit().map_err(map_external_connection_error)
    }

    fn marker_is_owned(&mut self) -> Result<bool, ExternalPostgresError> {
        marker_is_owned(&mut self.client, &self.config, &self.target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConnectionInfo {
    pub database: String,
    pub username: String,
    pub server_version: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct WriteCapabilitySnapshot {
    role: bool,
    database: bool,
    schema: bool,
    table: bool,
    sequence: bool,
    default_acl: bool,
}

impl WriteCapabilitySnapshot {
    fn read_only(self) -> bool {
        !self.role
            && !self.database
            && !self.schema
            && !self.table
            && !self.sequence
            && !self.default_acl
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalTable {
    pub schema: String,
    pub name: String,
    pub kind: String,
    pub columns: Vec<ExternalColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMetadata {
    pub tables: Vec<ExternalTable>,
    pub truncated: bool,
}

impl ExternalPostgresExplorer {
    pub fn connect(config: ExternalPostgresConfig) -> Result<Self, ExternalPostgresError> {
        Ok(Self {
            client: connect_external_client(&config)?,
        })
    }

    pub fn test_connection(&mut self) -> Result<ExternalConnectionInfo, ExternalPostgresError> {
        self.client
            .batch_execute("BEGIN READ ONLY; SET LOCAL statement_timeout = 2000")
            .map_err(map_external_connection_error)?;
        let result = (|| {
            let row = self
                .client
                .query_one(
                    "SELECT current_database(), current_user, current_setting('server_version'),
                            COALESCE((
                                SELECT r.rolsuper OR r.rolcreaterole OR r.rolcreatedb
                                       OR r.rolbypassrls
                                       OR EXISTS (
                                           SELECT 1
                                           FROM pg_catalog.pg_roles write_role
                                           WHERE write_role.rolname = 'pg_write_all_data'
                                             AND pg_catalog.pg_has_role(current_user, write_role.oid, 'member')
                                       )
                                FROM pg_catalog.pg_roles r
                                WHERE r.rolname = current_user
                            ), true) AS role_write,
                            -- TEMPORARY is granted to PUBLIC by default and only allows
                            -- session-local objects, which READ ONLY transactions also block.
                            has_database_privilege(current_user, current_database(), 'CREATE')
                                AS database_write,
                            EXISTS (
                                SELECT 1
                                FROM pg_catalog.pg_namespace n
                                WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
                                  AND n.nspname NOT LIKE 'pg_toast%'
                                  AND has_schema_privilege(current_user, n.oid, 'CREATE')
                            ) AS schema_write,
                            EXISTS (
                                SELECT 1
                                FROM pg_catalog.pg_class c
                                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                                WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
                                  AND n.nspname NOT LIKE 'pg_toast%'
                                  AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
                                  AND (
                                      has_table_privilege(current_user, c.oid, 'INSERT')
                                      OR has_table_privilege(current_user, c.oid, 'UPDATE')
                                      OR has_table_privilege(current_user, c.oid, 'DELETE')
                                      OR has_table_privilege(current_user, c.oid, 'TRUNCATE')
                                  )
                            ) AS table_write,
                            EXISTS (
                                SELECT 1
                                FROM pg_catalog.pg_class c
                                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                                WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
                                  AND n.nspname NOT LIKE 'pg_toast%'
                                  AND c.relkind = 'S'
                                  AND (
                                      has_sequence_privilege(current_user, c.oid, 'USAGE')
                                      OR has_sequence_privilege(current_user, c.oid, 'UPDATE')
                                  )
                            ) AS sequence_write,
                            EXISTS (
                                SELECT 1
                                FROM pg_catalog.pg_default_acl d
                                CROSS JOIN LATERAL pg_catalog.aclexplode(d.defaclacl) privilege
                                WHERE (
                                    (d.defaclobjtype = 'r' AND privilege.privilege_type IN ('INSERT', 'UPDATE', 'DELETE', 'TRUNCATE'))
                                    OR (d.defaclobjtype = 'S' AND privilege.privilege_type IN ('USAGE', 'UPDATE'))
                                    OR (d.defaclobjtype = 'n' AND privilege.privilege_type = 'CREATE')
                                )
                                AND (
                                    privilege.grantee = 0
                                    OR privilege.grantee = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = current_user)
                                    OR pg_catalog.pg_has_role(current_user, privilege.grantee, 'member')
                                )
                            ) AS default_acl_write",
                    &[],
                )
                .map_err(map_external_connection_error)?;
            let capabilities = WriteCapabilitySnapshot {
                role: row.get(3),
                database: row.get(4),
                schema: row.get(5),
                table: row.get(6),
                sequence: row.get(7),
                default_acl: row.get(8),
            };
            Ok(ExternalConnectionInfo {
                database: row.get(0),
                username: row.get(1),
                server_version: row.get(2),
                read_only: capabilities.read_only(),
            })
        })();
        let rollback = self.client.batch_execute("ROLLBACK");
        if rollback.is_err() {
            return Err(ExternalPostgresError::DatabaseError);
        }
        result
    }

    pub fn list_metadata(
        &mut self,
        limits: QueryLimits,
    ) -> Result<ExternalMetadata, ExternalPostgresError> {
        validate_external_limits(&limits)?;
        self.begin_read_only(&limits)?;
        let result = (|| {
            let requested = i64::from(limits.max_rows).saturating_add(1);
            let rows = self
                .client
                .query(
                    "SELECT n.nspname, c.relname, c.relkind::text, a.attname,
                            format_type(a.atttypid, a.atttypmod)
                     FROM pg_catalog.pg_class c
                     JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                     JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid
                     WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
                       AND n.nspname NOT LIKE 'pg_toast%'
                       AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
                       AND a.attnum > 0 AND NOT a.attisdropped
                     ORDER BY n.nspname, c.relname, a.attnum
                     LIMIT $1",
                    &[&requested],
                )
                .map_err(map_external_query_error)?;
            let truncated = rows.len() > limits.max_rows as usize;
            let mut tables = BTreeMap::<(String, String), ExternalTable>::new();
            let mut bytes = 0_u32;
            for row in rows.into_iter().take(limits.max_rows as usize) {
                let schema: String = row.get(0);
                let name: String = row.get(1);
                let kind: String = row.get(2);
                let column_name: String = row.get(3);
                let data_type: String = row.get(4);
                bytes = bytes
                    .checked_add(
                        u32::try_from(
                            schema.len()
                                + name.len()
                                + kind.len()
                                + column_name.len()
                                + data_type.len(),
                        )
                        .map_err(|_| ExternalPostgresError::ResultLimitExceeded)?,
                    )
                    .ok_or(ExternalPostgresError::ResultLimitExceeded)?;
                if bytes > limits.max_bytes {
                    return Err(ExternalPostgresError::ResultLimitExceeded);
                }
                tables
                    .entry((schema.clone(), name.clone()))
                    .or_insert_with(|| ExternalTable {
                        schema,
                        name,
                        kind,
                        columns: Vec::new(),
                    })
                    .columns
                    .push(ExternalColumn {
                        name: column_name,
                        data_type,
                    });
            }
            Ok(ExternalMetadata {
                tables: tables.into_values().collect(),
                truncated,
            })
        })();
        self.finish_read_only(result)
    }

    pub fn execute_select(
        &mut self,
        sql: &str,
        limits: QueryLimits,
    ) -> Result<QueryResult, ExternalPostgresError> {
        validate_external_select(sql)?;
        validate_external_limits(&limits)?;
        self.begin_read_only(&limits)?;
        let result = (|| {
            let statement = self.client.prepare(sql).map_err(map_external_query_error)?;
            let columns = statement
                .columns()
                .iter()
                .map(|column| {
                    logical_type(column.type_())
                        .map(|logical_type| Column::new(column.name(), logical_type))
                        .map_err(|error| match error {
                            QueryExecutionError::UnsupportedType(kind) => {
                                ExternalPostgresError::UnsupportedType(kind)
                            }
                            _ => ExternalPostgresError::DatabaseError,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut rows = self
                .client
                .query_raw(&statement, std::iter::empty::<&(dyn ToSql + Sync)>())
                .map_err(map_external_query_error)?;
            let mut values = Vec::with_capacity(limits.max_rows as usize);
            let mut result_bytes = 0_u32;
            let mut row_count = 0_usize;
            while let Some(row) = rows.next().map_err(map_external_query_error)? {
                if row_count >= limits.max_rows as usize {
                    return Err(ExternalPostgresError::ResultLimitExceeded);
                }
                let row_values = decode_row(&row).map_err(map_external_decode_error)?;
                result_bytes = result_bytes
                    .checked_add(
                        u32::try_from(
                            serde_json::to_vec(&row_values)
                                .map_err(|_| ExternalPostgresError::DatabaseError)?
                                .len(),
                        )
                        .map_err(|_| ExternalPostgresError::ResultLimitExceeded)?,
                    )
                    .ok_or(ExternalPostgresError::ResultLimitExceeded)?;
                if result_bytes > limits.max_bytes {
                    return Err(ExternalPostgresError::ResultLimitExceeded);
                }
                values.push(row_values);
                row_count += 1;
            }
            QueryResult::new(columns, values).map_err(|_| ExternalPostgresError::DatabaseError)
        })();
        self.finish_read_only(result)
    }

    fn begin_read_only(&mut self, limits: &QueryLimits) -> Result<(), ExternalPostgresError> {
        self.client
            .batch_execute(&format!(
                "BEGIN READ ONLY; SET LOCAL statement_timeout = {}; SET LOCAL idle_in_transaction_session_timeout = {}",
                limits.statement_timeout_ms, limits.statement_timeout_ms
            ))
            .map_err(map_external_query_error)
    }

    fn finish_read_only<T>(
        &mut self,
        result: Result<T, ExternalPostgresError>,
    ) -> Result<T, ExternalPostgresError> {
        if self.client.batch_execute("ROLLBACK").is_err() {
            return Err(ExternalPostgresError::DatabaseError);
        }
        result
    }
}

fn connect_external_client(
    config: &ExternalPostgresConfig,
) -> Result<Client, ExternalPostgresError> {
    validate_external_config(config)?;
    let mut connection = Config::new();
    connection
        .host(&config.host)
        .port(config.port)
        .dbname(&config.database)
        .user(&config.username)
        .password(&config.password);
    if config.tls_mode != ExternalTlsMode::Disable {
        // The driver default (`prefer`) silently continues in plaintext when the server declines TLS.
        connection.ssl_mode(SslMode::Require);
    }
    match config.tls_mode {
        ExternalTlsMode::Disable => connection
            .connect(NoTls)
            .map_err(map_external_connection_error),
        ExternalTlsMode::VerifyFull | ExternalTlsMode::Require => {
            let connector = native_tls::TlsConnector::builder()
                .build()
                .map_err(|_| ExternalPostgresError::TlsFailed)?;
            connection
                .connect(MakeTlsConnector::new(connector))
                .map_err(map_external_connection_error)
        }
        ExternalTlsMode::VerifyCa => {
            let connector = native_tls::TlsConnector::builder()
                .danger_accept_invalid_hostnames(true)
                .build()
                .map_err(|_| ExternalPostgresError::TlsFailed)?;
            connection
                .connect(MakeTlsConnector::new(connector))
                .map_err(map_external_connection_error)
        }
    }
}

fn marker_is_owned(
    client: &mut impl GenericClient,
    config: &ExternalSandboxConfig,
    target: &ExternalSandboxTargetIdentity,
) -> Result<bool, ExternalPostgresError> {
    let schema = quote_identifier(&config.namespace);
    let marker = format!("{schema}.\"__sql_judge_external_sandbox\"");
    let marker_is_owned = client
        .query_opt(
            &format!(
                "SELECT m.environment_id, m.profile_id, m.ownership_nonce,
                        m.database_name, m.database_oid, m.system_identifier,
                        n.nspowner = u.usesysid AND c.relowner = u.usesysid
                 FROM {marker} m
                 JOIN pg_catalog.pg_namespace n ON n.nspname = $1
                 JOIN pg_catalog.pg_class c
                   ON c.relnamespace = n.oid AND c.relname = $2 AND c.relkind IN ('r', 'p')
                 JOIN pg_catalog.pg_user u ON u.usename = current_user"
            ),
            &[&config.namespace, &"__sql_judge_external_sandbox"],
        )
        .map_err(map_external_connection_error)?
        .is_some_and(|row| {
            row.get::<_, String>(0) == config.environment_id
                && row.get::<_, String>(1) == config.profile_id
                && row.get::<_, String>(2) == config.ownership_nonce
                && row.get::<_, String>(3) == target.database
                && row.get::<_, String>(4) == target.database_oid.to_string()
                && row.get::<_, String>(5) == target.system_identifier
                && row.get::<_, bool>(6)
        });
    Ok(marker_is_owned)
}

fn validate_external_limits(limits: &QueryLimits) -> Result<(), ExternalPostgresError> {
    limits
        .validate()
        .map_err(|error| ExternalPostgresError::InvalidLimits(error.to_string()))
}

fn map_external_query_error(error: postgres::Error) -> ExternalPostgresError {
    if let Some(database_error) = error.as_db_error() {
        return match database_error.code().code() {
            "57014" => ExternalPostgresError::QueryTimeout,
            "42601" | "42602" | "42603" | "42604" | "42609" | "42610" => {
                ExternalPostgresError::QueryRejected
            }
            _ => ExternalPostgresError::DatabaseError,
        };
    }
    ExternalPostgresError::ServerUnreachable
}

fn map_external_decode_error(error: QueryExecutionError) -> ExternalPostgresError {
    match error {
        QueryExecutionError::UnsupportedType(kind) => ExternalPostgresError::UnsupportedType(kind),
        _ => ExternalPostgresError::DatabaseError,
    }
}

fn validate_external_config(config: &ExternalPostgresConfig) -> Result<(), ExternalPostgresError> {
    for (value, field) in [
        (&config.host, "host"),
        (&config.database, "database"),
        (&config.username, "username"),
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(ExternalPostgresError::InvalidProfile(field));
        }
    }
    if config.host.chars().any(char::is_whitespace)
        || config.host.contains('/')
        || config.host.contains('\\')
    {
        return Err(ExternalPostgresError::InvalidProfile("host"));
    }
    if config.port == 0 {
        return Err(ExternalPostgresError::InvalidProfile("port"));
    }
    if matches!(config.tls_mode, ExternalTlsMode::Disable)
        && (!config.allow_insecure_local || !is_loopback_host(&config.host))
    {
        return Err(ExternalPostgresError::TlsPolicyRejected);
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

#[cfg(test)]
mod tests {
    use super::{WriteCapabilitySnapshot, local_connection_config};

    #[test]
    fn local_connection_policy_parses_quoted_values_and_rejects_remote_targets() {
        #[cfg(unix)]
        assert!(
            local_connection_config(
                "host='/Users/a b/Library/Application Support/x/socket' user='a b' dbname=postgres"
            )
            .is_some()
        );
        assert!(
            local_connection_config(
                "host=127.0.0.1 port=50123 user='runner' password='it\\'s' dbname=postgres"
            )
            .is_some()
        );
        for remote in [
            "host=db.example.com user=a dbname=postgres",
            "host=127.0.0.1,db.example.com user=a dbname=postgres",
            "host='127.0.0.1 ' user=a dbname=postgres",
            "hostaddr=10.0.0.1 user=a dbname=postgres",
            "user=a dbname=postgres",
            "postgres://a@127.0.0.1/postgres",
            "host=/tmp user='unterminated dbname=postgres",
        ] {
            assert!(local_connection_config(remote).is_none(), "{remote}");
        }
    }

    #[test]
    fn read_only_requires_all_write_capabilities_to_be_false() {
        let baseline = WriteCapabilitySnapshot::default();
        assert!(baseline.read_only());

        for snapshot in [
            WriteCapabilitySnapshot {
                role: true,
                ..baseline
            },
            WriteCapabilitySnapshot {
                database: true,
                ..baseline
            },
            WriteCapabilitySnapshot {
                schema: true,
                ..baseline
            },
            WriteCapabilitySnapshot {
                table: true,
                ..baseline
            },
            WriteCapabilitySnapshot {
                sequence: true,
                ..baseline
            },
            WriteCapabilitySnapshot {
                default_acl: true,
                ..baseline
            },
        ] {
            assert!(!snapshot.read_only());
        }
    }
}

fn map_external_connection_error(error: postgres::Error) -> ExternalPostgresError {
    if let Some(database_error) = error.as_db_error() {
        return match database_error.code().code() {
            "28P01" | "28000" => ExternalPostgresError::AuthenticationFailed,
            _ => ExternalPostgresError::DatabaseError,
        };
    }
    // The driver keeps its error kind private; its TLS kind always renders with this prefix,
    // including "server does not support TLS". The text is only classified, never returned.
    if error
        .to_string()
        .starts_with("error performing TLS handshake")
    {
        return ExternalPostgresError::TlsFailed;
    }
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.downcast_ref::<native_tls::Error>().is_some() {
            return ExternalPostgresError::TlsFailed;
        }
        source = cause.source();
    }
    ExternalPostgresError::ServerUnreachable
}

pub fn validate_external_select(sql: &str) -> Result<(), ExternalPostgresError> {
    validate_read_only_sql(sql).map_err(|_| ExternalPostgresError::QueryRejected)
}

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    reason: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReason {
    User,
    Watchdog,
}

impl CancellationToken {
    pub fn cancel(&self) {
        if self
            .cancelled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.reason.store(false, Ordering::Release);
        }
    }

    pub fn cancel_for_watchdog(&self) {
        if self
            .cancelled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.reason.store(true, Ordering::Release);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn reason(&self) -> Option<CancellationReason> {
        if !self.is_cancelled() {
            return None;
        }
        Some(if self.reason.load(Ordering::Acquire) {
            CancellationReason::Watchdog
        } else {
            CancellationReason::User
        })
    }

    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled) && Arc::ptr_eq(&self.reason, &other.reason)
    }
}

struct CancellationWatcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CancellationWatcher {
    fn start(token: &CancellationToken, cancel_token: postgres::CancelToken) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let watcher_stop = Arc::clone(&stop);
        let cancellation = token.clone();
        let handle = thread::spawn(move || {
            while !watcher_stop.load(Ordering::Acquire) {
                if cancellation.is_cancelled() {
                    let _ = cancel_token.cancel_query(NoTls);
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for CancellationWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedEnvironment {
    environment_id: String,
    instance_id: String,
    namespace: String,
    ownership_nonce: String,
}

impl ManagedEnvironment {
    pub fn new(
        environment_id: impl Into<String>,
        instance_id: impl Into<String>,
        namespace: impl Into<String>,
        ownership_nonce: impl Into<String>,
    ) -> Result<Self, QueryExecutionError> {
        let environment = Self {
            environment_id: environment_id.into(),
            instance_id: instance_id.into(),
            namespace: namespace.into(),
            ownership_nonce: ownership_nonce.into(),
        };
        if environment.environment_id.trim().is_empty()
            || environment.instance_id.trim().is_empty()
            || environment.ownership_nonce.trim().is_empty()
            || !is_safe_namespace(&environment.namespace)
        {
            return Err(QueryExecutionError::PolicyViolation);
        }
        Ok(environment)
    }
}

pub struct PostgresSession {
    client: Client,
}

pub struct ManagedPostgresSession {
    session: PostgresSession,
}

impl ManagedPostgresSession {
    pub fn connect(
        connection_string: &str,
        expected_instance_id: &str,
    ) -> Result<Self, QueryExecutionError> {
        if expected_instance_id.trim().is_empty() {
            return Err(QueryExecutionError::PolicyViolation);
        }
        let mut session = PostgresSession::connect(connection_string)?;
        let marker_exists = session
            .client
            .query_one(
                "SELECT EXISTS (
                     SELECT 1
                     FROM pg_catalog.pg_class c
                     JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                     WHERE n.nspname = 'public' AND c.relname = '__sql_judge_runtime'
                       AND c.relkind IN ('r', 'p')
                 )",
                &[],
            )
            .map_err(map_database_error)?
            .get::<_, bool>(0);
        if !marker_exists {
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        let marker = session
            .client
            .query_opt(
                "SELECT r.instance_id
                 FROM public.__sql_judge_runtime r
                 JOIN pg_catalog.pg_class c ON c.relname = '__sql_judge_runtime'
                 JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                 JOIN pg_catalog.pg_user u ON u.usesysid = c.relowner
                 WHERE n.nspname = 'public' AND c.relname = '__sql_judge_runtime'
                   AND u.usename = current_user
                 LIMIT 1",
                &[],
            )
            .map_err(map_database_error)?;
        if marker.is_none_or(|row| row.get::<_, String>(0) != expected_instance_id) {
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        Ok(Self { session })
    }

    pub fn with_managed_fixture<T, F>(
        &mut self,
        runner_role: &str,
        schema_sql: &str,
        fixture_sql: &str,
        operation: F,
    ) -> Result<T, QueryExecutionError>
    where
        F: FnOnce(&mut PostgresSession) -> Result<T, QueryExecutionError>,
    {
        self.session
            .with_managed_fixture(runner_role, schema_sql, fixture_sql, operation)
    }

    pub fn execute(
        &mut self,
        sql: &str,
        limits: QueryLimits,
    ) -> Result<QueryResult, QueryExecutionError> {
        self.session.execute(sql, limits)
    }

    pub fn execute_with_cancel(
        &mut self,
        sql: &str,
        limits: QueryLimits,
        token: &CancellationToken,
    ) -> Result<QueryResult, QueryExecutionError> {
        self.session.execute_with_cancel(sql, limits, token)
    }

    pub fn provision_managed_namespace_with_schema(
        &mut self,
        environment: &ManagedEnvironment,
        runner_role: &str,
        schema_sql: &str,
    ) -> Result<(), QueryExecutionError> {
        self.session
            .provision_managed_namespace_with_schema(environment, runner_role, schema_sql)
    }
}

impl PostgresSession {
    pub fn connect(connection_string: &str) -> Result<Self, QueryExecutionError> {
        let config = local_connection_config(connection_string)
            .ok_or(QueryExecutionError::PolicyViolation)?;
        config
            .connect(NoTls)
            .map(|client| Self { client })
            .map_err(|_| QueryExecutionError::ConnectionError)
    }

    pub fn execute(
        &mut self,
        sql: &str,
        limits: QueryLimits,
    ) -> Result<QueryResult, QueryExecutionError> {
        self.execute_inner(sql, limits, None)
    }

    pub fn execute_with_cancel(
        &mut self,
        sql: &str,
        limits: QueryLimits,
        token: &CancellationToken,
    ) -> Result<QueryResult, QueryExecutionError> {
        self.execute_inner(sql, limits, Some(token))
    }

    fn execute_inner(
        &mut self,
        sql: &str,
        limits: QueryLimits,
        cancellation: Option<&CancellationToken>,
    ) -> Result<QueryResult, QueryExecutionError> {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(QueryExecutionError::RuntimeError);
        }
        limits
            .validate()
            .map_err(|error| QueryExecutionError::InvalidLimits(error.to_string()))?;
        validate_read_only_sql(sql)?;

        self.client
            .batch_execute(&format!(
                "SET statement_timeout = {}",
                limits.statement_timeout_ms
            ))
            .map_err(map_database_error)?;

        let statement = self.client.prepare(sql).map_err(map_database_error)?;
        let columns = statement
            .columns()
            .iter()
            .map(|column| {
                logical_type(column.type_())
                    .map(|logical_type| Column::new(column.name(), logical_type))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cancel_token = cancellation.map(|_| self.client.cancel_token());
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(QueryExecutionError::Cancelled);
        }
        let _watcher = match (cancellation, cancel_token) {
            (Some(token), Some(cancel_token)) => {
                Some(CancellationWatcher::start(token, cancel_token))
            }
            _ => None,
        };
        let mut rows = self
            .client
            .query_raw(&statement, std::iter::empty::<&(dyn ToSql + Sync)>())
            .map_err(|error| map_cancellable_error(error, cancellation))?;
        let mut values = Vec::with_capacity(limits.max_rows as usize);
        let mut result_bytes = 0_u32;
        let mut row_count = 0_usize;

        while let Some(row) = rows
            .next()
            .map_err(|error| map_cancellable_error(error, cancellation))?
        {
            if row_count >= limits.max_rows as usize {
                return Err(QueryExecutionError::ResultLimitExceeded);
            }
            let row_values = decode_row(&row)?;
            result_bytes = result_bytes
                .checked_add(
                    serde_json::to_vec(&row_values)
                        .map_err(|_| QueryExecutionError::RuntimeError)?
                        .len() as u32,
                )
                .ok_or(QueryExecutionError::ResultLimitExceeded)?;
            if result_bytes > limits.max_bytes {
                return Err(QueryExecutionError::ResultLimitExceeded);
            }
            values.push(row_values);
            row_count += 1;
        }

        QueryResult::new(columns, values).map_err(|_| QueryExecutionError::RuntimeError)
    }

    pub fn with_managed_fixture<T, F>(
        &mut self,
        runner_role: &str,
        schema_sql: &str,
        fixture_sql: &str,
        operation: F,
    ) -> Result<T, QueryExecutionError>
    where
        F: FnOnce(&mut Self) -> Result<T, QueryExecutionError>,
    {
        if !is_safe_role_name(runner_role) {
            return Err(QueryExecutionError::PolicyViolation);
        }
        let runner_is_constrained = self
            .client
            .query_opt(
                "SELECT NOT rolsuper AND NOT rolcreaterole AND NOT rolcreatedb
                    AND NOT rolinherit AND NOT rolbypassrls AND NOT rolreplication
                 FROM pg_catalog.pg_roles WHERE rolname = $1",
                &[&runner_role],
            )
            .map_err(map_database_error)?
            .is_some_and(|row| row.get::<_, bool>(0));
        if !runner_is_constrained {
            return Err(QueryExecutionError::PolicyViolation);
        }

        let fixture_suffix = unique_fixture_suffix();
        let fixture_schema = format!("sj_fixture_{fixture_suffix}");
        validate_setup_sql_for_namespace(schema_sql, Some(&fixture_schema))?;
        validate_setup_sql_for_namespace(fixture_sql, Some(&fixture_schema))?;
        let fixture_role = format!("sj_fixture_{fixture_suffix}");
        let schema = quote_identifier(&fixture_schema);
        let role = quote_identifier(&fixture_role);
        let runner = quote_identifier(runner_role);

        self.client
            .batch_execute("BEGIN")
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        let setup = (|| {
            self.client
                .batch_execute(&format!(
                    "REVOKE ALL ON SCHEMA public FROM PUBLIC;
                     REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
                     CREATE ROLE {role} NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;
                     CREATE SCHEMA {schema} AUTHORIZATION {role};
                     SET LOCAL search_path TO {schema}, pg_catalog;
                     SET LOCAL ROLE {role}"
                ))
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute(schema_sql)
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute(fixture_sql)
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute(&format!(
                    "RESET ROLE;
                     GRANT USAGE ON SCHEMA {schema} TO {runner};
                     GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO {runner};
                     SET LOCAL ROLE {runner}"
                ))
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            operation(self)
        })();
        let rollback = self.client.batch_execute("ROLLBACK");

        if rollback.is_err() {
            Err(QueryExecutionError::RuntimeError)
        } else {
            setup
        }
    }

    /// Loads one fixture into an owned managed namespace inside a transaction that is always
    /// rolled back, then runs `operation` as the constrained runner with the transaction
    /// switched to read-only (ADR-026). The namespace's committed state never changes.
    pub fn with_namespace_fixture<T, F>(
        &mut self,
        environment: &ManagedEnvironment,
        runner_role: &str,
        fixture_sql: &str,
        operation: F,
    ) -> Result<T, QueryExecutionError>
    where
        F: FnOnce(&mut Self) -> Result<T, QueryExecutionError>,
    {
        if !is_safe_role_name(runner_role) {
            return Err(QueryExecutionError::PolicyViolation);
        }
        validate_setup_sql_for_namespace(fixture_sql, Some(&environment.namespace))?;
        if !self.managed_namespace_is_owned(environment)? {
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        let runner_is_constrained = self
            .client
            .query_opt(
                "SELECT NOT rolsuper AND NOT rolcreaterole AND NOT rolcreatedb
                    AND NOT rolinherit AND NOT rolbypassrls AND NOT rolreplication
                 FROM pg_catalog.pg_roles WHERE rolname = $1",
                &[&runner_role],
            )
            .map_err(map_database_error)?
            .is_some_and(|row| row.get::<_, bool>(0));
        if !runner_is_constrained {
            return Err(QueryExecutionError::PolicyViolation);
        }
        let schema = quote_identifier(&environment.namespace);
        let runner = quote_identifier(runner_role);

        self.client
            .batch_execute("BEGIN")
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        let result = (|| {
            // Provisioning ran the pack schema as the runner, so the runner owns these tables.
            self.client
                .batch_execute(&format!(
                    "SET LOCAL search_path TO {schema}, pg_catalog;
                     SET LOCAL ROLE {runner}"
                ))
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute(fixture_sql)
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute("SET LOCAL transaction_read_only = on")
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            operation(self)
        })();
        let rollback = self.client.batch_execute("ROLLBACK");

        if rollback.is_err() {
            Err(QueryExecutionError::RuntimeError)
        } else {
            result
        }
    }

    pub fn provision_managed_namespace(
        &mut self,
        environment: &ManagedEnvironment,
        runner_role: &str,
    ) -> Result<(), QueryExecutionError> {
        if !is_safe_role_name(runner_role) {
            return Err(QueryExecutionError::PolicyViolation);
        }

        let schema = quote_identifier(&environment.namespace);
        let marker = format!("{schema}.\"__sql_judge_ownership\"");
        let role = quote_identifier(runner_role);
        let mut transaction = self
            .client
            .transaction()
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        let result = (|| {
            let runner_is_constrained = transaction
                .query_opt(
                    "SELECT NOT rolsuper AND NOT rolcreaterole AND NOT rolcreatedb
                        AND NOT rolinherit AND NOT rolbypassrls AND NOT rolreplication
                     FROM pg_catalog.pg_roles WHERE rolname = $1",
                    &[&runner_role],
                )
                .map_err(map_database_error)?
                .is_some_and(|row| row.get::<_, bool>(0));
            if !runner_is_constrained {
                return Err(QueryExecutionError::PolicyViolation);
            }
            let schema_exists = transaction
                .query_one(
                    "SELECT EXISTS (
                         SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = $1
                     )",
                    &[&environment.namespace],
                )
                .map_err(map_database_error)?
                .get::<_, bool>(0);
            let marker_exists = transaction
                .query_one(
                    "SELECT to_regclass($1) IS NOT NULL",
                    &[&format!(
                        "{}.{}",
                        environment.namespace, "__sql_judge_ownership"
                    )],
                )
                .map_err(map_database_error)?
                .get::<_, bool>(0);
            if schema_exists && !marker_exists {
                return Err(QueryExecutionError::OwnershipVerificationFailed);
            }
            if schema_exists {
                let marker_owned = transaction
                    .query_one(
                        "SELECT n.nspowner = u.usesysid AND c.relowner = u.usesysid
                         FROM pg_catalog.pg_namespace n
                         JOIN pg_catalog.pg_class c ON c.relnamespace = n.oid
                         JOIN pg_catalog.pg_user u ON u.usename = current_user
                         WHERE n.nspname = $1 AND c.relname = $2
                           AND c.relkind IN ('r', 'p')",
                        &[&environment.namespace, &"__sql_judge_ownership"],
                    )
                    .map_err(map_database_error)?
                    .get::<_, bool>(0);
                if !marker_owned {
                    return Err(QueryExecutionError::OwnershipVerificationFailed);
                }
            }
            transaction
                .batch_execute(&format!(
                    "CREATE SCHEMA IF NOT EXISTS {schema};
                     REVOKE ALL ON SCHEMA {schema} FROM PUBLIC;
                     CREATE TABLE IF NOT EXISTS {marker} (
                         marker_key text PRIMARY KEY CHECK (marker_key = 'environment'),
                         environment_id text NOT NULL,
                         instance_id text NOT NULL,
                         ownership_nonce text NOT NULL
                     )"
                ))
                .map_err(map_database_error)?;

            let existing = transaction
                .query_opt(
                    &format!(
                        "SELECT environment_id, instance_id, ownership_nonce FROM {marker} WHERE marker_key = 'environment'"
                    ),
                    &[],
                )
                .map_err(map_database_error)?;
            if let Some(row) = existing {
                let matches = row.get::<_, String>(0) == environment.environment_id
                    && row.get::<_, String>(1) == environment.instance_id
                    && row.get::<_, String>(2) == environment.ownership_nonce;
                if !matches {
                    return Err(QueryExecutionError::OwnershipVerificationFailed);
                }
            } else {
                transaction
                    .execute(
                        &format!(
                            "INSERT INTO {marker} (marker_key, environment_id, instance_id, ownership_nonce)
                             VALUES ('environment', $1, $2, $3)"
                        ),
                        &[
                            &environment.environment_id,
                            &environment.instance_id,
                            &environment.ownership_nonce,
                        ],
                    )
                    .map_err(map_database_error)?;
            }

            transaction
                .batch_execute(&format!(
                    "GRANT USAGE ON SCHEMA {schema} TO {role};
                     GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO {role};
                     REVOKE ALL ON TABLE {marker} FROM {role}, PUBLIC"
                ))
                .map_err(map_database_error)?;
            Ok(())
        })();

        match result {
            Ok(()) => transaction
                .commit()
                .map_err(|_| QueryExecutionError::RuntimeError),
            Err(error) => {
                let _ = transaction.rollback();
                Err(error)
            }
        }
    }

    pub fn provision_managed_namespace_with_schema(
        &mut self,
        environment: &ManagedEnvironment,
        runner_role: &str,
        schema_sql: &str,
    ) -> Result<(), QueryExecutionError> {
        validate_setup_sql_for_namespace(schema_sql, Some(&environment.namespace))?;
        let namespace_existed = self.managed_namespace_exists(environment)?;
        self.provision_managed_namespace(environment, runner_role)?;

        let schema = quote_identifier(&environment.namespace);
        let marker = format!("{schema}.\"__sql_judge_ownership\"");
        let role = quote_identifier(runner_role);
        self.client
            .batch_execute("BEGIN")
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        let setup = (|| {
            self.client
                .batch_execute(&format!(
                    "GRANT USAGE, CREATE ON SCHEMA {schema} TO {role};
                     SET LOCAL search_path TO {schema}, pg_catalog;
                     SET LOCAL ROLE {role};
                     {schema_sql}"
                ))
                .map_err(|_| QueryExecutionError::RuntimeError)?;
            self.client
                .batch_execute(&format!(
                    "RESET ROLE;
                     REVOKE CREATE ON SCHEMA {schema} FROM {role};
                     GRANT USAGE ON SCHEMA {schema} TO {role};
                     GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO {role};
                     REVOKE ALL ON TABLE {marker} FROM {role}, PUBLIC"
                ))
                .map_err(|_| QueryExecutionError::RuntimeError)
        })();
        let transaction = if setup.is_ok() {
            self.client.batch_execute("COMMIT")
        } else {
            self.client.batch_execute("ROLLBACK")
        };
        match (setup, transaction.is_ok()) {
            (Ok(()), true) => Ok(()),
            (setup, _) => {
                let setup_error = setup.err().unwrap_or(QueryExecutionError::RuntimeError);
                let revoke = self
                    .client
                    .batch_execute(&format!("REVOKE CREATE ON SCHEMA {schema} FROM {role}"));
                if revoke.is_err() {
                    return Err(QueryExecutionError::RuntimeError);
                }
                if namespace_existed {
                    return Err(setup_error);
                }
                match self.drop_managed_namespace(environment) {
                    Ok(()) => Err(setup_error),
                    Err(_) => Err(QueryExecutionError::RuntimeError),
                }
            }
        }
    }

    pub fn managed_namespace_exists(
        &mut self,
        environment: &ManagedEnvironment,
    ) -> Result<bool, QueryExecutionError> {
        self.client
            .query_one(
                "SELECT EXISTS (
                     SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = $1
                 )",
                &[&environment.namespace],
            )
            .map(|row| row.get::<_, bool>(0))
            .map_err(map_database_error)
    }

    pub fn managed_namespace_is_owned(
        &mut self,
        environment: &ManagedEnvironment,
    ) -> Result<bool, QueryExecutionError> {
        let marker_exists = self
            .client
            .query_one(
                "SELECT EXISTS (
                     SELECT 1 FROM pg_catalog.pg_namespace n
                     JOIN pg_catalog.pg_class c ON c.relnamespace = n.oid
                     WHERE n.nspname = $1 AND c.relname = $2
                       AND c.relkind IN ('r', 'p')
                 )",
                &[&environment.namespace, &"__sql_judge_ownership"],
            )
            .map_err(map_database_error)?
            .get::<_, bool>(0);
        if !marker_exists {
            return Ok(false);
        }
        let owned_marker = self
            .client
            .query_opt(
                &format!(
                    "SELECT m.environment_id, m.instance_id, m.ownership_nonce,
                            n.nspowner = u.usesysid AND c.relowner = u.usesysid
                     FROM {schema}.\"__sql_judge_ownership\" m
                     JOIN pg_catalog.pg_namespace n ON n.nspname = $1
                     JOIN pg_catalog.pg_class c
                       ON c.relnamespace = n.oid AND c.relname = $2
                     JOIN pg_catalog.pg_user u ON u.usename = current_user",
                    schema = quote_identifier(&environment.namespace),
                ),
                &[&environment.namespace, &"__sql_judge_ownership"],
            )
            .map_err(map_database_error)?
            .is_some_and(|row| {
                row.get::<_, String>(0) == environment.environment_id
                    && row.get::<_, String>(1) == environment.instance_id
                    && row.get::<_, String>(2) == environment.ownership_nonce
                    && row.get::<_, bool>(3)
            });
        if !owned_marker {
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        Ok(true)
    }

    pub fn managed_namespace_has_schema(
        &mut self,
        environment: &ManagedEnvironment,
    ) -> Result<bool, QueryExecutionError> {
        if !self.managed_namespace_is_owned(environment)? {
            return Ok(false);
        }
        self.client
            .query_one(
                "SELECT EXISTS (
                     SELECT 1
                     FROM pg_catalog.pg_class c
                     JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                     WHERE n.nspname = $1
                       AND c.relname <> $2
                       AND c.relkind IN ('r', 'p', 'v', 'm')
                 )",
                &[&environment.namespace, &"__sql_judge_ownership"],
            )
            .map(|row| row.get::<_, bool>(0))
            .map_err(map_database_error)
    }

    pub fn drop_managed_namespace(
        &mut self,
        environment: &ManagedEnvironment,
    ) -> Result<(), QueryExecutionError> {
        let schema = quote_identifier(&environment.namespace);
        let marker = format!("{schema}.\"__sql_judge_ownership\"");
        let mut transaction = self
            .client
            .transaction()
            .map_err(|_| QueryExecutionError::RuntimeError)?;
        let marker_exists = transaction
            .query_one(
                "SELECT EXISTS (
                     SELECT 1 FROM pg_catalog.pg_namespace n
                     JOIN pg_catalog.pg_class c ON c.relnamespace = n.oid
                     WHERE n.nspname = $1 AND c.relname = $2
                       AND c.relkind IN ('r', 'p')
                 )",
                &[&environment.namespace, &"__sql_judge_ownership"],
            )
            .map_err(map_database_error)?
            .get::<_, bool>(0);
        if !marker_exists {
            let _ = transaction.rollback();
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        let owned_marker = transaction
            .query_opt(
                &format!(
                    "SELECT m.environment_id, m.instance_id, m.ownership_nonce,
                            n.nspowner = u.usesysid AND c.relowner = u.usesysid
                     FROM {marker} m
                     JOIN pg_catalog.pg_namespace n ON n.nspname = $1
                     JOIN pg_catalog.pg_class c
                       ON c.relnamespace = n.oid AND c.relname = $2
                     JOIN pg_catalog.pg_user u ON u.usename = current_user"
                ),
                &[&environment.namespace, &"__sql_judge_ownership"],
            )
            .map_err(map_database_error)?
            .is_some_and(|row| {
                row.get::<_, String>(0) == environment.environment_id
                    && row.get::<_, String>(1) == environment.instance_id
                    && row.get::<_, String>(2) == environment.ownership_nonce
                    && row.get::<_, bool>(3)
            });
        if !owned_marker {
            let _ = transaction.rollback();
            return Err(QueryExecutionError::OwnershipVerificationFailed);
        }
        transaction
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .map_err(map_database_error)?;
        transaction
            .commit()
            .map_err(|_| QueryExecutionError::RuntimeError)
    }
}

fn is_safe_role_name(role: &str) -> bool {
    !role.is_empty()
        && role
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character == '_')
        && role.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Parses with the driver itself so quoted values (macOS "Application Support" socket paths)
/// are checked exactly as they will be connected.
fn local_connection_config(connection_string: &str) -> Option<Config> {
    if connection_string.starts_with("postgres://")
        || connection_string.starts_with("postgresql://")
    {
        return None;
    }
    let config: Config = connection_string.parse().ok()?;
    let hosts_are_local = config.get_hosts().iter().all(|host| match host {
        Host::Tcp(name) => matches!(name.as_str(), "127.0.0.1" | "::1" | "localhost"),
        #[cfg(unix)]
        Host::Unix(_) => true,
    });
    let addresses_are_local = config
        .get_hostaddrs()
        .iter()
        .all(|address| address.is_loopback());
    let has_target = !config.get_hosts().is_empty() || !config.get_hostaddrs().is_empty();
    (has_target && hosts_are_local && addresses_are_local).then_some(config)
}

fn unique_fixture_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}_{}", std::process::id(), timestamp)
}

fn is_safe_namespace(namespace: &str) -> bool {
    namespace.len() <= 63
        && namespace.starts_with("sj_")
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

pub fn validate_read_only_sql(sql: &str) -> Result<(), QueryExecutionError> {
    let normalized = strip_literals_comments_and_check_semicolons(sql)?;
    let tokens: Vec<&str> = normalized
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .collect();

    const FORBIDDEN: &[&str] = &[
        "insert",
        "update",
        "delete",
        "drop",
        "alter",
        "create",
        "truncate",
        "grant",
        "revoke",
        "copy",
        "call",
        "execute",
        "do",
        "listen",
        "notify",
        "vacuum",
        "analyze",
        "pg_sleep",
        "pg_sleep_for",
        "dblink",
        "lo_import",
        "lo_export",
        "nextval",
        "set_config",
        "pg_read_file",
        "pg_ls_dir",
    ];
    if !matches!(tokens.first(), Some(&"select" | &"with")) {
        if tokens
            .first()
            .is_some_and(|token| FORBIDDEN.contains(token))
        {
            return Err(QueryExecutionError::PolicyViolation);
        }
        return Err(QueryExecutionError::SyntaxError);
    }
    if tokens.iter().any(|token| FORBIDDEN.contains(token)) {
        return Err(QueryExecutionError::PolicyViolation);
    }

    Ok(())
}

pub fn validate_setup_sql(sql: &str) -> Result<(), QueryExecutionError> {
    validate_setup_sql_for_namespace(sql, None)
}

pub fn validate_setup_sql_for_namespace(
    sql: &str,
    namespace: Option<&str>,
) -> Result<(), QueryExecutionError> {
    let namespace = namespace.map(str::to_ascii_lowercase);
    const FORBIDDEN: &[&str] = &[
        "alter",
        "analyze",
        "call",
        "copy",
        "database",
        "dblink",
        "delete",
        "do",
        "drop",
        "execute",
        "function",
        "grant",
        "listen",
        "nextval",
        "notify",
        "pg_ls_dir",
        "pg_read_file",
        "pg_sleep",
        "pg_sleep_for",
        "public",
        "revoke",
        "role",
        "schema",
        "sequence",
        "select",
        "set",
        "trigger",
        "truncate",
        "update",
        "vacuum",
    ];
    for statement in split_setup_statements(sql)? {
        let tokens: Vec<&str> = statement
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.is_empty()
            || tokens.iter().any(|token| {
                FORBIDDEN.contains(token)
                    || (token.starts_with("pg_") && !is_allowed_pg_catalog_token(&statement, token))
                    || token.starts_with("lo_")
            })
            || has_unsafe_qualified_identifier(&statement, namespace.as_deref())
            || !matches!(
                tokens.as_slice(),
                ["create", "table", ..] | ["create", "index", ..] | ["insert", "into", ..]
            )
        {
            return Err(QueryExecutionError::PolicyViolation);
        }
    }
    Ok(())
}

fn has_unsafe_qualified_identifier(statement: &str, namespace: Option<&str>) -> bool {
    statement
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
        })
        .filter(|token| token.contains('.'))
        .any(|token| {
            let parts: Vec<_> = token.split('.').collect();
            if parts.iter().any(|part| part.is_empty()) || parts.len() < 2 {
                return true;
            }
            if parts
                .iter()
                .all(|part| part.chars().all(|character| character.is_ascii_digit()))
            {
                return false;
            }
            parts.len() != 2
                || !matches!(parts[0], "pg_catalog")
                    && namespace.is_none_or(|allowed| parts[0] != allowed)
        })
}

fn is_allowed_pg_catalog_token(statement: &str, token: &str) -> bool {
    token == "pg_catalog" || statement.contains(&format!("pg_catalog.{token}"))
}

fn split_setup_statements(sql: &str) -> Result<Vec<String>, QueryExecutionError> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut characters = sql.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(character) = characters.next() {
        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
            }
            current.push(' ');
            continue;
        }
        if in_block_comment {
            if character == '*' && characters.peek() == Some(&'/') {
                characters.next();
                in_block_comment = false;
            }
            current.push(' ');
            continue;
        }
        if in_single_quote {
            if character == '\'' {
                if characters.peek() == Some(&'\'') {
                    characters.next();
                } else {
                    in_single_quote = false;
                }
            }
            current.push(' ');
            continue;
        }
        if in_double_quote {
            if character == '"' {
                in_double_quote = false;
            } else {
                current.push(character.to_ascii_lowercase());
            }
            continue;
        }

        match character {
            '\'' => {
                in_single_quote = true;
                current.push(' ');
            }
            '"' => {
                in_double_quote = true;
                current.push(' ');
            }
            '-' if characters.peek() == Some(&'-') => {
                characters.next();
                in_line_comment = true;
                current.push(' ');
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                in_block_comment = true;
                current.push(' ');
            }
            ';' => {
                if !current.trim().is_empty() {
                    statements.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other.to_ascii_lowercase()),
        }
    }

    if in_single_quote || in_double_quote || in_block_comment {
        return Err(QueryExecutionError::SyntaxError);
    }
    if !current.trim().is_empty() {
        statements.push(current);
    }
    Ok(statements)
}

fn strip_literals_comments_and_check_semicolons(sql: &str) -> Result<String, QueryExecutionError> {
    let trimmed = sql.trim();
    let source = trimmed.strip_suffix(';').unwrap_or(trimmed);
    let mut normalized = String::with_capacity(source.len());
    let mut characters = source.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(character) = characters.next() {
        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
                normalized.push(' ');
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && characters.peek() == Some(&'/') {
                characters.next();
                in_block_comment = false;
                normalized.push(' ');
            }
            continue;
        }
        if in_single_quote {
            if character == '\'' {
                if characters.peek() == Some(&'\'') {
                    characters.next();
                } else {
                    in_single_quote = false;
                }
            }
            normalized.push(' ');
            continue;
        }
        if in_double_quote {
            if character == '"' {
                in_double_quote = false;
            } else {
                normalized.push(character.to_ascii_lowercase());
            }
            continue;
        }

        match character {
            '\'' => {
                in_single_quote = true;
                normalized.push(' ');
            }
            '"' => {
                in_double_quote = true;
                normalized.push(' ');
            }
            '-' if characters.peek() == Some(&'-') => {
                characters.next();
                in_line_comment = true;
                normalized.push(' ');
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                in_block_comment = true;
                normalized.push(' ');
            }
            ';' => return Err(QueryExecutionError::PolicyViolation),
            other => normalized.push(other.to_ascii_lowercase()),
        }
    }

    if in_single_quote || in_double_quote || in_block_comment {
        return Err(QueryExecutionError::SyntaxError);
    }

    Ok(normalized)
}

fn logical_type(postgres_type: &Type) -> Result<LogicalType, QueryExecutionError> {
    match *postgres_type {
        Type::INT2 | Type::INT4 => Ok(LogicalType::Integer),
        Type::INT8 => Ok(LogicalType::Bigint),
        Type::NUMERIC => Ok(LogicalType::Numeric),
        Type::FLOAT4 | Type::FLOAT8 => Ok(LogicalType::Float),
        Type::BOOL => Ok(LogicalType::Boolean),
        Type::TEXT | Type::VARCHAR | Type::CHAR | Type::NAME => Ok(LogicalType::Text),
        Type::BYTEA => Ok(LogicalType::Binary),
        Type::DATE => Ok(LogicalType::Date),
        Type::TIMESTAMP => Ok(LogicalType::Timestamp),
        Type::TIMESTAMPTZ => Ok(LogicalType::Timestamptz),
        Type::UUID => Ok(LogicalType::Uuid),
        Type::JSON | Type::JSONB => Ok(LogicalType::Json),
        _ => Err(QueryExecutionError::UnsupportedType(
            postgres_type.name().to_owned(),
        )),
    }
}

struct NumericText(String);

impl<'a> FromSql<'a> for NumericText {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn StdError + Sync + Send>> {
        if raw.len() < 8 {
            return Err("invalid PostgreSQL numeric value".into());
        }

        let read_i16 = |offset: usize| -> Result<i16, Box<dyn StdError + Sync + Send>> {
            let bytes = raw
                .get(offset..offset + 2)
                .ok_or("invalid PostgreSQL numeric value")?;
            Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
        };
        let ndigits =
            usize::try_from(read_i16(0)?).map_err(|_| "invalid PostgreSQL numeric value")?;
        let weight = read_i16(2)? as i32;
        let sign = read_i16(4)? as u16;
        let dscale =
            usize::try_from(read_i16(6)?).map_err(|_| "invalid PostgreSQL numeric value")?;
        let expected_len = 8usize
            .checked_add(
                ndigits
                    .checked_mul(2)
                    .ok_or("invalid PostgreSQL numeric value")?,
            )
            .ok_or("invalid PostgreSQL numeric value")?;
        if raw.len() != expected_len {
            return Err("invalid PostgreSQL numeric value".into());
        }
        if sign == 0xC000 {
            return Ok(Self("NaN".to_owned()));
        }
        if sign != 0x0000 && sign != 0x4000 {
            return Err("invalid PostgreSQL numeric sign".into());
        }

        let mut digits = String::new();
        for index in 0..ndigits {
            let offset = 8 + index * 2;
            let digit = u16::from_be_bytes([raw[offset], raw[offset + 1]]);
            if digit >= 10_000 {
                return Err("invalid PostgreSQL numeric digit".into());
            }
            digits.push_str(&format!("{digit:04}"));
        }

        let decimal_index = (weight + 1) * 4;
        let (mut integer, mut fraction) = if decimal_index <= 0 {
            (
                "0".to_owned(),
                format!("{}{}", "0".repeat((-decimal_index) as usize), digits),
            )
        } else if decimal_index as usize >= digits.len() {
            (
                format!(
                    "{}{}",
                    digits,
                    "0".repeat(decimal_index as usize - digits.len())
                ),
                String::new(),
            )
        } else {
            let split = decimal_index as usize;
            (digits[..split].to_owned(), digits[split..].to_owned())
        };

        integer = integer.trim_start_matches('0').to_owned();
        if integer.is_empty() {
            integer.push('0');
        }
        fraction.truncate(dscale);
        if fraction.len() < dscale {
            fraction.push_str(&"0".repeat(dscale - fraction.len()));
        }
        let is_zero = integer == "0" && fraction.chars().all(|character| character == '0');
        let value = if dscale == 0 {
            integer
        } else {
            format!("{integer}.{fraction}")
        };
        Ok(Self(if sign == 0x4000 && !is_zero {
            format!("-{value}")
        } else {
            value
        }))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

fn decode_row(row: &Row) -> Result<Vec<SqlValue>, QueryExecutionError> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| match *column.type_() {
            Type::BOOL => row
                .try_get::<_, Option<bool>>(index)
                .map(|value| value.map_or(SqlValue::Null, SqlValue::Bool))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::INT2 => row
                .try_get::<_, Option<i16>>(index)
                .map(|value| value.map_or(SqlValue::Null, |value| SqlValue::Signed(value as i64)))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::INT4 => row
                .try_get::<_, Option<i32>>(index)
                .map(|value| value.map_or(SqlValue::Null, |value| SqlValue::Signed(value as i64)))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::INT8 => row
                .try_get::<_, Option<i64>>(index)
                .map(|value| value.map_or(SqlValue::Null, SqlValue::Signed))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::NUMERIC => row
                .try_get::<_, Option<NumericText>>(index)
                .map(|value| value.map_or(SqlValue::Null, |value| SqlValue::Decimal(value.0)))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::FLOAT4 => row
                .try_get::<_, Option<f32>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| SqlValue::Float {
                        bits: value.to_bits() as u64,
                    })
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::FLOAT8 => row
                .try_get::<_, Option<f64>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| SqlValue::Float {
                        bits: value.to_bits(),
                    })
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::TEXT | Type::VARCHAR | Type::CHAR | Type::NAME => row
                .try_get::<_, Option<String>>(index)
                .map(|value| value.map_or(SqlValue::Null, SqlValue::Text))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::BYTEA => row
                .try_get::<_, Option<Vec<u8>>>(index)
                .map(|value| value.map_or(SqlValue::Null, SqlValue::Bytes))
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::DATE => row
                .try_get::<_, Option<chrono::NaiveDate>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| SqlValue::Date(value.to_string()))
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::TIMESTAMP => row
                .try_get::<_, Option<chrono::NaiveDateTime>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| {
                        SqlValue::Timestamp(value.to_string())
                    })
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::TIMESTAMPTZ => row
                .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| {
                        SqlValue::Timestamptz(value.to_rfc3339())
                    })
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::UUID => row
                .try_get::<_, Option<uuid::Uuid>>(index)
                .map(|value| {
                    value.map_or(SqlValue::Null, |value| SqlValue::Uuid(value.to_string()))
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            Type::JSON | Type::JSONB => row
                .try_get::<_, Option<serde_json::Value>>(index)
                .map_err(|_| QueryExecutionError::RuntimeError)
                .and_then(|value| {
                    value.map_or(Ok(SqlValue::Null), |value| {
                        serde_json::to_string(&value)
                            .map(SqlValue::Json)
                            .map_err(|_| QueryExecutionError::RuntimeError)
                    })
                })
                .map_err(|_| QueryExecutionError::RuntimeError),
            _ => Err(QueryExecutionError::UnsupportedType(
                column.type_().name().to_owned(),
            )),
        })
        .collect()
}

fn map_database_error(error: postgres::Error) -> QueryExecutionError {
    if let Some(database_error) = error.as_db_error() {
        match database_error.code().code() {
            "57014" => return QueryExecutionError::TimeLimitExceeded,
            "42601" | "42602" | "42603" | "42604" | "42609" | "42610" => {
                return QueryExecutionError::SyntaxError;
            }
            _ => {}
        }
    }
    QueryExecutionError::RuntimeError
}

fn map_cancellable_error(
    error: postgres::Error,
    cancellation: Option<&CancellationToken>,
) -> QueryExecutionError {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        QueryExecutionError::Cancelled
    } else {
        map_database_error(error)
    }
}
