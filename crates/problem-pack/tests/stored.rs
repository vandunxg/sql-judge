use problem_pack::{
    PackData, PackImporter, ValidatedPack, catalog_from_records, pack_records, problem_from_record,
};
use std::path::{Path, PathBuf};

fn sample_pack() -> ValidatedPack {
    PackData::from_path(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/packs/hr-analytics"),
    )
    .expect("load sample pack")
    .validate()
    .expect("validate sample pack")
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create destination");
    for entry in std::fs::read_dir(source).expect("read source") {
        let entry = entry.expect("source entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sql-judge-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn validated_pack_round_trips_through_stored_records() {
    let pack = sample_pack();
    let (record, problems) = pack_records(&pack).expect("convert pack to records");

    assert_eq!(record.digest, pack.digest());
    assert_eq!(record.pack_id, "hr-analytics");
    assert_eq!(record.version, pack.version());
    assert_eq!(record.schema_sql, pack.schema_sql().expect("pack schema"));

    let catalog = pack.catalog_entry().expect("pack catalog");
    assert_eq!(
        problems
            .iter()
            .map(|problem| problem.problem_id.as_str())
            .collect::<Vec<_>>(),
        catalog
            .problems
            .iter()
            .map(|problem| problem.id.as_str())
            .collect::<Vec<_>>()
    );
    let catalog_json = problems
        .iter()
        .map(|problem| problem.catalog_json.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        catalog_from_records(&record, &catalog_json).expect("catalog from records"),
        catalog
    );

    for stored in &problems {
        let expected = pack
            .problem(&stored.problem_id)
            .expect("load validated problem")
            .expect("problem exists");
        let restored = problem_from_record(&record, stored).expect("problem from record");
        assert_eq!(restored.id, expected.id);
        assert_eq!(restored.dialect, expected.dialect);
        assert_eq!(restored.schema_sql, expected.schema_sql);
        assert_eq!(restored.reference_sql, expected.reference_sql);
        assert_eq!(restored.output, expected.output);
        assert_eq!(restored.limits, expected.limits);
        assert_eq!(
            restored
                .tests
                .iter()
                .map(|test| (&test.id, test.hidden, &test.fixture_sql))
                .collect::<Vec<_>>(),
            expected
                .tests
                .iter()
                .map(|test| (&test.id, test.hidden, &test.fixture_sql))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn corrupt_stored_records_are_rejected() {
    let pack = sample_pack();
    let (record, problems) = pack_records(&pack).expect("convert pack to records");
    let mut corrupt = problems[0].clone();
    corrupt.output_json = "{not json".to_owned();
    assert!(problem_from_record(&record, &corrupt).is_err());
    let mut unknown_dialect = problems[0].clone();
    unknown_dialect.dialect = "oracle".to_owned();
    assert!(problem_from_record(&record, &unknown_dialect).is_err());
    assert!(catalog_from_records(&record, &["{not json"]).is_err());
}

#[test]
fn published_registry_packs_load_individually_for_migration() {
    let root = temp_root("legacy-registry");
    let pack = sample_pack();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/packs/hr-analytics"),
        &root.join("sha256").join(pack.digest()),
    );
    let mislabeled = "0".repeat(64);
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/packs/hr-analytics"),
        &root.join("sha256").join(&mislabeled),
    );

    let mut loaded = PackImporter::new(&root)
        .load_published()
        .expect("scan legacy registry");
    loaded.sort_by(|left, right| left.0.cmp(&right.0));

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].0, mislabeled);
    assert!(loaded[0].1.is_err(), "a digest mismatch must not migrate");
    assert_eq!(loaded[1].0, pack.digest());
    assert_eq!(
        loaded[1].1.as_ref().expect("valid published pack").digest(),
        pack.digest()
    );
    assert!(
        PackImporter::new(root.join("missing"))
            .load_published()
            .expect("missing registry")
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(root);
}
