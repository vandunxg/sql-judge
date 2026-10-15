use judge_core::{Column, LogicalType, OutputPolicy, QueryLimits, RowOrder, SqlValue, Verdict};
use judge_postgres::{
    CancellationToken, ExternalPostgresConfig, ExternalPostgresError, ExternalPostgresExplorer,
    ExternalPostgresSandbox, ExternalSandboxConfig, ExternalTlsMode, JudgeCase, ManagedEnvironment,
    PostgresSession, QueryExecutionError, judge_submission,
    judge_submission_in_namespace_with_watchdog_token, judge_submission_with_cancel,
    judge_submission_with_watchdog,
};
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct TestServer {
    data_dir: PathBuf,
    socket_dir: PathBuf,
    port: u16,
    password: String,
    provisioner_password: String,
}

impl TestServer {
    fn start() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sql-judge-pg-rust-{}-{nonce}", std::process::id()));
        let data_dir = root.join("data");
        let socket_dir = root.join("socket");
        std::fs::create_dir_all(&socket_dir).expect("create PostgreSQL socket directory");
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("reserve PostgreSQL port")
            .local_addr()
            .expect("read reserved port")
            .port();
        let admin = std::env::var("USER").expect("USER must identify the local test owner");

        assert!(
            Command::new("initdb")
                .args([
                    "-D",
                    data_dir.to_str().expect("data path"),
                    "--username",
                    &admin,
                    "--auth-local=peer",
                    "--auth-host=scram-sha-256",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("run initdb")
                .success()
        );
        assert!(
            Command::new("pg_ctl")
                .args([
                    "-D",
                    data_dir.to_str().expect("data path"),
                    "-o",
                    &format!(
                        "-h 127.0.0.1 -p {port} -k {} -c listen_addresses=127.0.0.1",
                        socket_dir.to_str().expect("socket path")
                    ),
                    "-w",
                    "start",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("start PostgreSQL")
                .success()
        );
        assert!(
            Command::new("createdb")
                .args([
                    "-h",
                    socket_dir.to_str().expect("socket path"),
                    "-p",
                    &port.to_string(),
                    "sqljudge_test",
                ])
                .status()
                .expect("create test database")
                .success()
        );

        let password = format!("sqljudge_test_{}", std::process::id());
        let provisioner_password = format!("sqljudge_provisioner_{}", std::process::id());
        let setup = format!(
            "CREATE ROLE sqljudge_runner LOGIN PASSWORD '{password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;\
             CREATE ROLE sqljudge_provisioner LOGIN PASSWORD '{provisioner_password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;\
             REVOKE CREATE, TEMPORARY ON DATABASE sqljudge_test FROM PUBLIC;\
             GRANT CREATE ON DATABASE sqljudge_test TO sqljudge_provisioner;\
             REVOKE CREATE ON SCHEMA public FROM PUBLIC;\
             GRANT USAGE ON SCHEMA public TO sqljudge_runner;\
             CREATE TABLE public.sample (id integer NOT NULL, label text);\
             INSERT INTO public.sample VALUES (1, 'one'), (2, NULL);\
             GRANT SELECT ON public.sample TO sqljudge_runner;\
             CREATE SEQUENCE public.sample_seq;"
        );
        let mut setup_process = Command::new("psql")
            .args([
                "-h",
                socket_dir.to_str().expect("socket path"),
                "-p",
                &port.to_string(),
                "-d",
                "sqljudge_test",
                "-v",
                "ON_ERROR_STOP=1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start psql setup");
        setup_process
            .stdin
            .take()
            .expect("open psql stdin")
            .write_all(setup.as_bytes())
            .expect("write setup SQL");
        assert!(setup_process.wait().expect("wait for setup SQL").success());

        Self {
            data_dir,
            socket_dir,
            port,
            password,
            provisioner_password,
        }
    }

    fn connection_string(&self) -> String {
        format!(
            "host=127.0.0.1 port={} user=sqljudge_runner password={} dbname=sqljudge_test",
            self.port, self.password
        )
    }

    fn admin_connection_string(&self) -> String {
        format!(
            "host={} port={} user={} dbname=sqljudge_test",
            self.socket_dir.to_str().expect("socket path"),
            self.port,
            std::env::var("USER").expect("USER must identify the local test owner")
        )
    }

    fn external_sandbox_config(&self, namespace: &str) -> ExternalSandboxConfig {
        ExternalSandboxConfig::new(
            ExternalPostgresConfig {
                host: "127.0.0.1".to_owned(),
                port: self.port,
                database: "sqljudge_test".to_owned(),
                username: "sqljudge_provisioner".to_owned(),
                password: self.provisioner_password.clone(),
                tls_mode: ExternalTlsMode::Disable,
                allow_insecure_local: true,
            },
            "profile-test",
            "environment-test",
            namespace,
            "nonce-test",
        )
        .expect("valid external sandbox config")
    }

    fn run_admin_sql(&self, sql: &str) {
        let mut process = Command::new("psql")
            .args([
                "-h",
                self.socket_dir.to_str().expect("socket path"),
                "-p",
                &self.port.to_string(),
                "-d",
                "sqljudge_test",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start admin SQL");
        assert!(process.wait().expect("wait for admin SQL").success());
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = Command::new("pg_ctl")
            .args([
                "-D",
                self.data_dir.to_str().expect("data path"),
                "-w",
                "stop",
                "-m",
                "fast",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = std::fs::remove_dir_all(self.data_dir.parent().expect("test root"));
        let _ = &self.socket_dir;
    }
}

#[test]
fn session_decodes_typed_rows_and_nulls() {
    let server = TestServer::start();
    let mut session =
        PostgresSession::connect(&server.connection_string()).expect("connect runner");

    let result = session
        .execute(
            "SELECT id, label FROM public.sample ORDER BY id",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("execute typed query");

    assert_eq!(result.columns[0].logical_type, LogicalType::Integer);
    assert_eq!(result.columns[1].logical_type, LogicalType::Text);
    assert_eq!(result.rows[0][0], SqlValue::Signed(1));
    assert_eq!(result.rows[1][1], SqlValue::Null);
}

#[test]
fn session_decodes_numeric_rows_as_decimal_values() {
    let server = TestServer::start();
    let mut session =
        PostgresSession::connect(&server.connection_string()).expect("connect runner");

    let result = session
        .execute(
            "SELECT 12.50::numeric AS amount",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("execute numeric query");

    assert_eq!(result.columns[0].logical_type, LogicalType::Numeric);
    assert_eq!(result.rows[0][0], SqlValue::Decimal("12.50".to_owned()));
}

#[test]
fn external_explorer_tests_connection_metadata_and_bounded_select() {
    let server = TestServer::start();
    let mut explorer = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
        host: "127.0.0.1".to_owned(),
        port: server.port,
        database: "sqljudge_test".to_owned(),
        username: "sqljudge_runner".to_owned(),
        password: server.password.clone(),
        tls_mode: ExternalTlsMode::Disable,
        allow_insecure_local: true,
    })
    .expect("connect external explorer");

    let connection = explorer
        .test_connection()
        .expect("test external connection");
    assert_eq!(connection.database, "sqljudge_test");
    assert_eq!(connection.username, "sqljudge_runner");
    assert!(connection.read_only);

    let metadata = explorer
        .list_metadata(QueryLimits::new(2_000, 100, 64 * 1024))
        .expect("list external metadata");
    let sample = metadata
        .tables
        .iter()
        .find(|table| table.schema == "public" && table.name == "sample")
        .expect("sample table metadata");
    assert_eq!(sample.columns[0].name, "id");

    let result = explorer
        .execute_select(
            "SELECT id, label FROM public.sample ORDER BY id",
            QueryLimits::new(2_000, 2, 64 * 1024),
        )
        .expect("bounded external select");
    assert_eq!(result.rows.len(), 2);
    assert!(matches!(
        explorer.execute_select(
            "SELECT generate_series(1, 4)",
            QueryLimits::new(2_000, 2, 64 * 1024),
        ),
        Err(ExternalPostgresError::ResultLimitExceeded)
    ));
    assert!(matches!(
        explorer.execute_select(
            "SELECT setval('public.sample_seq', 10)",
            QueryLimits::new(2_000, 2, 64 * 1024),
        ),
        Err(ExternalPostgresError::DatabaseError)
    ));
}

#[test]
fn external_connection_rejects_effective_write_capabilities() {
    let server = TestServer::start();
    for (grant, revoke) in [
        (
            "GRANT INSERT ON TABLE public.sample TO sqljudge_runner",
            "REVOKE INSERT ON TABLE public.sample FROM sqljudge_runner",
        ),
        (
            "GRANT CREATE ON SCHEMA public TO sqljudge_runner",
            "REVOKE CREATE ON SCHEMA public FROM sqljudge_runner",
        ),
        (
            "GRANT CREATE ON DATABASE sqljudge_test TO sqljudge_runner",
            "REVOKE CREATE ON DATABASE sqljudge_test FROM sqljudge_runner",
        ),
        (
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT INSERT ON TABLES TO sqljudge_runner",
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public REVOKE INSERT ON TABLES FROM sqljudge_runner",
        ),
    ] {
        server.run_admin_sql(grant);
        let mut explorer = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
            host: "127.0.0.1".to_owned(),
            port: server.port,
            database: "sqljudge_test".to_owned(),
            username: "sqljudge_runner".to_owned(),
            password: server.password.clone(),
            tls_mode: ExternalTlsMode::Disable,
            allow_insecure_local: true,
        })
        .expect("connect external explorer");
        assert!(
            !explorer
                .test_connection()
                .expect("test external connection")
                .read_only,
            "effective write capability must not be hidden by BEGIN READ ONLY: {grant}"
        );
        server.run_admin_sql(revoke);
    }
}

#[test]
fn external_connection_with_default_temporary_privilege_is_read_only() {
    let server = TestServer::start();
    // PostgreSQL grants TEMPORARY to PUBLIC by default. Temporary objects are session-local
    // and cannot be created inside the explorer's READ ONLY transactions.
    server.run_admin_sql("GRANT TEMPORARY ON DATABASE sqljudge_test TO PUBLIC");
    let mut explorer = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
        host: "127.0.0.1".to_owned(),
        port: server.port,
        database: "sqljudge_test".to_owned(),
        username: "sqljudge_runner".to_owned(),
        password: server.password.clone(),
        tls_mode: ExternalTlsMode::Disable,
        allow_insecure_local: true,
    })
    .expect("connect external explorer");

    assert!(
        explorer
            .test_connection()
            .expect("test external connection")
            .read_only
    );
}

#[test]
fn external_tls_modes_never_fall_back_to_plaintext() {
    // The test server runs without TLS, so every TLS mode must refuse to connect.
    let server = TestServer::start();
    for tls_mode in [
        ExternalTlsMode::VerifyFull,
        ExternalTlsMode::VerifyCa,
        ExternalTlsMode::Require,
    ] {
        let result = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
            host: "127.0.0.1".to_owned(),
            port: server.port,
            database: "sqljudge_test".to_owned(),
            username: "sqljudge_runner".to_owned(),
            password: server.password.clone(),
            tls_mode,
            allow_insecure_local: false,
        });
        assert!(
            matches!(result, Err(ExternalPostgresError::TlsFailed)),
            "{tls_mode:?} must not connect without TLS"
        );
    }
}

#[test]
fn external_sandbox_rejects_a_preexisting_unmarked_schema() {
    let server = TestServer::start();
    server.run_admin_sql("CREATE SCHEMA sj_external_unmarked");
    let config = server.external_sandbox_config("sj_external_unmarked");
    let mut sandbox = ExternalPostgresSandbox::connect(config).expect("connect provisioner");

    assert_eq!(
        sandbox.provision(),
        Err(ExternalPostgresError::OwnershipVerificationFailed)
    );
    server.run_admin_sql(
        "DO $$ BEGIN
             IF to_regnamespace('sj_external_unmarked') IS NULL
                OR to_regclass('sj_external_unmarked.__sql_judge_external_sandbox') IS NOT NULL THEN
               RAISE EXCEPTION 'unmarked schema was modified';
             END IF;
           END $$",
    );
}

#[test]
fn external_sandbox_deletes_only_its_marker_verified_schema() {
    let server = TestServer::start();
    let config = server.external_sandbox_config("sj_external_owned");
    let mut sandbox = ExternalPostgresSandbox::connect(config).expect("connect provisioner");

    sandbox.provision().expect("provision owned schema");
    assert!(sandbox.verify_ownership().expect("verify marker"));
    sandbox
        .delete_owned_schema()
        .expect("delete marker-verified schema");
    server.run_admin_sql(
        "DO $$ BEGIN
             IF to_regnamespace('sj_external_owned') IS NOT NULL THEN
               RAISE EXCEPTION 'owned schema was not deleted';
             END IF;
           END $$",
    );
}

fn sandbox_case<'a>(id: &'a str, fixture_sql: &'a str) -> JudgeCase<'a> {
    JudgeCase {
        id,
        schema_sql: "",
        fixture_sql,
        reference_sql: "SELECT id FROM values_table ORDER BY id",
        output: OutputPolicy::new(
            RowOrder::Ordered,
            true,
            true,
            vec![Column::new("id", LogicalType::Integer)],
        ),
        limits: QueryLimits::new(2_000, 100, 64 * 1024),
    }
}

#[test]
fn namespace_judge_loads_each_fixture_in_a_rolled_back_transaction() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let sandbox = ManagedEnvironment::new(
        "pack-sandbox",
        "instance-sandbox",
        "sj_pack_sandbox",
        "nonce",
    )
    .expect("sandbox identity");
    admin
        .provision_managed_namespace_with_schema(
            &sandbox,
            "sqljudge_runner",
            "CREATE TABLE values_table (id integer);
             CREATE TABLE counters (n integer GENERATED ALWAYS AS IDENTITY);",
        )
        .expect("provision sandbox");
    let cases = [
        sandbox_case("one", "INSERT INTO values_table VALUES (1);"),
        sandbox_case("two", "INSERT INTO values_table VALUES (2), (3);"),
    ];
    let judge = |admin: &mut PostgresSession, environment: &ManagedEnvironment, sql: &str| {
        judge_submission_in_namespace_with_watchdog_token(
            admin,
            environment,
            "sqljudge_runner",
            &cases,
            sql,
            &CancellationToken::default(),
            std::time::Duration::from_secs(5),
        )
    };

    let accepted = judge(
        &mut admin,
        &sandbox,
        "SELECT id FROM values_table ORDER BY id",
    );
    assert_eq!(accepted.verdict, Verdict::Accepted);
    assert_eq!(
        accepted.preview.expect("preview").rows,
        vec![vec![SqlValue::Signed(1)]],
        "each fixture only sees its own rows"
    );

    let only_first = judge(&mut admin, &sandbox, "SELECT 1 AS id");
    assert_eq!(
        only_first
            .tests
            .iter()
            .map(|test| test.verdict)
            .collect::<Vec<_>>(),
        vec![Verdict::Accepted, Verdict::WrongAnswer]
    );

    let writes = judge(
        &mut admin,
        &sandbox,
        "SELECT nextval('counters_n_seq')::integer AS id",
    );
    assert!(
        writes
            .tests
            .iter()
            .all(|test| test.verdict == Verdict::PolicyViolation)
    );
    let inside = admin
        .with_namespace_fixture(
            &sandbox,
            "sqljudge_runner",
            "INSERT INTO values_table VALUES (7);",
            |session| {
                session.execute(
                    "SELECT current_setting('transaction_read_only') AS read_only,
                            (SELECT count(*) FROM values_table) AS loaded",
                    QueryLimits::new(2_000, 10, 64 * 1024),
                )
            },
        )
        .expect("run inside the sandbox fixture");
    assert_eq!(
        inside.rows,
        vec![vec![SqlValue::Text("on".to_owned()), SqlValue::Signed(1)]],
        "queries see the loaded fixture inside a read-only transaction"
    );

    let remaining = admin
        .execute(
            "SELECT count(*) FROM sj_pack_sandbox.values_table",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect sandbox rows");
    assert_eq!(remaining.rows[0][0], SqlValue::Signed(0));

    let forged = ManagedEnvironment::new(
        "pack-sandbox",
        "instance-sandbox",
        "sj_pack_sandbox",
        "other-nonce",
    )
    .expect("forged identity");
    assert_eq!(
        judge(&mut admin, &forged, "SELECT id FROM values_table").verdict,
        Verdict::JudgeError
    );
}

#[test]
fn external_explorer_maps_wrong_credentials_without_returning_driver_text() {
    let server = TestServer::start();
    let result = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
        host: "127.0.0.1".to_owned(),
        port: server.port,
        database: "sqljudge_test".to_owned(),
        username: "sqljudge_runner".to_owned(),
        password: "wrong-secret-canary".to_owned(),
        tls_mode: ExternalTlsMode::Disable,
        allow_insecure_local: true,
    });
    assert!(matches!(
        result,
        Err(ExternalPostgresError::AuthenticationFailed)
    ));
}

#[test]
fn external_explorer_maps_offline_server_without_secret_leak() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve offline port");
    let port = listener.local_addr().expect("read offline port").port();
    drop(listener);
    let result = ExternalPostgresExplorer::connect(ExternalPostgresConfig {
        host: "127.0.0.1".to_owned(),
        port,
        database: "sqljudge_test".to_owned(),
        username: "reader".to_owned(),
        password: "offline-secret-canary".to_owned(),
        tls_mode: ExternalTlsMode::Disable,
        allow_insecure_local: true,
    });
    assert!(matches!(
        result,
        Err(ExternalPostgresError::ServerUnreachable)
    ));
}

#[test]
fn session_distinguishes_syntax_timeout_and_result_limits() {
    let server = TestServer::start();
    let mut session =
        PostgresSession::connect(&server.connection_string()).expect("connect runner");

    assert_eq!(
        session.execute("SELEC 1", QueryLimits::new(2_000, 10, 64 * 1024)),
        Err(QueryExecutionError::SyntaxError)
    );
    assert_eq!(
        session.execute(
            "SELECT generate_series(1, 4)",
            QueryLimits::new(2_000, 2, 64 * 1024),
        ),
        Err(QueryExecutionError::ResultLimitExceeded)
    );
    assert_eq!(
        session.execute(
            "SELECT count(*) FROM generate_series(1, 1000000000)",
            QueryLimits::new(100, 10, 64 * 1024),
        ),
        Err(QueryExecutionError::TimeLimitExceeded)
    );
}

#[test]
fn session_driver_cancellation_interrupts_an_in_flight_query() {
    let server = TestServer::start();
    let session = PostgresSession::connect(&server.connection_string()).expect("connect runner");
    let token = CancellationToken::default();
    let worker_token = token.clone();
    let handle = std::thread::spawn(move || {
        let mut session = session;
        session.execute_with_cancel(
            "SELECT count(*) FROM generate_series(1, 1000000000)",
            QueryLimits::new(10_000, 10, 64 * 1024),
            &worker_token,
        )
    });

    std::thread::sleep(std::time::Duration::from_millis(100));
    token.cancel();
    assert_eq!(
        handle.join().expect("query worker").unwrap_err(),
        QueryExecutionError::Cancelled
    );
}

#[test]
fn cancellation_racing_query_start_still_stops_the_query() {
    // A cancel request that reaches an idle backend is ignored, so a token cancelled while the
    // query is still being sent must not let it run on to the statement timeout.
    let server = TestServer::start();
    for step in 0..40_u64 {
        let session =
            PostgresSession::connect(&server.connection_string()).expect("connect runner");
        let token = CancellationToken::default();
        let worker_token = token.clone();
        let started = std::time::Instant::now();
        let handle = std::thread::spawn(move || {
            let mut session = session;
            session.execute_with_cancel(
                "SELECT count(*) FROM generate_series(1, 1000000000)",
                QueryLimits::new(10_000, 10, 64 * 1024),
                &worker_token,
            )
        });
        std::thread::sleep(std::time::Duration::from_micros(step * 500));
        token.cancel();
        assert!(handle.join().expect("query worker").is_err());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "cancel after {}us was lost",
            step * 500
        );
    }
}

#[test]
fn orchestrator_accepts_equivalent_window_query_and_rejects_hardcoded_rows() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let output = OutputPolicy::new(
        RowOrder::Unordered,
        true,
        true,
        vec![
            Column::new("employee_id", LogicalType::Integer),
            Column::new("employee_name", LogicalType::Text),
        ],
    );
    let case = JudgeCase {
        id: "basic",
        schema_sql: "CREATE TABLE employees (id integer, name text, department_id integer, salary integer);",
        fixture_sql: "INSERT INTO employees VALUES (1, 'Alice', 1, 150), (2, 'Bob', 1, 100), (3, 'Carol', 1, 50);",
        reference_sql: "SELECT e.id AS employee_id, e.name AS employee_name FROM employees e WHERE e.salary > (SELECT AVG(peer.salary) FROM employees peer WHERE peer.department_id = e.department_id);",
        output,
        limits: QueryLimits::new(2_000, 100, 64 * 1024),
    };

    let accepted = judge_submission(
        &mut admin,
        "sqljudge_runner",
        std::slice::from_ref(&case),
        "SELECT id AS employee_id, name AS employee_name FROM (SELECT e.*, AVG(e.salary) OVER (PARTITION BY department_id) AS average_salary FROM employees e) e WHERE salary > average_salary",
    );
    assert_eq!(accepted.verdict, Verdict::Accepted);
    assert_eq!(accepted.tests.len(), 1);
    assert_eq!(accepted.tests[0].id, "basic");
    assert!(accepted.tests[0].elapsed_ms < 2_000);

    let rejected = judge_submission(
        &mut admin,
        "sqljudge_runner",
        &[case],
        "SELECT 999 AS employee_id, 'Not Alice' AS employee_name",
    );
    assert_eq!(rejected.verdict, Verdict::WrongAnswer);
}

#[test]
fn orchestrator_maps_reference_failure_to_judge_error() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let report = judge_submission(
        &mut admin,
        "sqljudge_runner",
        &[JudgeCase {
            id: "broken",
            schema_sql: "CREATE TABLE values_table (id integer);",
            fixture_sql: "INSERT INTO values_table VALUES (1);",
            reference_sql: "SELEC 1",
            output: OutputPolicy::new(
                RowOrder::Ordered,
                true,
                true,
                vec![Column::new("id", LogicalType::Integer)],
            ),
            limits: QueryLimits::new(2_000, 100, 64 * 1024),
        }],
        "SELECT id FROM values_table",
    );

    assert_eq!(report.verdict, Verdict::JudgeError);
    assert_eq!(report.tests[0].elapsed_ms, 0, "user SQL never ran");
}

#[test]
fn managed_namespace_provisioning_isolated_and_marker_verified() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let first = ManagedEnvironment::new("env-a", "instance-a", "sj_env_a", "nonce-a")
        .expect("valid managed environment");
    let second = ManagedEnvironment::new("env-b", "instance-b", "sj_env_b", "nonce-b")
        .expect("valid managed environment");

    admin
        .provision_managed_namespace(&first, "sqljudge_runner")
        .expect("provision first namespace");
    admin
        .provision_managed_namespace(&second, "sqljudge_runner")
        .expect("provision second namespace");

    let mut runner = PostgresSession::connect(&server.connection_string()).expect("connect runner");
    let marker = runner
        .execute(
            "SELECT environment_id, instance_id, ownership_nonce FROM \"sj_env_a\".\"__sql_judge_ownership\"",
            QueryLimits::new(2_000, 10, 64 * 1024),
        );
    assert_eq!(marker, Err(QueryExecutionError::RuntimeError));
    let second_marker = admin
        .execute(
            "SELECT environment_id FROM \"sj_env_b\".\"__sql_judge_ownership\"",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("read second marker");
    assert_eq!(second_marker.rows[0][0], SqlValue::Text("env-b".to_owned()));

    let forged =
        ManagedEnvironment::new("env-forged", "instance-forged", "sj_env_a", "nonce-forged")
            .expect("valid forged shape");
    assert_eq!(
        admin.provision_managed_namespace(&forged, "sqljudge_runner"),
        Err(QueryExecutionError::OwnershipVerificationFailed)
    );
}

#[test]
fn managed_namespace_schema_setup_isolated_between_environments() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let first = ManagedEnvironment::new("env-a", "instance-a", "sj_env_a", "nonce-a")
        .expect("valid first environment");
    let second = ManagedEnvironment::new("env-b", "instance-b", "sj_env_b", "nonce-b")
        .expect("valid second environment");
    let schema_sql = "CREATE TABLE employees (name text); INSERT INTO employees VALUES ('first');";

    admin
        .provision_managed_namespace_with_schema(&first, "sqljudge_runner", schema_sql)
        .expect("provision first schema");
    assert!(
        admin
            .managed_namespace_has_schema(&first)
            .expect("inspect first schema")
    );
    admin
        .provision_managed_namespace_with_schema(&second, "sqljudge_runner", schema_sql)
        .expect("provision second schema");

    let mut runner = PostgresSession::connect(&server.connection_string()).expect("connect runner");
    let first_rows = runner
        .execute(
            "SELECT name FROM \"sj_env_a\".employees",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("read first schema");
    let second_rows = admin
        .execute(
            "SELECT name FROM \"sj_env_b\".employees",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("read second schema");

    assert_eq!(first_rows.rows, second_rows.rows);
    assert_eq!(first_rows.rows[0][0], SqlValue::Text("first".to_owned()));
}

#[test]
fn managed_schema_sql_runs_as_constrained_runner_and_cannot_escape_namespace() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let environment = ManagedEnvironment::new(
        "env-constrained-schema",
        "instance-constrained-schema",
        "sj_constrained_schema",
        "nonce-constrained-schema",
    )
    .expect("valid managed environment");

    admin
        .provision_managed_namespace_with_schema(
            &environment,
            "sqljudge_runner",
            "CREATE TABLE employees (id integer);",
        )
        .expect("provision constrained schema");

    let ownership = admin
        .execute(
            "SELECT EXISTS (
                        SELECT 1
                        FROM pg_catalog.pg_class table_entry
                        JOIN pg_catalog.pg_namespace namespace_entry
                          ON namespace_entry.oid = table_entry.relnamespace
                        JOIN pg_catalog.pg_roles role_entry
                          ON role_entry.oid = table_entry.relowner
                        WHERE namespace_entry.nspname = 'sj_constrained_schema'
                          AND table_entry.relname = 'employees'
                          AND role_entry.rolname = 'sqljudge_runner'
                    )
             ",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect constrained schema ownership");
    assert_eq!(ownership.rows, vec![vec![SqlValue::Bool(true)]]);
    let create_privilege = admin
        .execute(
            "SELECT has_schema_privilege('sqljudge_runner', 'sj_constrained_schema', 'CREATE')",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect constrained CREATE privilege");
    let usage_privilege = admin
        .execute(
            "SELECT has_schema_privilege('sqljudge_runner', 'sj_constrained_schema', 'USAGE')",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect constrained USAGE privilege");
    assert_eq!(create_privilege.rows, vec![vec![SqlValue::Bool(false)]]);
    assert_eq!(usage_privilege.rows, vec![vec![SqlValue::Bool(true)]]);

    assert_eq!(
        admin.provision_managed_namespace_with_schema(
            &environment,
            "sqljudge_runner",
            "CREATE TABLE public.pack_escape_probe (id integer);",
        ),
        Err(QueryExecutionError::PolicyViolation)
    );
    let escape_probe = admin
        .execute(
            "SELECT to_regclass('public.pack_escape_probe') IS NULL",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect rejected escape probe");
    assert_eq!(escape_probe.rows, vec![vec![SqlValue::Bool(true)]]);

    admin
        .drop_managed_namespace(&environment)
        .expect("drop constrained namespace");
}

#[test]
fn managed_namespace_drop_requires_owned_marker() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let environment = ManagedEnvironment::new("env-a", "instance-a", "sj_env_a", "nonce-a")
        .expect("valid managed environment");
    admin
        .provision_managed_namespace_with_schema(
            &environment,
            "sqljudge_runner",
            "CREATE TABLE employees (id integer);",
        )
        .expect("provision namespace");
    assert!(
        admin
            .managed_namespace_is_owned(&environment)
            .expect("inspect owned namespace")
    );
    let marker_only = ManagedEnvironment::new(
        "env-marker-only",
        "instance-marker-only",
        "sj_marker_only",
        "nonce-marker-only",
    )
    .expect("valid marker-only environment");
    admin
        .provision_managed_namespace(&marker_only, "sqljudge_runner")
        .expect("provision marker-only namespace");
    assert!(
        !admin
            .managed_namespace_has_schema(&marker_only)
            .expect("inspect marker-only namespace")
    );
    admin
        .drop_managed_namespace(&marker_only)
        .expect("drop marker-only namespace");

    let forged = ManagedEnvironment::new("env-a", "instance-a", "sj_env_a", "forged")
        .expect("valid forged shape");
    assert_eq!(
        admin.drop_managed_namespace(&forged),
        Err(QueryExecutionError::OwnershipVerificationFailed)
    );
    admin
        .drop_managed_namespace(&environment)
        .expect("drop owned namespace");
    assert!(
        !admin
            .managed_namespace_is_owned(&environment)
            .expect("inspect removed namespace")
    );
    assert_eq!(
        admin.execute(
            "SELECT id FROM \"sj_env_a\".employees",
            QueryLimits::new(2_000, 10, 64 * 1024),
        ),
        Err(QueryExecutionError::RuntimeError)
    );
}

#[test]
fn failed_schema_setup_removes_only_the_new_owned_namespace() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let untouched = ManagedEnvironment::new(
        "env-untouched-setup-failure",
        "instance-untouched-setup-failure",
        "sj_untouched_setup_failure",
        "nonce-untouched-setup-failure",
    )
    .expect("valid untouched environment");
    let environment = ManagedEnvironment::new(
        "env-setup-failure",
        "instance-setup-failure",
        "sj_setup_failure",
        "nonce-setup-failure",
    )
    .expect("valid managed environment");
    admin
        .provision_managed_namespace(&untouched, "sqljudge_runner")
        .expect("provision untouched namespace");

    let result = admin.provision_managed_namespace_with_schema(
        &environment,
        "sqljudge_runner",
        "CREATE TABLE employees (id integer); INSERT INTO missing_table VALUES (1);",
    );

    assert_eq!(result, Err(QueryExecutionError::RuntimeError));
    assert!(
        !admin
            .managed_namespace_is_owned(&environment)
            .expect("inspect cleanup namespace")
    );
    assert!(
        admin
            .managed_namespace_is_owned(&untouched)
            .expect("inspect untouched namespace")
    );
    admin
        .drop_managed_namespace(&untouched)
        .expect("drop untouched namespace");
}

#[test]
fn failed_schema_setup_preserves_a_preexisting_owned_namespace() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let environment = ManagedEnvironment::new(
        "env-existing-setup-failure",
        "instance-existing-setup-failure",
        "sj_existing_setup_failure",
        "nonce-existing-setup-failure",
    )
    .expect("valid managed environment");

    admin
        .provision_managed_namespace_with_schema(
            &environment,
            "sqljudge_runner",
            "CREATE TABLE existing_table (id integer);",
        )
        .expect("provision existing namespace");
    let result = admin.provision_managed_namespace_with_schema(
        &environment,
        "sqljudge_runner",
        "CREATE TABLE new_table (id integer); INSERT INTO missing_table VALUES (1);",
    );

    assert_eq!(result, Err(QueryExecutionError::RuntimeError));
    assert!(
        admin
            .managed_namespace_is_owned(&environment)
            .expect("inspect preserved namespace")
    );
    assert_eq!(
        admin
            .execute(
                "SELECT id FROM \"sj_existing_setup_failure\".existing_table",
                QueryLimits::new(2_000, 10, 64 * 1024),
            )
            .expect("read preserved table")
            .rows
            .len(),
        0
    );
}

#[test]
fn cancellation_stops_before_fixture_execution() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let report = judge_submission_with_cancel(
        &mut admin,
        "sqljudge_runner",
        &[JudgeCase {
            id: "cancelled",
            schema_sql: "CREATE TABLE values_table (id integer);",
            fixture_sql: "INSERT INTO values_table VALUES (1);",
            reference_sql: "SELECT id FROM values_table",
            output: OutputPolicy::new(
                RowOrder::Ordered,
                true,
                true,
                vec![Column::new("id", LogicalType::Integer)],
            ),
            limits: QueryLimits::new(2_000, 10, 64 * 1024),
        }],
        "SELECT id FROM values_table",
        || true,
    );

    assert_eq!(report.verdict, Verdict::Cancelled);
    assert!(report.tests.is_empty());
}

#[test]
fn watchdog_timeout_is_distinct_from_user_cancellation() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let started = std::time::Instant::now();
    let report = judge_submission_with_watchdog(
        &mut admin,
        "sqljudge_runner",
        &[JudgeCase {
            id: "watchdog",
            schema_sql: "CREATE TABLE values_table (id integer);",
            fixture_sql: "INSERT INTO values_table VALUES (1);",
            reference_sql: "SELECT count(*) FROM generate_series(1, 1000000000)",
            output: OutputPolicy::new(
                RowOrder::Ordered,
                true,
                true,
                vec![Column::new("count", LogicalType::Bigint)],
            ),
            limits: QueryLimits::new(10_000, 10, 64 * 1024),
        }],
        "SELECT 1",
        std::time::Duration::from_millis(100),
    );

    assert_eq!(report.verdict, Verdict::TimeLimitExceeded);
    assert!(report.tests.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let leftovers = admin
        .execute(
            "SELECT count(*) FROM pg_catalog.pg_namespace WHERE nspname LIKE 'sj_fixture_%'",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("inspect temporary fixture cleanup");
    assert_eq!(leftovers.rows[0][0], SqlValue::Signed(0));
}

#[test]
fn orchestrator_returns_user_result_preview_for_completed_case() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let report = judge_submission(
        &mut admin,
        "sqljudge_runner",
        &[JudgeCase {
            id: "preview",
            schema_sql: "CREATE TABLE values_table (id integer);",
            fixture_sql: "INSERT INTO values_table VALUES (1);",
            reference_sql: "SELECT 1 AS id",
            output: OutputPolicy::new(
                RowOrder::Ordered,
                true,
                true,
                vec![Column::new("id", LogicalType::Integer)],
            ),
            limits: QueryLimits::new(2_000, 10, 64 * 1024),
        }],
        "SELECT 1 AS id",
    );

    let preview = report.preview.expect("completed user preview");
    assert_eq!(preview.columns[0].name, "id");
    assert_eq!(preview.rows, vec![vec![SqlValue::Signed(1)]]);
}

#[test]
fn fixture_setup_cannot_write_to_public_schema_with_admin_session() {
    let server = TestServer::start();
    let mut admin =
        PostgresSession::connect(&server.admin_connection_string()).expect("connect admin");
    let report = judge_submission(
        &mut admin,
        "sqljudge_runner",
        &[JudgeCase {
            id: "escape",
            schema_sql: "CREATE TABLE public.should_not_exist (id integer);",
            fixture_sql: "INSERT INTO public.should_not_exist VALUES (1);",
            reference_sql: "SELECT id FROM public.should_not_exist",
            output: OutputPolicy::new(
                RowOrder::Ordered,
                true,
                true,
                vec![Column::new("id", LogicalType::Integer)],
            ),
            limits: QueryLimits::new(2_000, 10, 64 * 1024),
        }],
        "SELECT id FROM public.should_not_exist",
    );
    assert_eq!(report.verdict, Verdict::JudgeError);
    let table = admin
        .execute(
            "SELECT count(*)::int4 FROM pg_catalog.pg_class WHERE relname = 'should_not_exist'",
            QueryLimits::new(2_000, 1, 64 * 1024),
        )
        .expect("inspect escaped table");
    assert_eq!(table.rows[0][0], SqlValue::Signed(0));
}
