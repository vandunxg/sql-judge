use judge_postgres::{
    ExternalPostgresConfig, ExternalPostgresError, ExternalPostgresExplorer, ExternalTlsMode,
    PostgresSession, QueryExecutionError, validate_external_select, validate_read_only_sql,
    validate_setup_sql, validate_setup_sql_for_namespace,
};

#[test]
fn policy_rejects_multiple_statements_and_writes() {
    for sql in [
        "SELECT 1; SELECT 2",
        "INSERT INTO users VALUES (1)",
        "WITH changed AS (DELETE FROM users RETURNING id) SELECT * FROM changed",
        "SELECT pg_sleep(1)",
        "SELECT \"pg_catalog\".\"pg_sleep\"(1)",
    ] {
        assert_eq!(
            validate_read_only_sql(sql),
            Err(QueryExecutionError::PolicyViolation)
        );
    }
}

#[test]
fn policy_accepts_select_and_read_only_with_queries() {
    assert_eq!(validate_read_only_sql("SELECT 1"), Ok(()));
    assert_eq!(validate_read_only_sql("SELECT 1;"), Ok(()));
    assert_eq!(
        validate_read_only_sql("WITH rows AS (SELECT 1 AS value) SELECT value FROM rows"),
        Ok(())
    );
}

#[test]
fn setup_policy_rejects_privileged_or_mutating_admin_sql() {
    for sql in [
        "CREATE ROLE attacker LOGIN PASSWORD 'secret'",
        "DROP TABLE users",
        "CREATE TABLE public.users (id integer)",
        "GRANT ALL ON SCHEMA public TO attacker",
        "DO $$ BEGIN PERFORM pg_sleep(1); END $$",
    ] {
        assert_eq!(
            validate_setup_sql(sql),
            Err(QueryExecutionError::PolicyViolation)
        );
    }
    assert_eq!(
        validate_setup_sql(
            "CREATE TABLE employees (id integer); INSERT INTO employees VALUES (1);"
        ),
        Ok(())
    );
}

#[test]
fn setup_policy_accepts_insert_select_for_self_contained_fixtures() {
    assert_eq!(
        validate_setup_sql(
            "CREATE TABLE source_rows (id integer); CREATE TABLE target_rows (id integer); \
             INSERT INTO target_rows SELECT id FROM source_rows"
        ),
        Ok(())
    );
}

#[test]
fn setup_policy_accepts_insert_on_conflict_do_nothing() {
    assert_eq!(
        validate_setup_sql(
            "CREATE TABLE target_rows (id integer PRIMARY KEY); \
             INSERT INTO target_rows VALUES (1) ON CONFLICT (id) DO NOTHING"
        ),
        Ok(())
    );
}

#[test]
fn setup_policy_rejects_cross_namespace_and_database_qualified_pack_sql() {
    for sql in [
        "CREATE TABLE sj_other.evil (id integer)",
        "CREATE TABLE other_db.public.evil (id integer)",
        "CREATE INDEX evil_idx ON other_schema.users (id)",
    ] {
        assert_eq!(
            validate_setup_sql_for_namespace(sql, Some("sj_current")),
            Err(QueryExecutionError::PolicyViolation)
        );
    }
    assert_eq!(
        validate_setup_sql_for_namespace("CREATE TABLE users (id integer)", Some("sj_current")),
        Ok(())
    );
    assert_eq!(
        validate_setup_sql_for_namespace(
            "CREATE TABLE sj_current.users (id integer); CREATE INDEX users_id ON pg_catalog.pg_class (oid)",
            Some("sj_current")
        ),
        Ok(())
    );
}

#[test]
fn managed_session_rejects_non_loopback_connection_targets() {
    assert!(matches!(
        PostgresSession::connect("host=database.example user=runner dbname=practice"),
        Err(QueryExecutionError::PolicyViolation)
    ));
}

fn external_config(host: &str, tls_mode: ExternalTlsMode) -> ExternalPostgresConfig {
    ExternalPostgresConfig {
        host: host.to_owned(),
        port: 5432,
        database: "practice".to_owned(),
        username: "reader".to_owned(),
        password: "fake-secret".to_owned(),
        tls_mode,
        allow_insecure_local: false,
    }
}

#[test]
fn external_profile_rejects_plaintext_remote_connections() {
    assert!(matches!(
        ExternalPostgresExplorer::connect(external_config(
            "database.example.test",
            ExternalTlsMode::Disable,
        )),
        Err(ExternalPostgresError::TlsPolicyRejected)
    ));
}

#[test]
fn external_profile_rejects_ambiguous_host_values() {
    assert!(matches!(
        ExternalPostgresExplorer::connect(external_config(
            "database example.test",
            ExternalTlsMode::VerifyFull,
        )),
        Err(ExternalPostgresError::InvalidProfile("host"))
    ));
}

#[test]
fn external_plaintext_is_allowed_only_for_explicit_loopback_opt_in() {
    let mut config = external_config("127.0.0.1", ExternalTlsMode::Disable);
    config.allow_insecure_local = true;
    assert!(!matches!(
        ExternalPostgresExplorer::connect(config),
        Err(ExternalPostgresError::TlsPolicyRejected)
    ));
}

#[test]
fn external_select_policy_rejects_writes_and_multiple_statements() {
    for sql in [
        "INSERT INTO users VALUES (1)",
        "UPDATE users SET name = 'changed'",
        "DROP TABLE users",
        "SELECT 1; SELECT 2",
        "SELECT pg_sleep(1)",
    ] {
        assert_eq!(
            validate_external_select(sql),
            Err(ExternalPostgresError::QueryRejected)
        );
    }
}
