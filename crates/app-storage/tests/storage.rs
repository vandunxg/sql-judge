use app_storage::{
    BackupSnapshot, ConnectionProfile, EnvironmentStore, HistoryStore, RecoveryOperation,
    RecoveryRecord, StorageError, SubmissionRecord, SubmissionSummary, TestRecord, export_backup,
    restore_backup,
};
use environment_manager::{
    EnvironmentMode, EnvironmentRecord, EnvironmentState, ExternalSandboxTarget,
};
use rusqlite::Connection;

fn record(id: &str) -> SubmissionRecord {
    SubmissionRecord {
        submission_id: id.to_owned(),
        pack_id: "hr-sql-101".to_owned(),
        pack_version: "1.0.0".to_owned(),
        pack_hash: "sha256:test".to_owned(),
        engine_version: "postgresql-18".to_owned(),
        policy_version: "policy-v1".to_owned(),
        verdict: "Accepted".to_owned(),
        created_at: "2026-10-12T00:00:00Z".to_owned(),
        problem_id: Some("above-average".to_owned()),
        mode: "submit".to_owned(),
    }
}

fn run_record(id: &str) -> SubmissionRecord {
    SubmissionRecord {
        verdict: "WrongAnswer".to_owned(),
        created_at: "2026-10-13T00:00:00Z".to_owned(),
        mode: "run".to_owned(),
        ..record(id)
    }
}

fn temp_database(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("sql-judge-{name}-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn history_columns(path: &std::path::Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open history database");
    let mut statement = connection
        .prepare("PRAGMA table_info(submissions)")
        .expect("inspect submissions table");
    statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("read submissions columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("decode submissions columns")
}

const LEGACY_HISTORY_DDL: &str = "
    CREATE TABLE schema_version (
        version INTEGER PRIMARY KEY CHECK (version = 1)
    );
    INSERT INTO schema_version(version) VALUES (1);
    CREATE TABLE submissions (
        submission_id TEXT PRIMARY KEY NOT NULL,
        pack_id TEXT NOT NULL,
        pack_version TEXT NOT NULL,
        pack_hash TEXT NOT NULL,
        engine_version TEXT NOT NULL,
        policy_version TEXT NOT NULL,
        verdict TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE submission_tests (
        submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
        test_id TEXT NOT NULL,
        verdict TEXT NOT NULL,
        elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
        PRIMARY KEY (submission_id, test_id)
    );
    CREATE INDEX idx_submissions_created_at ON submissions(created_at);
    INSERT INTO submissions VALUES (
        'legacy-1', 'hr-sql-101', '1.0.0', 'sha256:legacy', 'postgresql-managed',
        'policy-v1', 'WrongAnswer', '1760000000'
    );
    INSERT INTO submission_tests VALUES ('legacy-1', 'basic', 'WrongAnswer', 7);";

fn test_record(id: &str) -> TestRecord {
    TestRecord {
        test_id: id.to_owned(),
        verdict: "Accepted".to_owned(),
        elapsed_ms: 12,
    }
}

fn environment(id: &str, state: EnvironmentState) -> EnvironmentRecord {
    EnvironmentRecord {
        env_id: id.to_owned(),
        instance_id: format!("instance-{id}"),
        pack_id: "hr-sql-101".to_owned(),
        pack_version: "1.0.0".to_owned(),
        pack_digest: "a".repeat(64),
        namespace: format!("sj_{id}").replace('-', "_"),
        ownership_nonce: format!("nonce-{id}"),
        mode: EnvironmentMode::Managed,
        state,
    }
}

#[test]
fn external_sandbox_target_round_trips_through_ledger_and_backup_without_credentials() {
    let path = temp_database("external-sandbox-target");
    let sandbox = EnvironmentRecord {
        env_id: "external-sandbox".to_owned(),
        instance_id: "external-instance".to_owned(),
        pack_id: "hr-sql-101".to_owned(),
        pack_version: "1.0.0".to_owned(),
        pack_digest: "a".repeat(64),
        namespace: "sj_external_sandbox".to_owned(),
        ownership_nonce: "external-nonce".to_owned(),
        mode: EnvironmentMode::ExternalSandbox,
        state: EnvironmentState::Ready,
    };
    let target = ExternalSandboxTarget {
        profile_id: "profile-training".to_owned(),
        server_identity: "postgresql://db.example.test:5432".to_owned(),
        database_identity: "training".to_owned(),
    };

    let mut environments = EnvironmentStore::open(&path).expect("open environments");
    environments
        .save_environment(&sandbox)
        .expect("save external sandbox");
    environments
        .save_external_sandbox_target("external-sandbox", &target)
        .expect("save external target");
    drop(environments);
    let restored = EnvironmentStore::open(&path)
        .expect("reopen environments")
        .get_environment("external-sandbox")
        .expect("load sandbox")
        .expect("sandbox exists");
    assert_eq!(restored, sandbox);
    assert_eq!(
        EnvironmentStore::open(&path)
            .expect("reopen target store")
            .get_external_sandbox_target("external-sandbox")
            .expect("load sandbox target"),
        Some(target.clone())
    );

    let snapshot = export_backup(&path).expect("export backup");
    let encoded = serde_json::to_string(&snapshot).expect("encode backup");
    assert!(encoded.contains("profile-training"));
    assert!(!encoded.contains("password"));
    assert_eq!(
        snapshot.environments[0].external_target.as_ref(),
        Some(&target)
    );
    let _ = std::fs::remove_file(path);
}

fn connection_profile(id: &str) -> ConnectionProfile {
    ConnectionProfile {
        profile_id: id.to_owned(),
        display_name: "Training database".to_owned(),
        engine: "postgresql".to_owned(),
        host: "db.example.test".to_owned(),
        port: 5432,
        database: "training".to_owned(),
        username: "explorer".to_owned(),
        tls_mode: "verify_full".to_owned(),
        secret_ref: Some(format!("vault://sql-judge/{id}")),
    }
}

#[test]
fn environment_identity_columns_migrate_existing_metadata() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-environment-migration-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let connection = Connection::open(&path).expect("open legacy database");
    connection
        .execute_batch(
            "CREATE TABLE environment_ledger (
                 env_id TEXT PRIMARY KEY NOT NULL,
                 instance_id TEXT NOT NULL,
                 pack_id TEXT NOT NULL,
                 namespace TEXT NOT NULL UNIQUE,
                 ownership_nonce TEXT NOT NULL,
                 mode TEXT NOT NULL,
                 state TEXT NOT NULL
             );
             INSERT INTO environment_ledger VALUES
                 ('legacy', 'instance', 'hr-sql-101', 'sj_legacy', 'nonce', 'managed', 'ready');",
        )
        .expect("create legacy schema");
    drop(connection);

    let environments = EnvironmentStore::open(&path).expect("migrate environment store");
    let legacy = environments
        .get_environment("legacy")
        .expect("read legacy environment")
        .expect("legacy environment exists");
    assert!(legacy.pack_version.is_empty());
    assert!(legacy.pack_digest.is_empty());
    drop(environments);
    let _ = std::fs::remove_file(path);
}

#[test]
fn environment_migration_keeps_external_target_foreign_key_on_current_ledger() {
    let path = temp_database("external-sandbox-migration");
    Connection::open(&path)
        .expect("open legacy database")
        .execute_batch(
            "CREATE TABLE environment_ledger (
                 env_id TEXT PRIMARY KEY NOT NULL,
                 instance_id TEXT NOT NULL,
                 pack_id TEXT NOT NULL,
                 namespace TEXT NOT NULL UNIQUE,
                 ownership_nonce TEXT NOT NULL,
                 mode TEXT NOT NULL,
                 state TEXT NOT NULL
             );",
        )
        .expect("create legacy ledger");

    drop(EnvironmentStore::open(&path).expect("migrate environment store"));
    let connection = Connection::open(&path).expect("open migrated database");
    let foreign_table: String = connection
        .query_row(
            "SELECT \"table\" FROM pragma_foreign_key_list('external_sandbox_targets')",
            [],
            |row| row.get(0),
        )
        .expect("external target foreign key");
    assert_eq!(foreign_table, "environment_ledger");
    let _ = std::fs::remove_file(path);
}

#[test]
fn submission_problem_id_column_migrates_legacy_history() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-history-problem-migration-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let connection = Connection::open(&path).expect("open legacy database");
    connection
        .execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY CHECK (version = 1));
             INSERT INTO schema_version(version) VALUES (1);
             CREATE TABLE submissions (
                 submission_id TEXT PRIMARY KEY NOT NULL,
                 pack_id TEXT NOT NULL,
                 pack_version TEXT NOT NULL,
                 pack_hash TEXT NOT NULL,
                 engine_version TEXT NOT NULL,
                 policy_version TEXT NOT NULL,
                 verdict TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );
             CREATE TABLE submission_tests (
                 submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
                 test_id TEXT NOT NULL,
                 verdict TEXT NOT NULL,
                 elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
                 PRIMARY KEY (submission_id, test_id)
             );
             INSERT INTO submissions VALUES ('legacy', 'hr-sql-101', '1.0.0', 'sha256:legacy',
                 'postgresql-managed', 'policy-v1', 'Accepted', '2026-10-01T00:00:00Z');
             INSERT INTO submission_tests VALUES ('legacy', 'basic', 'Accepted', 3);",
        )
        .expect("create legacy history schema");
    drop(connection);

    {
        let mut history = HistoryStore::open(&path).expect("migrate history store");
        history
            .save_submission(&record("submission-new"), &[test_record("basic")])
            .expect("save submission with problem id");
    }
    let history = HistoryStore::open(&path).expect("reopen migrated history");
    let submissions = history.list_submissions().expect("list submissions");
    let legacy = submissions
        .iter()
        .find(|submission| submission.submission_id == "legacy")
        .expect("legacy submission survives");
    assert_eq!(legacy.problem_id, None);
    assert_eq!(legacy.verdict, "Accepted");
    let current = submissions
        .iter()
        .find(|submission| submission.submission_id == "submission-new")
        .expect("new submission");
    assert_eq!(current.problem_id.as_deref(), Some("above-average"));
    assert_eq!(history.test_count("legacy").expect("legacy tests"), 1);
    drop(history);
    let _ = std::fs::remove_file(path);
}

#[test]
fn submission_problem_id_must_be_a_valid_problem_identifier() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-history-problem-validation-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut history = HistoryStore::open(&path).expect("open history");
    for invalid in [
        "",
        " ",
        "Above Average",
        "../above-average",
        &"a".repeat(81),
    ] {
        let mut submission = record("submission-invalid-problem");
        submission.problem_id = Some(invalid.to_owned());
        assert!(
            history.save_submission(&submission, &[]).is_err(),
            "{invalid:?} must be rejected"
        );
    }
    let mut legacy = record("submission-without-problem");
    legacy.problem_id = None;
    history
        .save_submission(&legacy, &[])
        .expect("problem id stays optional");
    assert_eq!(
        history.list_submissions().expect("list submissions")[0].problem_id,
        None
    );
    drop(history);
    let _ = std::fs::remove_file(path);
}

#[test]
fn legacy_backup_without_problem_id_restores_and_new_backup_keeps_it() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-backup-problem-id-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    {
        let mut history = HistoryStore::open(&path).expect("open history");
        history
            .save_submission(&record("submission-with-problem"), &[test_record("basic")])
            .expect("save submission");
    }
    let snapshot = export_backup(&path).expect("export backup");
    assert_eq!(snapshot.version, 2);
    assert_eq!(
        snapshot.submissions[0].problem_id.as_deref(),
        Some("above-average")
    );
    let encoded = serde_json::to_string(&snapshot).expect("encode backup");
    assert!(encoded.contains("\"problemId\":\"above-average\""));

    let legacy_json = r#"{"version":2,"environments":[],"submissions":[{
        "submissionId":"legacy-submission","packId":"hr-sql-101","packVersion":"1.0.0",
        "packHash":"sha256:legacy","engineVersion":"postgresql-managed",
        "policyVersion":"policy-v1","verdict":"WrongAnswer",
        "createdAt":"2026-10-01T00:00:00Z",
        "tests":[{"testId":"basic","verdict":"WrongAnswer","elapsedMs":4}]}]}"#;
    let legacy: app_storage::BackupSnapshot =
        serde_json::from_str(legacy_json).expect("legacy v2 backup parses");
    assert_eq!(legacy.submissions[0].problem_id, None);
    assert!(
        !serde_json::to_string(&legacy)
            .expect("re-encode legacy backup")
            .contains("problemId")
    );
    restore_backup(&path, &legacy).expect("restore legacy backup");
    let history = HistoryStore::open(&path).expect("reopen history");
    let submissions = history.list_submissions().expect("list submissions");
    assert_eq!(submissions.len(), 1);
    assert_eq!(submissions[0].submission_id, "legacy-submission");
    assert_eq!(submissions[0].problem_id, None);
    drop(history);

    let mut forged = snapshot.clone();
    forged.submissions[0].problem_id = Some("Not A Problem".to_owned());
    assert!(restore_backup(&path, &forged).is_err());

    restore_backup(&path, &snapshot).expect("restore current backup");
    let history = HistoryStore::open(&path).expect("reopen history");
    assert_eq!(
        history.list_submissions().expect("list submissions")[0]
            .problem_id
            .as_deref(),
        Some("above-average")
    );
    drop(history);
    let _ = std::fs::remove_file(path);
}

#[test]
fn history_survives_store_reopen_without_credentials() {
    let path =
        std::env::temp_dir().join(format!("sql-judge-history-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);

    {
        let mut store = HistoryStore::open(&path).expect("open history");
        store
            .save_submission(&record("submission-1"), &[test_record("basic")])
            .expect("save submission");
    }

    let store = HistoryStore::open(&path).expect("reopen history");
    let submissions = store.list_submissions().expect("list submissions");
    assert_eq!(submissions.len(), 1);
    assert_eq!(submissions[0].pack_hash, "sha256:test");
    assert!(!submissions[0].pack_hash.contains("password"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn runtime_instance_id_survives_store_reopen() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-runtime-id-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let first = {
        let mut store = EnvironmentStore::open(&path).expect("open store");
        store
            .get_or_create_runtime_instance_id()
            .expect("create runtime identity")
    };
    let second = {
        let mut store = EnvironmentStore::open(&path).expect("reopen store");
        store
            .get_or_create_runtime_instance_id()
            .expect("restore runtime identity")
    };

    assert_eq!(first, second);
    assert!(first.starts_with("runtime-"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn environment_store_can_remove_deleted_environment() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-environment-delete-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut store = EnvironmentStore::open(&path).expect("open store");
    store
        .save_environment(&environment("env-delete", EnvironmentState::Deleting))
        .expect("save deleting environment");

    store
        .delete_environment("env-delete")
        .expect("delete environment");
    assert!(
        store
            .get_environment("env-delete")
            .expect("read deleted")
            .is_none()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn failed_submission_write_rolls_back_all_test_rows() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-history-rollback-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut store = HistoryStore::open(&path).expect("open history");
    store
        .save_submission(&record("submission-1"), &[test_record("basic")])
        .expect("save initial submission");

    let error = store.save_submission(
        &record("submission-1"),
        &[test_record("should-not-persist")],
    );
    assert!(error.is_err());
    assert_eq!(store.test_count("submission-1").expect("count tests"), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn environment_ledger_survives_reopen_and_updates_state_without_credentials() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-environments-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    {
        let mut store = EnvironmentStore::open(&path).expect("open environments");
        store
            .save_environment(&environment("env-a", EnvironmentState::Provisioning))
            .expect("save provisioning environment");
        store
            .save_environment(&environment("env-a", EnvironmentState::Ready))
            .expect("update environment state");
    }

    let store = EnvironmentStore::open(&path).expect("reopen environments");
    let restored = store
        .get_environment("env-a")
        .expect("load environment")
        .expect("environment exists");
    assert_eq!(restored.state, EnvironmentState::Ready);
    assert_eq!(restored.namespace, "sj_env_a");
    assert!(!restored.ownership_nonce.contains("password"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn environment_ledger_lists_records_for_restart_reconciliation() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-environments-list-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let mut store = EnvironmentStore::open(&path).expect("open environments");
    store
        .save_environment(&environment("env-a", EnvironmentState::Ready))
        .expect("save first");
    store
        .save_environment(&environment("env-b", EnvironmentState::NeedsRecovery))
        .expect("save second");

    let records = store.list_environments().expect("list environments");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].env_id, "env-a");
    assert_eq!(records[1].state, EnvironmentState::NeedsRecovery);
    let _ = std::fs::remove_file(path);
}

#[test]
fn environment_store_rejects_unowned_namespace_names() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-environments-invalid-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut store = EnvironmentStore::open(&path).expect("open environments");
    let mut record = environment("env-a", EnvironmentState::Ready);
    record.namespace = "public".to_owned();

    assert!(matches!(
        store.save_environment(&record),
        Err(app_storage::StorageError::InvalidEnvironment("namespace"))
    ));
    assert!(
        store
            .get_environment("env-a")
            .expect("query environments")
            .is_none()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn connection_profile_metadata_survives_reopen_without_secret_value() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-connection-profile-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    {
        let mut store = EnvironmentStore::open(&path).expect("open profiles");
        store
            .save_connection_profile(&connection_profile("profile-a"))
            .expect("save profile");
    }

    let store = EnvironmentStore::open(&path).expect("reopen profiles");
    let profiles = store.list_connection_profiles().expect("list profiles");
    assert_eq!(profiles, vec![connection_profile("profile-a")]);
    let encoded = serde_json::to_string(&profiles).expect("encode profiles");
    assert!(!encoded.contains("password"));
    assert!(!encoded.contains("super-secret"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn connection_profile_validation_rejects_unsafe_metadata() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-connection-profile-invalid-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut store = EnvironmentStore::open(&path).expect("open profiles");
    let mut invalid = connection_profile("profile-invalid");
    invalid.port = 0;
    assert!(matches!(
        store.save_connection_profile(&invalid),
        Err(app_storage::StorageError::InvalidConnectionProfile("port"))
    ));
    invalid.port = 5432;
    invalid.profile_id = "profile/invalid".to_owned();
    assert!(matches!(
        store.save_connection_profile(&invalid),
        Err(app_storage::StorageError::InvalidConnectionProfile(
            "profile id"
        ))
    ));
    invalid.profile_id = "profile-invalid".to_owned();
    invalid.tls_mode = "disable".to_owned();
    assert!(matches!(
        store.save_connection_profile(&invalid),
        Err(app_storage::StorageError::InvalidConnectionProfile(
            "tls mode"
        ))
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn connection_profile_delete_removes_only_metadata() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-connection-profile-delete-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut store = EnvironmentStore::open(&path).expect("open profiles");
    store
        .save_connection_profile(&connection_profile("profile-delete"))
        .expect("save profile");
    store
        .delete_connection_profile("profile-delete")
        .expect("delete profile");
    assert!(
        store
            .get_connection_profile("profile-delete")
            .expect("read deleted profile")
            .is_none()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn backup_roundtrip_excludes_secrets_and_restores_metadata() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-backup-roundtrip-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    {
        let mut environments = EnvironmentStore::open(&path).expect("open environments");
        environments
            .save_environment(&environment("env-backup", EnvironmentState::Ready))
            .expect("save environment");
        environments
            .save_environment(&EnvironmentRecord {
                env_id: "external-backup".to_owned(),
                instance_id: "external-instance".to_owned(),
                pack_id: "external".to_owned(),
                pack_version: String::new(),
                pack_digest: String::new(),
                namespace: "sj_external".to_owned(),
                ownership_nonce: "external-nonce".to_owned(),
                mode: EnvironmentMode::ExternalExplorer,
                state: EnvironmentState::Ready,
            })
            .expect("save external environment");
        environments
            .save_connection_profile(&connection_profile("profile-backup"))
            .expect("save external profile");
        let mut history = HistoryStore::open(&path).expect("open history");
        history
            .save_submission(&record("submission-backup"), &[test_record("basic")])
            .expect("save submission");
    }

    let mut snapshot = export_backup(&path).expect("export backup");
    snapshot
        .environments
        .retain(|environment| environment.env_id != "external-backup");
    let encoded = serde_json::to_string(&snapshot).expect("encode backup");
    assert!(!encoded.contains("password"));
    assert!(!encoded.contains("secret_ref"));
    assert!(!encoded.contains("SELECT"));
    assert_eq!(snapshot.submissions[0].pack_id, "hr-sql-101");
    assert_eq!(snapshot.submissions[0].pack_version, "1.0.0");
    assert_eq!(snapshot.submissions[0].pack_hash, "sha256:test");
    assert!(!encoded.contains("schema_sql"));
    assert!(!encoded.contains("registry"));
    assert!(!encoded.contains("files"));

    let mut changed_version = snapshot.clone();
    changed_version.environments[0].pack_version = "9.9.9".to_owned();
    assert!(restore_backup(&path, &changed_version).is_err());
    let preserved = EnvironmentStore::open(&path)
        .expect("reopen after version rejection")
        .get_environment("env-backup")
        .expect("read preserved environment")
        .expect("preserved environment exists");
    assert_eq!(preserved.pack_version, "1.0.0");

    let mut changed_digest = snapshot.clone();
    changed_digest.environments[0].pack_digest = "b".repeat(64);
    assert!(restore_backup(&path, &changed_digest).is_err());
    let preserved = EnvironmentStore::open(&path)
        .expect("reopen after digest rejection")
        .get_environment("env-backup")
        .expect("read preserved environment")
        .expect("preserved environment exists");
    assert!(
        preserved
            .pack_digest
            .chars()
            .all(|character| character == 'a')
    );

    let embedded_pack_payload = encoded.replace(
        "\"submissions\":[",
        "\"submissions\":[{\"schemaSql\":\"DROP TABLE users\"},",
    );
    assert!(serde_json::from_str::<app_storage::BackupSnapshot>(&embedded_pack_payload).is_err());

    let mut invalid_version = snapshot.clone();
    invalid_version.version = 3;
    assert!(restore_backup(&path, &invalid_version).is_err());

    restore_backup(&path, &snapshot).expect("restore backup");
    let environments = EnvironmentStore::open(&path).expect("reopen environments");
    let history = HistoryStore::open(&path).expect("reopen history");
    assert_eq!(
        environments
            .list_environments()
            .expect("list environments")
            .len(),
        2
    );
    assert_eq!(
        environments
            .get_environment("external-backup")
            .expect("read external environment")
            .expect("external environment remains")
            .mode,
        EnvironmentMode::ExternalExplorer
    );
    assert_eq!(
        environments
            .list_connection_profiles()
            .expect("list external profiles")
            .iter()
            .map(|profile| profile.profile_id.as_str())
            .collect::<Vec<_>>(),
        vec!["profile-backup"]
    );
    assert_eq!(
        history.list_submissions().expect("list submissions").len(),
        1
    );
    assert_eq!(
        history
            .test_count("submission-backup")
            .expect("count tests"),
        1
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn backup_export_rejects_pending_recovery_without_changing_metadata() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-backup-pending-recovery-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut environments = EnvironmentStore::open(&path).expect("open environments");
    environments
        .save_environment(&environment("env-pending", EnvironmentState::Ready))
        .expect("save environment");
    environments
        .save_recovery(&RecoveryRecord {
            env_id: "env-pending".to_owned(),
            operation: RecoveryOperation::Rebuild,
            instance_id: "instance-env-pending".to_owned(),
            ownership_nonce: "nonce-env-pending".to_owned(),
            namespace: "sj_env_pending".to_owned(),
            replacement_namespace: Some("sj_env_pending_next".to_owned()),
        })
        .expect("save recovery");
    let mut history = HistoryStore::open(&path).expect("open history");
    history
        .save_submission(&record("pending-submission"), &[test_record("basic")])
        .expect("save history");

    assert!(matches!(
        export_backup(&path),
        Err(app_storage::StorageError::InvalidEnvironment(
            "pending environment recovery"
        ))
    ));
    assert_eq!(
        EnvironmentStore::open(&path)
            .expect("reopen environments")
            .list_recoveries()
            .expect("list recoveries")
            .len(),
        1
    );
    assert_eq!(
        HistoryStore::open(&path)
            .expect("reopen history")
            .list_submissions()
            .expect("list history")
            .len(),
        1
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn malformed_backup_environment_is_rejected_before_replacement() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-backup-invalid-environment-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    {
        let mut environments = EnvironmentStore::open(&path).expect("open environments");
        environments
            .save_environment(&environment("env-existing", EnvironmentState::Ready))
            .expect("save environment");
        let mut history = HistoryStore::open(&path).expect("open history");
        history
            .save_submission(&record("submission-existing"), &[test_record("basic")])
            .expect("save submission");
    }

    let mut snapshot = export_backup(&path).expect("export backup");
    snapshot.environments[0].namespace = "public".to_owned();
    assert!(restore_backup(&path, &snapshot).is_err());

    let environments = EnvironmentStore::open(&path).expect("reopen environments");
    let history = HistoryStore::open(&path).expect("reopen history");
    assert_eq!(
        environments
            .get_environment("env-existing")
            .expect("read existing environment")
            .expect("existing environment remains")
            .namespace,
        "sj_env_existing"
    );
    assert_eq!(
        history
            .list_submissions()
            .expect("read existing history")
            .len(),
        1
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn backup_rejects_unstable_managed_environment_states() {
    for state in [
        EnvironmentState::Provisioning,
        EnvironmentState::Running,
        EnvironmentState::Rebuilding,
        EnvironmentState::Deleting,
        EnvironmentState::NeedsRecovery,
    ] {
        let path = std::env::temp_dir().join(format!(
            "sql-judge-backup-unstable-{}-{}",
            std::process::id(),
            state as u8
        ));
        let _ = std::fs::remove_file(&path);
        let mut environments = EnvironmentStore::open(&path).expect("open environments");
        environments
            .save_environment(&environment("env-unstable", state))
            .expect("save unstable environment");

        assert!(matches!(
            export_backup(&path),
            Err(app_storage::StorageError::InvalidEnvironment(
                "environment state"
            ))
        ));
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn backup_restore_rejects_unstable_managed_environment_states() {
    let path = std::env::temp_dir().join(format!(
        "sql-judge-backup-restore-unstable-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut environments = EnvironmentStore::open(&path).expect("open environments");
    environments
        .save_environment(&environment("env-unstable", EnvironmentState::Ready))
        .expect("save ready environment");
    let mut snapshot = export_backup(&path).expect("export backup");

    for state in [
        "provisioning",
        "running",
        "rebuilding",
        "deleting",
        "needs_recovery",
    ] {
        snapshot.environments[0].state = state.to_owned();
        assert!(matches!(
            restore_backup(&path, &snapshot),
            Err(app_storage::StorageError::InvalidEnvironment(
                "environment state"
            ))
        ));
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn recovery_journal_survives_reopen_and_can_be_cleared() {
    let path =
        std::env::temp_dir().join(format!("sql-judge-recovery-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let recovery = RecoveryRecord {
        env_id: "env-a".to_owned(),
        operation: RecoveryOperation::Rebuild,
        instance_id: "instance-env-a".to_owned(),
        ownership_nonce: "nonce-env-a".to_owned(),
        namespace: "sj_env_a".to_owned(),
        replacement_namespace: Some("sj_env_a_next".to_owned()),
    };

    {
        let mut store = EnvironmentStore::open(&path).expect("open recovery store");
        store
            .save_recovery(&recovery)
            .expect("save recovery record");
    }

    let mut store = EnvironmentStore::open(&path).expect("reopen recovery store");
    let records = store.list_recoveries().expect("list recovery records");
    assert_eq!(records, vec![recovery]);
    store
        .clear_recovery("env-a")
        .expect("clear recovery record");
    assert!(
        store
            .list_recoveries()
            .expect("list cleared records")
            .is_empty()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn legacy_history_schema_migrates_and_lists_rows_as_untracked_submissions() {
    let path = temp_database("history-legacy-migration");
    Connection::open(&path)
        .expect("open legacy history")
        .execute_batch(LEGACY_HISTORY_DDL)
        .expect("create legacy history schema");

    drop(HistoryStore::open(&path).expect("migrate legacy history"));
    let store = HistoryStore::open(&path).expect("reopen migrated history");

    assert_eq!(
        store.list_submissions().expect("list migrated history"),
        vec![SubmissionSummary {
            submission_id: "legacy-1".to_owned(),
            pack_id: "hr-sql-101".to_owned(),
            pack_version: "1.0.0".to_owned(),
            pack_hash: "sha256:legacy".to_owned(),
            engine_version: "postgresql-managed".to_owned(),
            policy_version: "policy-v1".to_owned(),
            verdict: "WrongAnswer".to_owned(),
            created_at: "1760000000".to_owned(),
            problem_id: None,
            mode: "submit".to_owned(),
            tests: vec![TestRecord {
                test_id: "basic".to_owned(),
                verdict: "WrongAnswer".to_owned(),
                elapsed_ms: 7,
            }],
        }]
    );
    let columns = history_columns(&path);
    assert_eq!(
        columns
            .iter()
            .filter(|column| *column == "problem_id")
            .count(),
        1
    );
    assert_eq!(columns.iter().filter(|column| *column == "mode").count(), 1);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn sample_run_round_trips_with_problem_mode_and_tests() {
    let path = temp_database("history-run-roundtrip");
    {
        let mut store = HistoryStore::open(&path).expect("open history");
        store
            .save_submission(
                &run_record("run-1"),
                &[
                    TestRecord {
                        test_id: "visible-b".to_owned(),
                        verdict: "WrongAnswer".to_owned(),
                        elapsed_ms: 9,
                    },
                    test_record("visible-a"),
                ],
            )
            .expect("save sample run");
        store
            .save_submission(&record("submit-1"), &[test_record("hidden")])
            .expect("save submission");
    }

    let listed = HistoryStore::open(&path)
        .expect("reopen history")
        .list_submissions()
        .expect("list history");
    assert_eq!(listed.len(), 2);
    let run = &listed[0];
    assert_eq!(run.submission_id, "run-1");
    assert_eq!(run.problem_id.as_deref(), Some("above-average"));
    assert_eq!(run.mode, "run");
    assert_eq!(
        run.tests,
        vec![
            test_record("visible-a"),
            TestRecord {
                test_id: "visible-b".to_owned(),
                verdict: "WrongAnswer".to_owned(),
                elapsed_ms: 9,
            },
        ]
    );
    assert_eq!(listed[1].mode, "submit");
    assert_eq!(listed[1].tests, vec![test_record("hidden")]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_submission_mode_and_problem_id_are_rejected() {
    let path = temp_database("history-invalid-mode");
    let mut store = HistoryStore::open(&path).expect("open history");
    for (mode, problem_id, field) in [
        ("debug", Some("above-average"), "mode"),
        ("", Some("above-average"), "mode"),
        ("RUN", Some("above-average"), "mode"),
        ("run", Some(""), "problem id"),
        ("run", Some("bad\nid"), "problem id"),
        ("submit", Some(" "), "problem id"),
    ] {
        let invalid = SubmissionRecord {
            mode: mode.to_owned(),
            problem_id: problem_id.map(str::to_owned),
            ..record("invalid")
        };
        assert!(
            matches!(
                store.save_submission(&invalid, &[test_record("basic")]),
                Err(StorageError::InvalidRecord(rejected)) if rejected == field
            ),
            "{mode:?}/{problem_id:?} must be rejected as {field}"
        );
    }
    assert!(store.list_submissions().expect("list history").is_empty());
    drop(store);

    let raw_insert = Connection::open(&path).expect("open raw history").execute(
        "INSERT INTO submissions (
                 submission_id, pack_id, pack_version, pack_hash, engine_version,
                 policy_version, verdict, created_at, problem_id, mode
             ) VALUES ('raw', 'p', '1', 'h', 'e', 'v', 'Accepted', '0', NULL, 'debug')",
        [],
    );
    assert!(
        raw_insert.is_err(),
        "SQLite must enforce the mode constraint"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn backup_exports_problem_and_run_mode_and_restores_them() {
    let path = temp_database("backup-run-mode-history");
    let before = {
        let mut store = HistoryStore::open(&path).expect("open history");
        store
            .save_submission(&run_record("run-1"), &[test_record("visible")])
            .expect("save sample run");
        store
            .save_submission(
                &SubmissionRecord {
                    problem_id: None,
                    ..record("legacy-submit")
                },
                &[test_record("basic")],
            )
            .expect("save untracked submission");
        store.list_submissions().expect("list history")
    };

    let snapshot = export_backup(&path).expect("export backup");
    assert_eq!(snapshot.version, 2);
    let encoded = serde_json::to_string(&snapshot).expect("encode backup");
    assert!(encoded.contains("\"problemId\":\"above-average\""));
    assert!(encoded.contains("\"mode\":\"run\""));
    assert!(!encoded.contains("SELECT"));
    let untracked = serde_json::to_string(
        snapshot
            .submissions
            .iter()
            .find(|submission| submission.submission_id == "legacy-submit")
            .expect("untracked submission"),
    )
    .expect("encode untracked submission");
    assert!(
        !untracked.contains("problemId") && !untracked.contains("mode"),
        "untracked submissions serialize exactly as before: {untracked}"
    );

    restore_backup(&path, &snapshot).expect("restore v3 backup");
    assert_eq!(
        HistoryStore::open(&path)
            .expect("reopen history")
            .list_submissions()
            .expect("list restored history"),
        before
    );
    let _ = std::fs::remove_file(path);
}

const V2_BACKUP: &str = r#"{"version":2,"environments":[],"submissions":[{"submissionId":"v2-submission","packId":"hr-sql-101","packVersion":"1.0.0","packHash":"sha256:v2","engineVersion":"postgresql-managed","policyVersion":"policy-v1","verdict":"Accepted","createdAt":"1760000000","tests":[{"testId":"basic","verdict":"Accepted","elapsedMs":3}]}]}"#;

#[test]
fn backup_without_tracking_fields_restores_as_untracked_submissions() {
    let path = temp_database("backup-v2-restore");
    let snapshot = serde_json::from_str::<BackupSnapshot>(V2_BACKUP).expect("decode v2 backup");

    restore_backup(&path, &snapshot).expect("restore v2 backup");

    let listed = HistoryStore::open(&path)
        .expect("reopen history")
        .list_submissions()
        .expect("list restored history");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].submission_id, "v2-submission");
    assert_eq!(listed[0].problem_id, None);
    assert_eq!(listed[0].mode, "submit");
    assert_eq!(
        listed[0].tests,
        vec![TestRecord {
            test_id: "basic".to_owned(),
            verdict: "Accepted".to_owned(),
            elapsed_ms: 3,
        }]
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn backup_restore_rejects_invalid_history_fields_without_replacement() {
    let path = temp_database("backup-invalid-history-fields");
    HistoryStore::open(&path)
        .expect("open history")
        .save_submission(&record("existing"), &[test_record("basic")])
        .expect("save existing submission");

    for payload in [
        V2_BACKUP.replace("\"version\":2", "\"version\":3"),
        V2_BACKUP.replace("\"tests\":", "\"mode\":\"debug\",\"tests\":"),
        V2_BACKUP.replace("\"tests\":", "\"mode\":\"RUN\",\"tests\":"),
        V2_BACKUP.replace(
            "\"tests\":",
            "\"problemId\":\"\",\"mode\":\"run\",\"tests\":",
        ),
        V2_BACKUP.replace("\"tests\":", "\"problemId\":\"Above Average\",\"tests\":"),
    ] {
        let snapshot = serde_json::from_str::<BackupSnapshot>(&payload).expect("decode payload");
        assert!(
            restore_backup(&path, &snapshot).is_err(),
            "payload must be rejected: {payload}"
        );
    }

    let listed = HistoryStore::open(&path)
        .expect("reopen history")
        .list_submissions()
        .expect("list preserved history");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].submission_id, "existing");
    let _ = std::fs::remove_file(path);
}
