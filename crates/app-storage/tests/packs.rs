use app_storage::{
    HistoryStore, PackInsert, PackProblemRecord, PackRecord, PackStore, PackTestRecord,
    StorageError,
};

fn temp_database(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("sql-judge-{name}-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn digest(seed: char) -> String {
    seed.to_string().repeat(64)
}

fn pack(seed: char, pack_id: &str, version: &str) -> PackRecord {
    PackRecord {
        digest: digest(seed),
        pack_id: pack_id.to_owned(),
        version: version.to_owned(),
        title: format!("{pack_id} {version}"),
        dialect: "postgresql".to_owned(),
        schema_sql: "CREATE TABLE employees (id integer);".to_owned(),
    }
}

fn problem(problem_id: &str, tests: &[(&str, bool)]) -> PackProblemRecord {
    PackProblemRecord {
        problem_id: problem_id.to_owned(),
        catalog_json: format!(r#"{{"id":"{problem_id}"}}"#),
        dialect: "postgresql".to_owned(),
        reference_sql: "SELECT id FROM employees".to_owned(),
        output_json: r#"{"row_order":"unordered"}"#.to_owned(),
        statement_timeout_ms: 2_000,
        max_rows: 100,
        max_bytes: 65_536,
        tests: tests
            .iter()
            .map(|(test_id, hidden)| PackTestRecord {
                test_id: (*test_id).to_owned(),
                hidden: *hidden,
                fixture_sql: format!("INSERT INTO employees VALUES (1); -- {test_id}"),
            })
            .collect(),
    }
}

#[test]
fn installed_pack_round_trips_problems_and_tests_in_declared_order() {
    let path = temp_database("packs-roundtrip");
    let problems = vec![
        problem(
            "second",
            &[("basic", false), ("hidden-b", true), ("hidden-a", true)],
        ),
        problem("first", &[("basic", false)]),
    ];
    {
        let mut store = PackStore::open(&path).expect("open pack store");
        assert_eq!(
            store
                .insert_pack(&pack('a', "hr", "1.0.0"), &problems)
                .expect("insert pack"),
            PackInsert::Inserted
        );
    }

    let store = PackStore::open(&path).expect("reopen pack store");
    assert_eq!(
        store.find_pack(&digest('a')).expect("find pack"),
        Some(pack('a', "hr", "1.0.0"))
    );
    assert_eq!(
        store
            .find_problem(&digest('a'), "second")
            .expect("find problem"),
        Some(problems[0].clone())
    );
    assert_eq!(
        store
            .find_problem(&digest('a'), "missing")
            .expect("find missing"),
        None
    );
    assert_eq!(
        store.list_problem_catalog().expect("list catalog"),
        vec![
            (digest('a'), r#"{"id":"second"}"#.to_owned()),
            (digest('a'), r#"{"id":"first"}"#.to_owned()),
        ]
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn installed_versions_are_immutable_and_reinstall_is_idempotent() {
    let path = temp_database("packs-immutable");
    let mut store = PackStore::open(&path).expect("open pack store");
    let original = vec![problem("first", &[("basic", false)])];
    store
        .insert_pack(&pack('a', "hr", "1.0.0"), &original)
        .expect("insert pack");

    assert_eq!(
        store
            .insert_pack(&pack('a', "hr", "1.0.0"), &original)
            .expect("reinstall same digest"),
        PackInsert::AlreadyInstalled
    );
    assert_eq!(
        store
            .insert_pack(
                &pack('b', "hr", "1.0.0"),
                &[problem("other", &[("basic", false)])]
            )
            .expect("conflicting version"),
        PackInsert::VersionConflict
    );
    assert_eq!(
        store.find_pack(&digest('b')).expect("find conflicting"),
        None
    );
    assert_eq!(
        store
            .insert_pack(&pack('c', "hr", "1.1.0"), &original)
            .expect("insert newer version"),
        PackInsert::Inserted
    );
    assert_eq!(
        store
            .list_packs()
            .expect("list packs")
            .iter()
            .map(|pack| pack.version.as_str())
            .collect::<Vec<_>>(),
        vec!["1.1.0", "1.0.0"]
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn deleting_a_pack_removes_its_problems_and_tests_only() {
    let path = temp_database("packs-delete");
    let mut store = PackStore::open(&path).expect("open pack store");
    store
        .insert_pack(
            &pack('a', "hr", "1.0.0"),
            &[problem("first", &[("basic", false)])],
        )
        .expect("insert hr");
    store
        .insert_pack(
            &pack('b', "sales", "1.0.0"),
            &[problem("first", &[("basic", false)])],
        )
        .expect("insert sales");

    assert!(store.delete_pack(&digest('a')).expect("delete hr"));
    assert!(!store.delete_pack(&digest('a')).expect("delete hr again"));

    assert_eq!(store.find_pack(&digest('a')).expect("find deleted"), None);
    assert_eq!(
        store
            .find_problem(&digest('a'), "first")
            .expect("find deleted problem"),
        None
    );
    assert!(
        store
            .find_problem(&digest('b'), "first")
            .expect("find kept")
            .is_some()
    );
    assert_eq!(
        store.list_problem_catalog().expect("list catalog"),
        vec![(digest('b'), r#"{"id":"first"}"#.to_owned())]
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_pack_records_are_rejected_without_partial_writes() {
    let path = temp_database("packs-invalid");
    let mut store = PackStore::open(&path).expect("open pack store");
    let mut bad_digest = pack('a', "hr", "1.0.0");
    bad_digest.digest = "not-a-digest".to_owned();
    let mut empty_title = pack('a', "hr", "1.0.0");
    empty_title.title = " ".to_owned();
    for (record, problems) in [
        (bad_digest, vec![problem("first", &[("basic", false)])]),
        (empty_title, vec![problem("first", &[("basic", false)])]),
        (
            pack('a', "hr", "1.0.0"),
            vec![problem("", &[("basic", false)])],
        ),
        (
            pack('a', "hr", "1.0.0"),
            vec![problem("first", &[("", false)])],
        ),
        (
            pack('a', "hr", "1.0.0"),
            vec![
                problem("dup", &[("basic", false)]),
                problem("dup", &[("basic", false)]),
            ],
        ),
    ] {
        assert!(matches!(
            store.insert_pack(&record, &problems),
            Err(StorageError::InvalidRecord(_))
        ));
    }
    assert!(store.list_packs().expect("list packs").is_empty());
    assert!(
        store
            .list_problem_catalog()
            .expect("list catalog")
            .is_empty()
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pack_store_shares_the_database_with_history() {
    let path = temp_database("packs-shared");
    drop(HistoryStore::open(&path).expect("open history first"));
    let mut store = PackStore::open(&path).expect("open pack store");
    store
        .insert_pack(
            &pack('a', "hr", "1.0.0"),
            &[problem("first", &[("basic", false)])],
        )
        .expect("insert pack");
    drop(store);
    let history = HistoryStore::open(&path).expect("reopen history");
    assert!(history.list_submissions().expect("list history").is_empty());
    assert_eq!(
        PackStore::open(&path)
            .expect("reopen packs")
            .list_packs()
            .expect("list packs")
            .len(),
        1
    );
    drop(history);
    let _ = std::fs::remove_file(path);
}
