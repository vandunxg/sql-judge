use judge_core::{QueryLimits, SqlValue};
use problem_pack::PackData;
use problem_pack::PackError;
use problem_pack::PackImporter;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

#[test]
fn managed_pack_session_rejects_database_without_runtime_marker() {
    let server = LocalPostgres::start();
    let error = judge_postgres::ManagedPostgresSession::connect(
        &server.connection_string(),
        "test-instance",
    )
    .err()
    .expect("unmarked database must not become a managed pack target");
    assert_eq!(
        error,
        judge_postgres::QueryExecutionError::OwnershipVerificationFailed
    );
}

#[test]
fn validated_pack_exposes_exact_problem_judge_inputs() {
    let (_temp, pack) = copy_sample();
    let validated = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");

    let problem = validated
        .problem("above-average")
        .expect("load problem")
        .expect("problem exists");

    assert_eq!(problem.id, "above-average");
    assert_eq!(problem.tests.len(), 4);
    assert_eq!(problem.tests[0].id, "basic");
    assert!(problem.schema_sql.contains("CREATE TABLE"));
    assert!(problem.reference_sql.contains("SELECT"));
    assert!(problem.tests[0].fixture_sql.contains("INSERT"));

    let catalog = validated.catalog_entry().expect("catalog entry");
    let catalog_problem = &catalog.problems[0];
    assert_eq!(catalog_problem.schema[0].name, "departments");
    assert_eq!(catalog_problem.sample_rows[0]["employee_id"], 1);
    assert!(
        catalog_problem
            .starter_sql
            .as_deref()
            .is_some_and(|sql| sql.contains("SELECT e.id"))
    );
}

#[test]
fn directory_pack_rejects_excessive_relative_path_depth_and_length() {
    let temp = tempfile_like::TempDir::new();
    let deep = (0..40).map(|_| "nested").collect::<Vec<_>>().join("/");
    let deep_path = temp.path().join(&deep);
    fs::create_dir_all(&deep_path).expect("create deep path");
    fs::write(deep_path.join("file.sql"), "SELECT 1;").expect("write deep file");
    assert!(matches!(
        PackData::from_directory(temp.path()),
        Err(PackError::ResourceLimit)
    ));

    let temp = tempfile_like::TempDir::new();
    let long_path = (0..16)
        .map(|_| "x".repeat(40))
        .collect::<Vec<_>>()
        .join("/");
    let long_directory = temp.path().join(&long_path);
    fs::create_dir_all(&long_directory).expect("create long path");
    fs::write(long_directory.join("file.sql"), "SELECT 1;").expect("write long file");
    assert!(matches!(
        PackData::from_directory(temp.path()),
        Err(PackError::ResourceLimit)
    ));
}

#[test]
fn directory_pack_rejects_hardlinked_files() {
    let temp = tempfile_like::TempDir::new();
    let original = temp.path().join("original.sql");
    let linked = temp.path().join("linked.sql");
    fs::write(&original, "SELECT 1;").expect("write original");
    fs::hard_link(&original, &linked).expect("create hard link");

    assert!(matches!(
        PackData::from_directory(temp.path()),
        Err(PackError::HardLink(_))
    ));
}

#[test]
fn zip_pack_rejects_excessive_entry_count_and_path_depth() {
    let entries = (0..2_100)
        .map(|index| (format!("files/{index}.sql"), b"SELECT 1;".to_vec()))
        .collect::<Vec<_>>();
    let entries = entries
        .iter()
        .map(|(name, content)| (name.as_str(), content.clone()))
        .collect();
    let (_temp, archive) = write_zip(entries);
    assert!(matches!(
        PackData::from_path(archive),
        Err(PackError::ResourceLimit)
    ));

    let entries = (0..4_100)
        .map(|index| (format!("entries/{index}.sql"), b"SELECT 1;".to_vec()))
        .collect::<Vec<_>>();
    let entries = entries
        .iter()
        .map(|(name, content)| (name.as_str(), content.clone()))
        .collect();
    let (_temp, archive) = write_zip(entries);
    assert!(matches!(
        PackData::from_path(archive),
        Err(PackError::ResourceLimit)
    ));

    let deep = (0..40).map(|_| "nested").collect::<Vec<_>>().join("/");
    let (_temp, archive) = write_zip(vec![(&format!("{deep}/file.sql"), b"SELECT 1;".to_vec())]);
    assert!(matches!(
        PackData::from_path(archive),
        Err(PackError::ResourceLimit)
    ));
}

#[test]
fn import_into_managed_environment_provisions_pack_schema() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.join("registry");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let environment = judge_postgres::ManagedEnvironment::new(
        "env-import",
        "test-instance",
        "sj_import_a",
        "nonce-import",
    )
    .expect("valid managed environment");

    PackImporter::new(&registry)
        .import_into_managed_environment(&data, &mut session, &environment, "sqljudge_runner")
        .expect("validate, provision, and publish pack");

    let result = session
        .execute(
            "SELECT COUNT(*) FROM \"sj_import_a\".employees",
            QueryLimits::new(2_000, 10, 64 * 1024),
        )
        .expect("read provisioned schema");
    assert_eq!(result.rows[0][0], SqlValue::Signed(0));
}

#[test]
fn failed_managed_schema_provisioning_does_not_publish_pack() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let environment = judge_postgres::ManagedEnvironment::new(
        "env-import",
        "test-instance",
        "sj_import_a",
        "nonce-import",
    )
    .expect("valid managed environment");
    let first_registry = pack.join("registry-first");
    PackImporter::new(&first_registry)
        .import_into_managed_environment(&data, &mut session, &environment, "sqljudge_runner")
        .expect("first import");

    let second_registry = pack.join("registry-second");
    assert!(matches!(
        PackImporter::new(&second_registry).import_into_managed_environment(
            &data,
            &mut session,
            &environment,
            "sqljudge_runner"
        ),
        Err(problem_pack::PackError::EnvironmentProvisioning)
    ));
    assert!(!second_registry.exists());
}

#[test]
fn publish_conflict_does_not_provision_a_managed_schema() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let installed_registry = pack
        .parent()
        .expect("pack parent")
        .join("registry-installed");
    PackImporter::new(&installed_registry)
        .import(&data, &mut session, "sqljudge_runner")
        .expect("install original pack");

    fs::write(pack.join("README.md"), "different immutable content")
        .expect("change optional content");
    let changed = PackData::from_directory(&pack)
        .expect("load changed pack")
        .validate()
        .expect("changed pack remains structurally valid");
    let environment = judge_postgres::ManagedEnvironment::new(
        "env-conflict",
        "test-instance",
        "sj_import_conflict",
        "nonce-conflict",
    )
    .expect("valid managed environment");

    let result = PackImporter::new(&installed_registry).import_into_managed_environment(
        &changed,
        &mut session,
        &environment,
        "sqljudge_runner",
    );
    assert!(matches!(
        result,
        Err(problem_pack::PackError::ImmutableVersionConflict { .. })
    ));
    let namespace_exists = session
        .execute(
            "SELECT EXISTS (
                 SELECT 1 FROM pg_catalog.pg_namespace WHERE nspname = 'sj_import_conflict'
             )",
            QueryLimits::new(2_000, 2, 64 * 1024),
        )
        .expect("check managed namespace");
    assert_eq!(namespace_exists.rows[0][0], SqlValue::Bool(false));
}

fn sample_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/packs/hr-analytics")
}

fn copy_sample() -> (tempfile_like::TempDir, PathBuf) {
    let temp = tempfile_like::TempDir::new();
    let destination = temp.path().join("pack");
    copy_tree(&sample_path(), &destination);
    (temp, destination)
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination");
    for entry in fs::read_dir(source).expect("read source") {
        let entry = entry.expect("read entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

fn write_zip(entries: Vec<(&str, Vec<u8>)>) -> (tempfile_like::TempDir, PathBuf) {
    let temp = tempfile_like::TempDir::new();
    let archive = temp.path().join("fixture.zip");
    let file = fs::File::create(&archive).expect("create archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in entries {
        writer
            .start_file(name, options)
            .expect("start archive entry");
        writer.write_all(&content).expect("write archive entry");
    }
    writer.finish().expect("finish archive");
    (temp, archive)
}

fn write_stored_zip(entries: Vec<(&str, Vec<u8>)>) -> (tempfile_like::TempDir, PathBuf) {
    let temp = tempfile_like::TempDir::new();
    let archive = temp.path().join("fixture-stored.zip");
    let file = fs::File::create(&archive).expect("create archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in entries {
        writer
            .start_file(name, options)
            .expect("start archive entry");
        writer.write_all(&content).expect("write archive entry");
    }
    writer.finish().expect("finish archive");
    (temp, archive)
}

fn bytes_for_zip(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index % 251) as u8).collect()
}

#[test]
fn sample_pack_passes_strict_structural_validation() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(pack).expect("load sample pack");
    data.validate().expect("validate sample pack");
}

#[test]
fn checked_in_zip_matches_the_sample_pack_directory() {
    let archive =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hr-analytics.sqljudge.zip");
    let from_zip = PackData::from_path(archive)
        .expect("load checked-in zip")
        .validate()
        .expect("checked-in zip must satisfy the v1 contract");
    let from_directory = PackData::from_directory(sample_path())
        .expect("load checked-in directory")
        .validate()
        .expect("checked-in directory must satisfy the v1 contract");
    assert_eq!(from_zip.digest(), from_directory.digest());
    assert_eq!(from_zip.id(), "hr-analytics");
    assert_eq!(from_zip.version(), "1.2.0");
}

#[test]
fn sample_pack_catalog_matches_the_solve_design_example() {
    let catalog = PackData::from_directory(sample_path())
        .expect("load sample pack")
        .validate()
        .expect("validate sample pack")
        .catalog_entry()
        .expect("catalog entry");
    assert_eq!(catalog.title, "HR Analytics");
    assert_eq!(catalog.problems.len(), 8);
    assert!(catalog.problems.iter().all(|problem| {
        problem.fixture_count >= 3
            && !problem.sample_rows.is_empty()
            && !problem.sample_tables.is_empty()
            && problem.starter_sql.is_some()
    }));
    let encoded = serde_json::to_value(&catalog).expect("encode catalog");
    let second = encoded["problems"]
        .as_array()
        .expect("problems")
        .iter()
        .find(|problem| problem["id"] == "second-highest-salary")
        .expect("second highest salary problem");
    assert_eq!(second["title"], "Second highest salary per department");
    assert_eq!(second["difficulty"], "medium");
    assert_eq!(
        second["topics"],
        serde_json::json!(["window-functions", "ranking"])
    );
    assert_eq!(
        second["outputColumns"],
        serde_json::json!([
            {"name": "department_name", "type": "text"},
            {"name": "second_salary", "type": "numeric"}
        ])
    );
    assert_eq!(second["rowOrder"], "unordered");
    assert_eq!(second["timeLimitMs"], 2000);
    assert_eq!(second["fixtureCount"], 4);
    assert_eq!(
        second["sampleTables"],
        serde_json::json!([
            {"name": "employees", "rows": [
                {"id": 1, "name": "Linh", "department_id": 1, "salary": 135000},
                {"id": 2, "name": "Minh", "department_id": 1, "salary": 120000},
                {"id": 3, "name": "Hoa", "department_id": 1, "salary": 120000},
                {"id": 4, "name": "Quan", "department_id": 2, "salary": 98000},
                {"id": 5, "name": "Trang", "department_id": 3, "salary": 87000},
                {"id": 6, "name": "Bao", "department_id": 3, "salary": 72000}
            ]},
            {"name": "departments", "rows": [
                {"id": 1, "name": "Engineering"},
                {"id": 2, "name": "Finance"},
                {"id": 3, "name": "Design"}
            ]}
        ])
    );
    assert_eq!(
        second["sampleRows"],
        serde_json::json!([
            {"department_name": "Engineering", "second_salary": 120000},
            {"department_name": "Finance", "second_salary": null},
            {"department_name": "Design", "second_salary": 72000}
        ])
    );
}

#[test]
fn pack_schema_rejects_cross_namespace_and_database_qualified_identifiers() {
    let (_temp, pack) = copy_sample();
    fs::write(
        pack.join("schema/001_tables.sql"),
        "CREATE TABLE sj_other.evil (id integer)",
    )
    .expect("write unsafe schema");
    let data = PackData::from_directory(pack).expect("load pack");
    assert!(matches!(
        data.validate(),
        Err(PackError::InvalidSql(path)) if path.contains("schema/001_tables.sql")
    ));
}

#[test]
fn registry_lookup_returns_the_validated_schema_for_an_imported_non_default_pack() {
    let (temp, pack) = copy_sample();
    let manifest_path = pack.join("pack.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["id"] = serde_json::Value::String("cte-junior".to_owned());
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    let validated = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("validate pack");
    let registry = temp.path().join("registry");
    let published = registry.join("sha256").join(validated.digest());
    copy_tree(&pack, &published);

    let loaded = PackImporter::new(&registry)
        .load_validated("cte-junior")
        .expect("lookup pack")
        .expect("pack is installed");
    assert_eq!(loaded.id(), "cte-junior");
    assert!(
        loaded
            .schema_sql()
            .expect("schema SQL")
            .contains("CREATE TABLE")
    );
}

#[test]
fn sample_zip_pack_passes_strict_structural_validation() {
    let archive =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hr-analytics.sqljudge.zip");
    let data = PackData::from_zip(&archive).expect("load sample zip pack");
    data.validate().expect("validate sample zip pack");
}

#[test]
fn pack_path_loader_accepts_directory_and_zip_sources() {
    let (_temp, pack) = copy_sample();
    PackData::from_path(&pack)
        .expect("load directory source")
        .validate()
        .expect("validate directory source");

    let archive =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hr-analytics.sqljudge.zip");
    PackData::from_path(&archive)
        .expect("load zip source")
        .validate()
        .expect("validate zip source");
}

#[test]
fn zip_path_traversal_is_rejected_before_validation() {
    let (_temp, archive) = write_zip(vec![("../escape.sql", b"SELECT 1".to_vec())]);
    assert!(matches!(
        PackData::from_zip(archive),
        Err(PackError::UnsafePath(_))
    ));
}

#[test]
fn zip_compression_bomb_is_rejected_before_allocation() {
    let (_temp, archive) = write_zip(vec![("payload.sql", vec![b'a'; 1024 * 1024])]);
    assert!(matches!(
        PackData::from_zip(archive),
        Err(PackError::ResourceLimit)
    ));
}

#[test]
fn zip_pack_rejects_cumulative_compressed_size() {
    let content = bytes_for_zip(900_000);
    let entries = (0..10)
        .map(|index| (format!("files/{index}.sql"), content.clone()))
        .collect::<Vec<_>>();
    let entries = entries
        .iter()
        .map(|(name, content)| (name.as_str(), content.clone()))
        .collect();
    let (_temp, archive) = write_stored_zip(entries);

    assert!(matches!(
        PackData::from_path(archive),
        Err(PackError::ResourceLimit)
    ));
}

#[test]
fn zip_pack_rejects_link_like_non_regular_entries() {
    let temp = tempfile_like::TempDir::new();
    let archive = temp.path().join("link-like.zip");
    let file = fs::File::create(&archive).expect("create archive");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().external_attributes(0o120777 << 16);
    writer
        .start_file("link.sql", options)
        .expect("start archive entry");
    writer
        .write_all(b"target.sql")
        .expect("write archive entry");
    writer.finish().expect("finish archive");

    assert!(matches!(
        PackData::from_path(archive),
        Err(PackError::Symlink(_))
    ));
}

#[test]
fn malformed_zip_is_rejected() {
    let temp = tempfile_like::TempDir::new();
    let archive = temp.path().join("malformed.zip");
    fs::write(&archive, b"not a zip archive").expect("write malformed archive");
    assert!(PackData::from_zip(archive).is_err());
}

#[test]
fn unknown_manifest_fields_are_rejected() {
    let (_temp, pack) = copy_sample();
    let manifest = pack.join("pack.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest).expect("read manifest"))
            .expect("parse manifest");
    value["unexpected"] = serde_json::Value::Bool(true);
    fs::write(
        &manifest,
        serde_json::to_vec(&value).expect("encode manifest"),
    )
    .expect("write manifest");

    let data = PackData::from_directory(pack).expect("load pack");
    assert!(data.validate().is_err());
}

#[test]
fn mismatched_dialect_is_rejected() {
    let (_temp, pack) = copy_sample();
    let problem = pack.join("problems/above-average/problem.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&problem).expect("read problem"))
            .expect("parse problem");
    value["dialect"] = serde_json::Value::String("sqlserver".to_owned());
    fs::write(
        &problem,
        serde_json::to_vec(&value).expect("encode problem"),
    )
    .expect("write problem");

    let data = PackData::from_directory(pack).expect("load pack");
    assert!(data.validate().is_err());
}

#[test]
fn finder_and_explorer_metadata_files_are_ignored_in_directory_packs() {
    let (_temp, pack) = copy_sample();
    let clean = PackData::from_directory(&pack)
        .expect("load clean pack")
        .validate()
        .expect("validate clean pack");
    fs::write(pack.join(".DS_Store"), b"\0\0\0\x01Bud1").expect("write .DS_Store");
    fs::write(pack.join("problems/._pack.json"), b"AppleDouble").expect("write AppleDouble");
    fs::write(pack.join("schema/Thumbs.db"), b"thumbs").expect("write Thumbs.db");
    fs::write(pack.join("desktop.ini"), b"[.ShellClassInfo]").expect("write desktop.ini");

    let with_metadata = PackData::from_directory(&pack)
        .expect("load pack with OS metadata")
        .validate()
        .expect("OS metadata files are not pack content");
    assert_eq!(with_metadata.digest(), clean.digest());
}

#[test]
fn finder_compressed_zip_metadata_entries_are_ignored() {
    let mut entries = Vec::new();
    collect_files(&sample_path(), "", &mut entries);
    let clean_names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
    entries.push((".DS_Store".to_owned(), b"\0\0\0\x01Bud1".to_vec()));
    for name in &clean_names {
        entries.push((
            format!("__MACOSX/{}", apple_double(name)),
            b"AppleDouble".to_vec(),
        ));
    }
    let (_temp, archive) = write_zip(
        entries
            .iter()
            .map(|(name, content)| (name.as_str(), content.clone()))
            .collect(),
    );

    let from_zip = PackData::from_zip(archive)
        .expect("load Finder zip")
        .validate()
        .expect("Finder metadata entries are not pack content");
    let from_directory = PackData::from_directory(sample_path())
        .expect("load sample directory")
        .validate()
        .expect("validate sample directory");
    assert_eq!(from_zip.digest(), from_directory.digest());
}

#[test]
fn metadata_names_do_not_hide_traversal_or_symlinks() {
    let (_temp, archive) = write_zip(vec![("../.DS_Store", b"x".to_vec())]);
    assert!(PackData::from_zip(archive).is_err());

    let (_temp, pack) = copy_sample();
    std::os::unix::fs::symlink("/etc/passwd", pack.join("schema/.DS_Store"))
        .expect("create metadata-named symlink");
    assert!(matches!(
        PackData::from_directory(pack),
        Err(PackError::Symlink(_))
    ));
}

fn collect_files(root: &Path, prefix: &str, entries: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(root).expect("read source") {
        let entry = entry.expect("read entry");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().expect("entry type").is_dir() {
            collect_files(&entry.path(), &format!("{name}/"), entries);
        } else {
            entries.push((name, fs::read(entry.path()).expect("read file")));
        }
    }
}

fn apple_double(name: &str) -> String {
    match name.rsplit_once('/') {
        Some((parent, file)) => format!("{parent}/._{file}"),
        None => format!("._{name}"),
    }
}

#[test]
fn traversal_and_symlink_sources_are_rejected() {
    let (_temp, pack) = copy_sample();
    std::os::unix::fs::symlink(pack.join("pack.json"), pack.join("escape.json"))
        .expect("create symlink");
    assert!(PackData::from_directory(pack).is_err());
}

#[test]
fn case_colliding_files_are_rejected() {
    let (_temp, pack) = copy_sample();
    fs::write(pack.join("README.MD"), "collision").expect("write colliding file");
    let data = PackData::from_directory(pack).expect("load pack");
    assert!(data.validate().is_err());
}

#[test]
fn duplicate_problem_ids_are_rejected() {
    let (_temp, pack) = copy_sample();
    let duplicate = pack.join("problems/duplicate/problem.json");
    fs::create_dir_all(duplicate.parent().expect("duplicate parent")).expect("create problem");
    fs::copy(pack.join("problems/above-average/problem.json"), &duplicate).expect("copy problem");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(pack.join("pack.json")).expect("read manifest"))
            .expect("parse manifest");
    manifest["problems"] = serde_json::json!([
        "problems/above-average/problem.json",
        "problems/duplicate/problem.json"
    ]);
    fs::write(
        pack.join("pack.json"),
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");

    let data = PackData::from_directory(pack).expect("load pack");
    assert!(data.validate().is_err());
}

#[test]
fn failed_runtime_validation_leaves_no_published_or_staged_artifact() {
    let (_temp, pack) = copy_sample();
    fs::write(
        pack.join("problems/above-average/reference.sql"),
        "SELEC broken",
    )
    .expect("break reference");
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.join("registry");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();

    assert!(
        PackImporter::new(&registry)
            .import(&data, &mut session, "sqljudge_runner")
            .is_err()
    );
    assert!(!registry.exists());
}

#[test]
fn successful_runtime_validation_publishes_immutable_digest_and_is_idempotent() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.join("registry");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let importer = PackImporter::new(&registry);

    let first = importer
        .import(&data, &mut session, "sqljudge_runner")
        .expect("runtime validation and publish");
    assert!(!first.already_installed);
    assert!(first.path.is_dir());
    assert!(first.path.join("pack.json").is_file());

    let second = importer
        .import(&data, &mut session, "sqljudge_runner")
        .expect("same content is an idempotent import");
    assert!(second.already_installed);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.path, second.path);

    fs::write(first.path.join("README.md"), "tampered installed content")
        .expect("tamper installed content");
    assert!(
        importer
            .import(&data, &mut session, "sqljudge_runner")
            .is_err()
    );
}

#[test]
fn exact_registry_lookup_does_not_fall_back_to_latest_pack() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.parent().expect("pack parent").join("registry-exact");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let published = PackImporter::new(&registry)
        .import(&data, &mut session, "sqljudge_runner")
        .expect("publish pack");
    let importer = PackImporter::new(&registry);

    assert!(
        importer
            .load_validated_exact(data.id(), data.version(), &published.digest)
            .expect("exact lookup")
            .is_some()
    );
    assert!(
        importer
            .load_validated_exact(data.id(), "9.9.9", &published.digest)
            .expect("wrong version lookup")
            .is_none()
    );
}

#[test]
fn validated_catalog_exposes_non_default_pack_problem_metadata_only() {
    let (_temp, pack) = copy_sample();
    let manifest_path = pack.join("pack.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["id"] = serde_json::Value::String("catalog-sql-101".to_owned());
    fs::write(
        manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.parent().expect("pack parent").join("registry-catalog");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    PackImporter::new(&registry)
        .import(&data, &mut session, "sqljudge_runner")
        .expect("publish pack");

    let catalog = PackImporter::new(&registry)
        .list_catalog()
        .expect("list validated catalog");
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].pack_id, "catalog-sql-101");
    assert!(!catalog[0].problems.is_empty());
    assert_eq!(catalog[0].problems[0].schema.len(), 2);
    assert_eq!(catalog[0].problems[0].sample_rows.len(), 2);
    assert!(catalog[0].problems[0].starter_sql.is_some());
    assert!(
        !serde_json::to_string(&catalog)
            .unwrap()
            .contains("CREATE TABLE")
    );
}

#[test]
fn catalog_starter_sql_allows_newlines_and_tabs_only() {
    for accepted in [
        "SELECT e.id\nFROM employees AS e;",
        "SELECT e.id\r\nFROM employees AS e\r\n\tWHERE e.id > 0;",
    ] {
        let (_temp, pack) = copy_sample();
        rewrite_above_average(&pack, |problem| {
            problem["catalog"]["starter_sql"] = serde_json::json!(accepted);
        });
        assert_eq!(
            above_average_catalog(&pack)["starterSql"],
            serde_json::json!(accepted)
        );
    }
    for rejected in [
        "SELECT 1\u{0000}",
        "SELECT 1\rFROM employees",
        "SELECT 1\u{001b}[31m",
        "SELECT 1\u{000b}",
        "SELECT 1\u{0085}",
    ] {
        let (_temp, pack) = copy_sample();
        rewrite_above_average(&pack, |problem| {
            problem["catalog"]["starter_sql"] = serde_json::json!(rejected);
        });
        assert!(
            matches!(
                PackData::from_directory(&pack)
                    .expect("load pack")
                    .validate(),
                Err(PackError::InvalidManifest(_))
            ),
            "{rejected:?} must be rejected"
        );
    }
}

#[test]
fn catalog_sample_strings_still_reject_newlines() {
    let (_temp, pack) = copy_sample();
    rewrite_above_average(&pack, |problem| {
        problem["catalog"]["sample_rows"] =
            serde_json::json!([{"employee_id": 1, "employee_name": "Linh\nTran"}]);
    });
    assert!(matches!(
        PackData::from_directory(&pack)
            .expect("load pack")
            .validate(),
        Err(PackError::InvalidManifest(_))
    ));
}

#[test]
fn invalid_catalog_metadata_is_rejected_before_publication() {
    let (_temp, pack) = copy_sample();
    let problem_path = pack.join("problems/above-average/problem.json");
    let mut problem: serde_json::Value =
        serde_json::from_slice(&fs::read(&problem_path).expect("read problem"))
            .expect("parse problem");
    problem["catalog"]["sample_rows"] = serde_json::json!([{
        "employee_id": {"nested": "value"}
    }]);
    fs::write(
        problem_path,
        serde_json::to_vec(&problem).expect("encode problem"),
    )
    .expect("write problem");

    let error = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect_err("nested sample values must not cross the catalog boundary");
    assert!(matches!(error, PackError::InvalidManifest(_)));
}

fn rewrite_above_average(pack: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let problem_path = pack.join("problems/above-average/problem.json");
    let mut problem: serde_json::Value =
        serde_json::from_slice(&fs::read(&problem_path).expect("read problem"))
            .expect("parse problem");
    change(&mut problem);
    fs::write(
        problem_path,
        serde_json::to_vec(&problem).expect("encode problem"),
    )
    .expect("write problem");
}

fn above_average_catalog(pack: &Path) -> serde_json::Value {
    let catalog = PackData::from_directory(pack)
        .expect("load pack")
        .validate()
        .expect("structural validation")
        .catalog_entry()
        .expect("catalog entry");
    let encoded = serde_json::to_value(&catalog).expect("encode catalog");
    encoded["problems"]
        .as_array()
        .expect("problems array")
        .iter()
        .find(|problem| problem["id"] == "above-average")
        .expect("above-average problem")
        .clone()
}

#[test]
fn catalog_entry_exposes_display_metadata_and_bounded_sample_tables() {
    let (_temp, pack) = copy_sample();
    rewrite_above_average(&pack, |problem| {
        problem["limits"]["statement_timeout_ms"] = serde_json::json!(1500);
        problem["catalog"]["sample_tables"] = serde_json::json!([
            {"name": "departments", "rows": [{"id": 1, "name": "Engineering"}]},
            {"name": "employees", "rows": [
                {"id": 1, "name": "Linh", "department_id": 1, "salary": 135000},
                {"id": 2, "name": "Minh", "department_id": 1, "salary": null}
            ]}
        ]);
    });

    let problem = above_average_catalog(&pack);
    assert_eq!(
        problem["outputColumns"],
        serde_json::json!([
            {"name": "employee_id", "type": "integer"},
            {"name": "employee_name", "type": "text"}
        ])
    );
    assert_eq!(problem["rowOrder"], "unordered");
    assert_eq!(problem["timeLimitMs"], 1500);
    assert_eq!(problem["fixtureCount"], 4);
    assert_eq!(
        problem["sampleTables"],
        serde_json::json!([
            {"name": "departments", "rows": [{"id": 1, "name": "Engineering"}]},
            {"name": "employees", "rows": [
                {"id": 1, "name": "Linh", "department_id": 1, "salary": 135000},
                {"id": 2, "name": "Minh", "department_id": 1, "salary": null}
            ]}
        ])
    );
    let encoded = problem.to_string();
    assert!(!encoded.contains("INSERT"));
    assert!(!encoded.contains("tests/"));
}

#[test]
fn catalog_sample_tables_default_to_empty() {
    let (_temp, pack) = copy_sample();
    rewrite_above_average(&pack, |problem| {
        problem["catalog"]
            .as_object_mut()
            .expect("catalog object")
            .remove("sample_tables");
    });
    assert_eq!(
        above_average_catalog(&pack)["sampleTables"],
        serde_json::json!([])
    );
}

#[test]
fn invalid_catalog_sample_tables_are_rejected() {
    let row = serde_json::json!({"id": 1, "name": "Engineering"});
    let too_many_tables = (0..9)
        .map(|index| serde_json::json!({"name": format!("t{index}"), "rows": []}))
        .collect::<Vec<_>>();
    let cases = vec![
        (
            "too many rows",
            serde_json::json!([{"name": "departments", "rows": vec![row.clone(); 21]}]),
            false,
        ),
        ("too many tables", serde_json::json!(too_many_tables), true),
        (
            "unknown table",
            serde_json::json!([{"name": "salaries", "rows": [row.clone()]}]),
            false,
        ),
        (
            "invalid table name",
            serde_json::json!([{"name": "bad-name", "rows": [row.clone()]}]),
            true,
        ),
        (
            "duplicate table",
            serde_json::json!([
                {"name": "departments", "rows": []},
                {"name": "departments", "rows": []}
            ]),
            false,
        ),
        (
            "nested value",
            serde_json::json!([{"name": "departments", "rows": [{"id": {"nested": 1}}]}]),
            false,
        ),
        (
            "invalid column key",
            serde_json::json!([{"name": "departments", "rows": [{"bad key": 1}]}]),
            false,
        ),
    ];
    for (label, sample_tables, drop_schema) in cases {
        let (_temp, pack) = copy_sample();
        rewrite_above_average(&pack, |problem| {
            if drop_schema {
                problem["catalog"]
                    .as_object_mut()
                    .expect("catalog object")
                    .remove("schema");
            }
            problem["catalog"]["sample_tables"] = sample_tables;
        });
        let result = PackData::from_directory(&pack)
            .expect("load pack")
            .validate();
        assert!(
            matches!(result, Err(PackError::InvalidManifest(_))),
            "{label} must be rejected as an invalid manifest, got {result:?}"
        );
    }

    let (_temp, pack) = copy_sample();
    rewrite_above_average(&pack, |problem| {
        problem["catalog"]["sample_tables"] =
            serde_json::json!([{"name": "departments", "rows": [], "evil": true}]);
    });
    assert!(matches!(
        PackData::from_directory(&pack)
            .expect("load pack")
            .validate(),
        Err(PackError::InvalidJson { .. })
    ));
}

#[test]
fn validated_catalog_rejects_missing_or_invalid_registry_entries() {
    let (_temp, pack) = copy_sample();
    let registry = pack.parent().expect("pack parent").join("registry-invalid");
    assert!(
        PackImporter::new(&registry)
            .list_catalog()
            .expect("missing registry is empty")
            .is_empty()
    );
    let invalid = registry.join("sha256/not-a-digest");
    fs::create_dir_all(&invalid).expect("create invalid registry entry");
    fs::write(invalid.join("pack.json"), b"{}").expect("write invalid manifest");
    assert!(PackImporter::new(&registry).list_catalog().is_err());
}

#[test]
fn different_content_cannot_replace_an_installed_pack_version() {
    let (_temp, pack) = copy_sample();
    let data = PackData::from_directory(&pack)
        .expect("load pack")
        .validate()
        .expect("structural validation");
    let registry = pack.parent().expect("pack parent").join("registry");
    let server = LocalPostgres::start();
    let mut session = server.admin_session();
    let importer = PackImporter::new(&registry);
    importer
        .import(&data, &mut session, "sqljudge_runner")
        .expect("first import");

    fs::write(pack.join("README.md"), "different immutable content")
        .expect("change optional content");
    let changed = PackData::from_directory(&pack)
        .expect("load changed pack")
        .validate()
        .expect("changed pack remains structurally valid");
    assert!(
        importer
            .import(&changed, &mut session, "sqljudge_runner")
            .is_err()
    );
    assert_eq!(
        fs::read_dir(registry.join("sha256"))
            .expect("read registry")
            .count(),
        1
    );
}

struct LocalPostgres {
    root: PathBuf,
    socket: PathBuf,
    port: u16,
}

impl LocalPostgres {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sql-judge-pack-pg-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let data = root.join("data");
        let socket = root.join("socket");
        fs::create_dir_all(&socket).expect("create socket directory");
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("reserve port")
            .local_addr()
            .expect("read port")
            .port();
        let admin = std::env::var("USER").expect("USER identifies test owner");

        assert!(
            Command::new("initdb")
                .args([
                    "-D",
                    data.to_str().expect("data path"),
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
                    data.to_str().expect("data path"),
                    "-o",
                    &format!(
                        "-h 127.0.0.1 -p {port} -k {} -c listen_addresses=127.0.0.1",
                        socket.to_str().expect("socket path")
                    ),
                    "-w",
                    "start",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("start postgres")
                .success()
        );
        assert!(
            Command::new("createdb")
                .args([
                    "-h",
                    socket.to_str().expect("socket path"),
                    "-p",
                    &port.to_string(),
                    "sqljudge_test",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("create database")
                .success()
        );

        let mut psql = Command::new("psql")
            .args([
                "-h",
                socket.to_str().expect("socket path"),
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
            .expect("start setup psql");
        psql.stdin
            .take()
            .expect("open psql stdin")
            .write_all(
                b"CREATE ROLE sqljudge_runner LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT; REVOKE CREATE ON SCHEMA public FROM PUBLIC;",
            )
            .expect("write setup");
        assert!(psql.wait().expect("wait setup").success());
        Self { root, socket, port }
    }

    fn connection_string(&self) -> String {
        format!(
            "host={} port={} user={} dbname=sqljudge_test",
            self.socket.to_str().expect("socket path"),
            self.port,
            std::env::var("USER").expect("USER identifies test owner")
        )
    }

    fn mark_runtime(&self) {
        let sql = "CREATE TABLE public.__sql_judge_runtime (instance_id text PRIMARY KEY); INSERT INTO public.__sql_judge_runtime (instance_id) VALUES ('test-instance'); REVOKE ALL ON TABLE public.__sql_judge_runtime FROM PUBLIC;";
        assert!(
            Command::new("psql")
                .args([
                    "-h",
                    self.socket.to_str().expect("socket path"),
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
                .status()
                .expect("mark managed runtime")
                .success()
        );
    }

    fn admin_session(&self) -> judge_postgres::ManagedPostgresSession {
        self.mark_runtime();
        judge_postgres::ManagedPostgresSession::connect(&self.connection_string(), "test-instance")
            .expect("connect managed test target")
    }
}

impl Drop for LocalPostgres {
    fn drop(&mut self) {
        let _ = Command::new("pg_ctl")
            .args([
                "-D",
                self.root.join("data").to_str().expect("data path"),
                "-w",
                "stop",
                "-m",
                "fast",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos()
}

mod tempfile_like {
    use std::path::{Path, PathBuf};

    pub struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sql-judge-pack-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).expect("create temp directory");
            Self { path }
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
