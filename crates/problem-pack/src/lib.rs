use judge_core::{Column, Dialect, OutputPolicy, QueryLimits, RowOrder};
use judge_postgres::{ManagedEnvironment, ManagedPostgresSession, validate_setup_sql};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

mod stored;

pub use stored::{catalog_from_records, pack_records, problem_from_record};

const MAX_FILES: usize = 2_000;
const MAX_ENTRIES: usize = 4_096;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_COMPRESSED_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 1_000;
const MAX_RELATIVE_PATH_BYTES: usize = 512;
const MAX_PATH_DEPTH: usize = 16;

#[derive(Debug, Clone)]
pub struct PackData {
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct ValidatedPack {
    files: BTreeMap<String, Vec<u8>>,
    manifest: PackManifest,
    digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackCatalogEntry {
    pub pack_id: String,
    pub version: String,
    pub title: String,
    pub dialect: String,
    pub digest: String,
    pub problems: Vec<ProblemCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemCatalogEntry {
    pub id: String,
    pub title: String,
    pub difficulty: String,
    pub dialect: String,
    pub topics: Vec<String>,
    pub statement: String,
    pub schema: Vec<CatalogTable>,
    pub sample_rows: Vec<BTreeMap<String, serde_json::Value>>,
    pub starter_sql: Option<String>,
    pub output_columns: Vec<Column>,
    pub row_order: RowOrder,
    pub time_limit_ms: u32,
    pub fixture_count: usize,
    pub sample_tables: Vec<CatalogSampleTable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSampleTable {
    pub name: String,
    pub rows: Vec<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogTable {
    pub name: String,
    pub columns: Vec<CatalogColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogColumn {
    pub name: String,
    pub r#type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone)]
pub struct ValidatedProblem {
    pub id: String,
    pub dialect: Dialect,
    pub schema_sql: String,
    pub reference_sql: String,
    pub output: OutputPolicy,
    pub limits: QueryLimits,
    pub tests: Vec<ValidatedTest>,
}

#[derive(Debug, Clone)]
pub struct ValidatedTest {
    pub id: String,
    pub hidden: bool,
    pub fixture_sql: String,
}

impl ValidatedPack {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn schema_sql(&self) -> Result<String, PackError> {
        self.manifest
            .schema
            .iter()
            .map(|path| sql_file(&self.files, path))
            .collect::<Result<Vec<_>, _>>()
            .map(|files| files.join("\n"))
    }

    pub fn problem(&self, problem_id: &str) -> Result<Option<ValidatedProblem>, PackError> {
        let Some(problem_path) = self.manifest.problems.iter().find(|path| {
            json_file::<ProblemManifest>(&self.files, path)
                .map(|problem| problem.id == problem_id)
                .unwrap_or(false)
        }) else {
            return Ok(None);
        };
        let problem: ProblemManifest = json_file(&self.files, problem_path)?;
        let reference_path = sibling_path(problem_path, &problem.reference);
        let tests = problem
            .tests
            .iter()
            .map(|test| {
                let fixture_path = sibling_path(problem_path, &test.file);
                Ok(ValidatedTest {
                    id: test.id.clone(),
                    hidden: test.hidden,
                    fixture_sql: sql_file(&self.files, &fixture_path)?,
                })
            })
            .collect::<Result<Vec<_>, PackError>>()?;

        Ok(Some(ValidatedProblem {
            id: problem.id,
            dialect: problem.dialect,
            schema_sql: self.schema_sql()?,
            reference_sql: sql_file(&self.files, &reference_path)?,
            output: OutputPolicy::new(
                problem.output.row_order,
                problem.output.strict_column_names,
                problem.output.strict_column_types,
                problem.output.columns,
            ),
            limits: problem.limits,
            tests,
        }))
    }

    pub fn catalog_entry(&self) -> Result<PackCatalogEntry, PackError> {
        let problems = self
            .manifest
            .problems
            .iter()
            .map(|path| {
                let problem: ProblemManifest = json_file(&self.files, path)?;
                let schema = catalog_schema(&problem)?;
                let sample_rows = catalog_sample_rows(&problem)?;
                let starter_sql = catalog_starter_sql(&problem)?;
                let sample_tables = catalog_sample_tables(&problem)?;
                Ok(ProblemCatalogEntry {
                    id: problem.id,
                    title: problem.title,
                    difficulty: format!("{:?}", problem.difficulty).to_lowercase(),
                    dialect: dialect_name(problem.dialect),
                    topics: problem.topics,
                    statement: problem.statement,
                    schema,
                    sample_rows,
                    starter_sql,
                    output_columns: problem.output.columns,
                    row_order: problem.output.row_order,
                    time_limit_ms: problem.limits.statement_timeout_ms,
                    fixture_count: problem.tests.len(),
                    sample_tables,
                })
            })
            .collect::<Result<Vec<_>, PackError>>()?;
        Ok(PackCatalogEntry {
            pack_id: self.id().to_owned(),
            version: self.version().to_owned(),
            title: self.manifest.title.clone(),
            dialect: dialect_name(self.manifest.dialect),
            digest: self.digest().to_owned(),
            problems,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPack {
    pub digest: String,
    pub path: PathBuf,
    pub already_installed: bool,
}

#[derive(Debug)]
pub struct PackImporter {
    registry: PathBuf,
}

#[derive(Debug, Error)]
pub enum PackError {
    #[error("pack source must be a real directory")]
    SourceNotDirectory,
    #[error("pack contains a symlink: {0}")]
    Symlink(String),
    #[error("pack contains a hard link: {0}")]
    HardLink(String),
    #[error("pack contains an unsafe path: {0}")]
    UnsafePath(String),
    #[error("pack contains a non-regular file: {0}")]
    NonRegularFile(String),
    #[error("pack contains duplicate or case-colliding path: {0}")]
    DuplicatePath(String),
    #[error("pack resource limits exceeded")]
    ResourceLimit,
    #[error("pack filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("pack ZIP archive is invalid")]
    Zip(#[from] zip::result::ZipError),
    #[error("invalid JSON in {file}")]
    InvalidJson { file: String },
    #[error("invalid pack manifest: {0}")]
    InvalidManifest(String),
    #[error("missing pack file: {0}")]
    MissingFile(String),
    #[error("undeclared pack file: {0}")]
    UndeclaredFile(String),
    #[error("invalid SQL file: {0}")]
    InvalidSql(String),
    #[error("pack dialect is not supported by the managed judge")]
    UnsupportedDialect,
    #[error("runtime validation failed for {problem}/{test}")]
    RuntimeValidation { problem: String, test: String },
    #[error("managed environment provisioning failed")]
    EnvironmentProvisioning,
    #[error("immutable pack version conflict for {pack_id} {version}")]
    ImmutableVersionConflict { pack_id: String, version: String },
    #[error("published pack destination is not a directory")]
    InvalidPublishedDestination,
    #[error("pack publication failed")]
    Publication,
    #[error("installed pack record is invalid: {0}")]
    InvalidStoredRecord(&'static str),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackManifest {
    spec_version: String,
    id: String,
    version: String,
    title: String,
    dialect: Dialect,
    #[serde(default)]
    min_engine_version: Option<String>,
    schema: Vec<String>,
    problems: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProblemManifest {
    id: String,
    title: String,
    difficulty: Difficulty,
    topics: Vec<String>,
    statement: String,
    dialect: Dialect,
    reference: String,
    output: OutputManifest,
    limits: QueryLimits,
    tests: Vec<TestManifest>,
    #[serde(default)]
    catalog: Option<CatalogManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogManifest {
    #[serde(default)]
    schema: Vec<CatalogTableManifest>,
    #[serde(default)]
    sample_rows: Vec<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    starter_sql: Option<String>,
    #[serde(default)]
    sample_tables: Vec<CatalogSampleTableManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSampleTableManifest {
    name: String,
    rows: Vec<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogTableManifest {
    name: String,
    columns: Vec<CatalogColumnManifest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogColumnManifest {
    name: String,
    r#type: String,
    #[serde(default)]
    nullable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputManifest {
    row_order: RowOrder,
    strict_column_names: bool,
    strict_column_types: bool,
    columns: Vec<Column>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestManifest {
    id: String,
    file: String,
    hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct InstalledIdentity {
    id: String,
    version: String,
}

impl PackData {
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, PackError> {
        let root = path.as_ref();
        let metadata = fs::symlink_metadata(root)?;
        if !metadata.is_dir() {
            return Err(PackError::SourceNotDirectory);
        }

        let mut files = BTreeMap::new();
        let mut entry_count = 0;
        collect_directory(root, root, &mut files, &mut entry_count, 0)?;
        Ok(Self { files })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PackError> {
        let path = path.as_ref();
        let metadata = fs::symlink_metadata(path)?;
        if metadata.is_dir() {
            return Self::from_directory(path);
        }
        if metadata.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        {
            return Self::from_zip(path);
        }
        Err(PackError::SourceNotDirectory)
    }

    pub fn from_zip(path: impl AsRef<Path>) -> Result<Self, PackError> {
        let file = fs::File::open(path)?;
        let mut archive = ZipArchive::new(file)?;
        if archive.len() > MAX_ENTRIES {
            return Err(PackError::ResourceLimit);
        }

        let mut entries = Vec::new();
        let mut total_bytes = 0usize;
        let mut compressed_bytes = 0usize;
        let mut file_count = 0usize;
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            let raw_name = entry.name()?.into_owned();
            let name = raw_name.trim_end_matches('/');
            compressed_bytes = compressed_bytes
                .checked_add(
                    usize::try_from(entry.compressed_size())
                        .map_err(|_| PackError::ResourceLimit)?,
                )
                .ok_or(PackError::ResourceLimit)?;
            if compressed_bytes > MAX_COMPRESSED_BYTES {
                return Err(PackError::ResourceLimit);
            }
            validate_path(name)?;
            if name.split('/').count() > MAX_PATH_DEPTH {
                return Err(PackError::ResourceLimit);
            }
            if entry.is_symlink() {
                return Err(PackError::Symlink(raw_name));
            }
            if entry.is_dir() || is_os_metadata(name) {
                continue;
            }
            if !entry.is_file() {
                return Err(PackError::NonRegularFile(raw_name));
            }

            file_count += 1;
            if file_count > MAX_FILES {
                return Err(PackError::ResourceLimit);
            }
            let uncompressed_size = entry.size();
            let compressed_size = entry.compressed_size();
            if uncompressed_size > MAX_FILE_BYTES as u64
                || (uncompressed_size > 0
                    && (compressed_size == 0
                        || compressed_size
                            .checked_mul(MAX_COMPRESSION_RATIO)
                            .is_none_or(|limit| uncompressed_size > limit)))
            {
                return Err(PackError::ResourceLimit);
            }
            total_bytes = total_bytes
                .checked_add(uncompressed_size as usize)
                .ok_or(PackError::ResourceLimit)?;
            if total_bytes > MAX_TOTAL_BYTES {
                return Err(PackError::ResourceLimit);
            }

            let mut content = Vec::with_capacity(uncompressed_size as usize);
            entry
                .take((MAX_FILE_BYTES + 1) as u64)
                .read_to_end(&mut content)?;
            if content.len() > MAX_FILE_BYTES {
                return Err(PackError::ResourceLimit);
            }
            entries.push((name.to_owned(), content));
        }

        Self::from_entries(entries)
    }

    pub fn from_entries<I>(entries: I) -> Result<Self, PackError>
    where
        I: IntoIterator<Item = (String, Vec<u8>)>,
    {
        let mut files = BTreeMap::new();
        let mut folded = BTreeSet::new();
        for (name, content) in entries {
            validate_path(&name)?;
            if !folded.insert(name.to_lowercase()) {
                return Err(PackError::DuplicatePath(name));
            }
            if content.len() > MAX_FILE_BYTES {
                return Err(PackError::ResourceLimit);
            }
            files.insert(name, content);
            if files.len() > MAX_FILES {
                return Err(PackError::ResourceLimit);
            }
        }
        if files.values().map(Vec::len).sum::<usize>() > MAX_TOTAL_BYTES {
            return Err(PackError::ResourceLimit);
        }
        Ok(Self { files })
    }

    pub fn validate(&self) -> Result<ValidatedPack, PackError> {
        if self.files.is_empty()
            || self.files.len() > MAX_FILES
            || self.files.values().map(Vec::len).sum::<usize>() > MAX_TOTAL_BYTES
        {
            return Err(PackError::ResourceLimit);
        }
        for (name, content) in &self.files {
            validate_path(name)?;
            if content.len() > MAX_FILE_BYTES {
                return Err(PackError::ResourceLimit);
            }
        }
        let mut folded = BTreeSet::new();
        for name in self.files.keys() {
            if !folded.insert(name.to_lowercase()) {
                return Err(PackError::DuplicatePath(name.clone()));
            }
        }

        let manifest: PackManifest = json_file(&self.files, "pack.json")?;
        if manifest.spec_version != "sqljudge.pack/v1"
            || !is_id(&manifest.id)
            || !is_semver(&manifest.version)
            || !(3..=140).contains(&manifest.title.chars().count())
            || manifest.schema.is_empty()
            || manifest.schema.len() > 30
            || manifest.problems.is_empty()
            || manifest.problems.len() > 500
        {
            return Err(PackError::InvalidManifest(
                "pack.json contract violation".to_owned(),
            ));
        }
        if manifest
            .min_engine_version
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.chars().count() > 30)
        {
            return Err(PackError::InvalidManifest(
                "invalid minimum engine version".to_owned(),
            ));
        }

        let mut expected = BTreeSet::from(["pack.json".to_owned()]);
        let mut schema_paths = BTreeSet::new();
        for path in &manifest.schema {
            if !is_schema_path(path) || !schema_paths.insert(path.clone()) {
                return Err(PackError::InvalidManifest(
                    "invalid schema path list".to_owned(),
                ));
            }
            expected.insert(path.clone());
            check_sql(&self.files, path)?;
            let schema_sql = sql_file(&self.files, path)?;
            validate_setup_sql(&schema_sql).map_err(|_| PackError::InvalidSql(path.clone()))?;
        }

        let mut problem_paths = BTreeSet::new();
        let mut problem_ids = BTreeSet::new();
        for path in &manifest.problems {
            if !is_problem_path(path) || !problem_paths.insert(path.clone()) {
                return Err(PackError::InvalidManifest(
                    "invalid problem path list".to_owned(),
                ));
            }
            expected.insert(path.clone());
            let problem: ProblemManifest = json_file(&self.files, path)?;
            validate_problem(
                &self.files,
                &manifest,
                path,
                &problem,
                &mut expected,
                &mut problem_ids,
            )?;
        }

        for path in self.files.keys() {
            if !expected.contains(path) && path != "README.md" && path != "LICENSE" {
                return Err(PackError::UndeclaredFile(path.clone()));
            }
        }

        Ok(ValidatedPack {
            files: self.files.clone(),
            manifest,
            digest: content_digest(&self.files),
        })
    }
}

/// One legacy registry directory: its folder name and the pack it validated to, if any.
pub type PublishedPackLoad = (String, Result<ValidatedPack, PackError>);

/// Runs every reference test in disposable fixtures without publishing anything (ADR-025).
pub fn validate_pack_runtime(
    pack: &ValidatedPack,
    session: &mut ManagedPostgresSession,
    runner_role: &str,
) -> Result<(), PackError> {
    if pack.manifest.dialect != Dialect::Postgresql {
        return Err(PackError::UnsupportedDialect);
    }
    validate_runtime(pack, session, runner_role)
}

impl PackImporter {
    pub fn new(registry: impl Into<PathBuf>) -> Self {
        Self {
            registry: registry.into(),
        }
    }

    pub fn import(
        &self,
        pack: &ValidatedPack,
        session: &mut ManagedPostgresSession,
        runner_role: &str,
    ) -> Result<PublishedPack, PackError> {
        if pack.manifest.dialect != Dialect::Postgresql {
            return Err(PackError::UnsupportedDialect);
        }
        validate_runtime(pack, session, runner_role)?;
        self.publish(pack)
    }

    /// Loads every published registry directory on its own, so one corrupt pack cannot block
    /// the others. Each entry pairs the directory name with its validated pack.
    pub fn load_published(&self) -> Result<Vec<PublishedPackLoad>, PackError> {
        let digest_root = self.registry.join("sha256");
        if !digest_root.is_dir() {
            return Ok(Vec::new());
        }
        let mut packs = Vec::new();
        for entry in fs::read_dir(digest_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let digest = entry.file_name().to_string_lossy().into_owned();
            let pack = PackData::from_directory(entry.path())
                .and_then(|data| data.validate())
                .and_then(|pack| {
                    if pack.digest() == digest {
                        Ok(pack)
                    } else {
                        Err(PackError::InvalidPublishedDestination)
                    }
                });
            packs.push((digest, pack));
        }
        Ok(packs)
    }

    pub fn load_validated(&self, pack_id: &str) -> Result<Option<ValidatedPack>, PackError> {
        if !is_id(pack_id) {
            return Err(PackError::InvalidManifest(
                "invalid requested pack ID".to_owned(),
            ));
        }
        let digest_root = self.registry.join("sha256");
        if !digest_root.is_dir() {
            return Ok(None);
        }

        let mut selected: Option<ValidatedPack> = None;
        for entry in fs::read_dir(&digest_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let data = PackData::from_directory(entry.path())?;
            let pack = data.validate()?;
            if pack.digest() != entry.file_name().to_string_lossy() {
                return Err(PackError::InvalidPublishedDestination);
            }
            if pack.id() != pack_id {
                continue;
            }
            if let Some(existing) = &selected {
                match version_cmp(pack.version(), existing.version()) {
                    std::cmp::Ordering::Greater => selected = Some(pack),
                    std::cmp::Ordering::Equal if pack.digest() != existing.digest() => {
                        return Err(PackError::ImmutableVersionConflict {
                            pack_id: pack.id().to_owned(),
                            version: pack.version().to_owned(),
                        });
                    }
                    _ => {}
                }
            } else {
                selected = Some(pack);
            }
        }
        Ok(selected)
    }

    pub fn load_validated_exact(
        &self,
        pack_id: &str,
        version: &str,
        digest: &str,
    ) -> Result<Option<ValidatedPack>, PackError> {
        if !is_id(pack_id) || !is_pack_digest(digest) || version.trim().is_empty() {
            return Err(PackError::InvalidManifest(
                "invalid requested pack identity".to_owned(),
            ));
        }
        let destination = self.registry.join("sha256").join(digest);
        if !destination.is_dir() {
            return Ok(None);
        }
        let pack = PackData::from_directory(destination)?.validate()?;
        if pack.digest() != digest {
            return Err(PackError::InvalidPublishedDestination);
        }
        if pack.id() != pack_id || pack.version() != version {
            return Ok(None);
        }
        Ok(Some(pack))
    }

    pub fn list_catalog(&self) -> Result<Vec<PackCatalogEntry>, PackError> {
        let digest_root = self.registry.join("sha256");
        if !digest_root.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(digest_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let digest = entry.file_name().to_string_lossy().into_owned();
            if !is_pack_digest(&digest) {
                return Err(PackError::InvalidPublishedDestination);
            }
            let pack = PackData::from_directory(entry.path())?.validate()?;
            if pack.digest() != digest {
                return Err(PackError::InvalidPublishedDestination);
            }
            entries.push(pack.catalog_entry()?);
        }
        entries.sort_by(|left, right| {
            left.pack_id
                .cmp(&right.pack_id)
                .then_with(|| right.version.cmp(&left.version))
        });
        Ok(entries)
    }

    pub fn import_into_managed_environment(
        &self,
        pack: &ValidatedPack,
        session: &mut ManagedPostgresSession,
        environment: &ManagedEnvironment,
        runner_role: &str,
    ) -> Result<PublishedPack, PackError> {
        if pack.manifest.dialect != Dialect::Postgresql {
            return Err(PackError::UnsupportedDialect);
        }
        validate_runtime(pack, session, runner_role)?;
        let schema_sql = pack
            .manifest
            .schema
            .iter()
            .map(|path| sql_file(&pack.files, path))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        let published = self.publish(pack)?;
        if session
            .provision_managed_namespace_with_schema(environment, runner_role, &schema_sql)
            .is_err()
        {
            if !published.already_installed && fs::remove_dir_all(&published.path).is_err() {
                return Err(PackError::Publication);
            }
            if !published.already_installed
                && let Some(digest_root) = published.path.parent()
            {
                let _ = fs::remove_dir(digest_root);
                if let Some(registry) = digest_root.parent() {
                    let _ = fs::remove_dir(registry);
                }
            }
            return Err(PackError::EnvironmentProvisioning);
        }
        Ok(published)
    }

    fn publish(&self, pack: &ValidatedPack) -> Result<PublishedPack, PackError> {
        let digest_root = self.registry.join("sha256");
        let destination = digest_root.join(&pack.digest);
        if let Some(metadata) = symlink_metadata_if_present(&destination)? {
            if !metadata.is_dir() {
                return Err(PackError::InvalidPublishedDestination);
            }
            let installed = PackData::from_directory(&destination)
                .map_err(|_| PackError::InvalidPublishedDestination)?;
            if content_digest(&installed.files) != pack.digest {
                return Err(PackError::InvalidPublishedDestination);
            }
            return Ok(PublishedPack {
                digest: pack.digest.clone(),
                path: destination,
                already_installed: true,
            });
        }

        if digest_root.is_dir() {
            for entry in fs::read_dir(&digest_root)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let identity_path = entry.path().join("pack.json");
                if !identity_path.is_file() {
                    continue;
                }
                let identity: InstalledIdentity =
                    match serde_json::from_slice(&fs::read(identity_path)?) {
                        Ok(identity) => identity,
                        Err(_) => continue,
                    };
                if identity.id == pack.manifest.id && identity.version == pack.manifest.version {
                    return Err(PackError::ImmutableVersionConflict {
                        pack_id: identity.id,
                        version: identity.version,
                    });
                }
            }
        }

        let root_existed = self.registry.exists();
        let digest_root_existed = digest_root.exists();
        let stage = self.registry.join(format!(
            ".staging-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let result = (|| {
            fs::create_dir_all(&digest_root)?;
            fs::create_dir(&stage)?;
            for (name, content) in &pack.files {
                let target = stage.join(name);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(target, content)?;
            }
            fs::rename(&stage, &destination).map_err(|_| PackError::Publication)
        })();

        if result.is_err() {
            let _ = fs::remove_dir_all(&stage);
            if !digest_root_existed {
                let _ = fs::remove_dir(&digest_root);
            }
            if !root_existed {
                let _ = fs::remove_dir(&self.registry);
            }
        }
        result.map(|_| PublishedPack {
            digest: pack.digest.clone(),
            path: destination,
            already_installed: false,
        })
    }
}

fn validate_runtime(
    pack: &ValidatedPack,
    session: &mut ManagedPostgresSession,
    runner_role: &str,
) -> Result<(), PackError> {
    let schema_sql = pack
        .manifest
        .schema
        .iter()
        .map(|path| sql_file(&pack.files, path))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");

    for problem_path in &pack.manifest.problems {
        let problem: ProblemManifest = json_file(&pack.files, problem_path)?;
        let reference_path = sibling_path(problem_path, &problem.reference);
        let reference = sql_file(&pack.files, &reference_path)?;
        for test in &problem.tests {
            let test_path = sibling_path(problem_path, &test.file);
            let fixture = sql_file(&pack.files, &test_path)?;
            let result =
                session.with_managed_fixture(runner_role, &schema_sql, &fixture, |session| {
                    session.execute(&reference, problem.limits)
                });
            let result = result.map_err(|_| PackError::RuntimeValidation {
                problem: problem.id.clone(),
                test: test.id.clone(),
            })?;
            if result.columns != problem.output.columns || result.validate().is_err() {
                return Err(PackError::RuntimeValidation {
                    problem: problem.id.clone(),
                    test: test.id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_problem(
    files: &BTreeMap<String, Vec<u8>>,
    pack: &PackManifest,
    path: &str,
    problem: &ProblemManifest,
    expected: &mut BTreeSet<String>,
    problem_ids: &mut BTreeSet<String>,
) -> Result<(), PackError> {
    let directory = path
        .rsplit('/')
        .nth(1)
        .ok_or_else(|| PackError::InvalidManifest("problem path has no parent".to_owned()))?;
    if directory != problem.id
        || !is_id(&problem.id)
        || !problem_ids.insert(problem.id.clone())
        || !(3..=140).contains(&problem.title.chars().count())
        || problem.topics.is_empty()
        || problem.topics.len() > 15
        || problem.statement.chars().count() < 20
        || problem.statement.chars().count() > 12_000
        || problem.dialect != pack.dialect
        || problem.reference != "reference.sql"
        || problem.output.columns.is_empty()
        || problem.output.columns.len() > 128
        || problem.tests.len() < 3
        || problem.tests.len() > 50
    {
        return Err(PackError::InvalidManifest(format!(
            "problem contract violation: {path}"
        )));
    }
    if !problem.topics.iter().all(|topic| is_id(topic))
        || problem.topics.iter().collect::<BTreeSet<_>>().len() != problem.topics.len()
        || problem
            .output
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != problem.output.columns.len()
    {
        return Err(PackError::InvalidManifest(format!(
            "problem uniqueness violation: {path}"
        )));
    }
    match problem.difficulty {
        Difficulty::Easy | Difficulty::Medium | Difficulty::Hard => {}
    }
    let output_policy = OutputPolicy::new(
        problem.output.row_order,
        problem.output.strict_column_names,
        problem.output.strict_column_types,
        problem.output.columns.clone(),
    );
    output_policy
        .validate()
        .map_err(|error| PackError::InvalidManifest(error.to_string()))?;
    problem
        .output
        .columns
        .iter()
        .try_for_each(validate_column)?;
    problem
        .limits
        .validate()
        .map_err(|error| PackError::InvalidManifest(error.to_string()))?;
    validate_catalog(problem)?;

    let reference_path = sibling_path(path, &problem.reference);
    expected.insert(reference_path.clone());
    check_sql(files, &reference_path)?;

    let mut test_ids = BTreeSet::new();
    let mut test_paths = BTreeSet::new();
    let mut visible = 0;
    let mut hidden = 0;
    for test in &problem.tests {
        if !is_id(&test.id)
            || !is_test_path(&test.file)
            || !test_ids.insert(test.id.clone())
            || !test_paths.insert(test.file.clone())
        {
            return Err(PackError::InvalidManifest(format!(
                "test contract violation: {path}"
            )));
        }
        if test.hidden {
            hidden += 1;
        } else {
            visible += 1;
        }
        let test_path = sibling_path(path, &test.file);
        expected.insert(test_path.clone());
        check_sql(files, &test_path)?;
    }
    if visible < 1 || hidden < 2 {
        return Err(PackError::InvalidManifest(format!(
            "fixture visibility violation: {path}"
        )));
    }
    Ok(())
}

const MAX_CATALOG_TABLES: usize = 64;
const MAX_CATALOG_COLUMNS: usize = 128;
const MAX_CATALOG_ROWS: usize = 100;
const MAX_CATALOG_ROW_COLUMNS: usize = 128;
const MAX_CATALOG_CELL_BYTES: usize = 4 * 1024;
const MAX_CATALOG_STARTER_BYTES: usize = 64 * 1024;
const MAX_CATALOG_SAMPLE_TABLES: usize = 8;
const MAX_CATALOG_SAMPLE_TABLE_ROWS: usize = 20;

fn validate_catalog(problem: &ProblemManifest) -> Result<(), PackError> {
    let Some(catalog) = &problem.catalog else {
        return Ok(());
    };
    if catalog.schema.len() > MAX_CATALOG_TABLES
        || catalog.sample_rows.len() > MAX_CATALOG_ROWS
        || catalog.sample_tables.len() > MAX_CATALOG_SAMPLE_TABLES
        || catalog
            .sample_tables
            .iter()
            .any(|table| table.rows.len() > MAX_CATALOG_SAMPLE_TABLE_ROWS)
        || catalog
            .starter_sql
            .as_deref()
            .is_some_and(|sql| sql.is_empty() || sql.len() > MAX_CATALOG_STARTER_BYTES)
    {
        return Err(PackError::InvalidManifest(
            "catalog metadata resource limit exceeded".to_owned(),
        ));
    }
    let mut table_names = BTreeSet::new();
    for table in &catalog.schema {
        if !is_catalog_identifier(&table.name)
            || !table_names.insert(table.name.clone())
            || table.columns.is_empty()
            || table.columns.len() > MAX_CATALOG_COLUMNS
        {
            return Err(PackError::InvalidManifest(
                "catalog table metadata is invalid".to_owned(),
            ));
        }
        let mut column_names = BTreeSet::new();
        for column in &table.columns {
            if !is_catalog_identifier(&column.name)
                || !column_names.insert(column.name.clone())
                || column.r#type.is_empty()
                || column.r#type.len() > 64
                || column.r#type.chars().any(char::is_control)
            {
                return Err(PackError::InvalidManifest(
                    "catalog column metadata is invalid".to_owned(),
                ));
            }
        }
    }
    for row in &catalog.sample_rows {
        validate_catalog_row(row)?;
    }
    let mut sample_table_names = BTreeSet::new();
    for table in &catalog.sample_tables {
        if !is_catalog_identifier(&table.name)
            || !sample_table_names.insert(table.name.as_str())
            || (!table_names.is_empty() && !table_names.contains(&table.name))
        {
            return Err(PackError::InvalidManifest(
                "catalog sample table metadata is invalid".to_owned(),
            ));
        }
        for row in &table.rows {
            validate_catalog_row(row)?;
        }
    }
    if catalog
        .starter_sql
        .as_deref()
        .is_some_and(|sql| !starter_sql_characters_are_allowed(sql))
    {
        return Err(PackError::InvalidManifest(
            "catalog starter SQL is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// Starter SQL may span lines: `\n`, `\r\n` and `\t` are the only control
/// characters allowed. A lone `\r` and every other control character are rejected.
fn starter_sql_characters_are_allowed(sql: &str) -> bool {
    let mut characters = sql.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' | '\t' => {}
            '\r' if characters.peek() == Some(&'\n') => {}
            other if other.is_control() => return false,
            _ => {}
        }
    }
    true
}

fn validate_catalog_row(row: &BTreeMap<String, serde_json::Value>) -> Result<(), PackError> {
    if row.len() > MAX_CATALOG_ROW_COLUMNS {
        return Err(PackError::InvalidManifest(
            "catalog sample row is too wide".to_owned(),
        ));
    }
    for (name, value) in row {
        if !is_catalog_identifier(name) || !catalog_value_is_bounded(value) {
            return Err(PackError::InvalidManifest(
                "catalog sample row is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn catalog_sample_tables(problem: &ProblemManifest) -> Result<Vec<CatalogSampleTable>, PackError> {
    validate_catalog(problem)?;
    Ok(problem
        .catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .sample_tables
                .iter()
                .map(|table| CatalogSampleTable {
                    name: table.name.clone(),
                    rows: table.rows.clone(),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn catalog_schema(problem: &ProblemManifest) -> Result<Vec<CatalogTable>, PackError> {
    validate_catalog(problem)?;
    Ok(problem
        .catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .schema
                .iter()
                .map(|table| CatalogTable {
                    name: table.name.clone(),
                    columns: table
                        .columns
                        .iter()
                        .map(|column| CatalogColumn {
                            name: column.name.clone(),
                            r#type: column.r#type.clone(),
                            nullable: column.nullable,
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn catalog_sample_rows(
    problem: &ProblemManifest,
) -> Result<Vec<BTreeMap<String, serde_json::Value>>, PackError> {
    validate_catalog(problem)?;
    Ok(problem
        .catalog
        .as_ref()
        .map(|catalog| catalog.sample_rows.clone())
        .unwrap_or_default())
}

fn catalog_starter_sql(problem: &ProblemManifest) -> Result<Option<String>, PackError> {
    validate_catalog(problem)?;
    Ok(problem
        .catalog
        .as_ref()
        .and_then(|catalog| catalog.starter_sql.clone()))
}

fn is_catalog_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn catalog_value_is_bounded(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
        serde_json::Value::String(value) => {
            value.len() <= MAX_CATALOG_CELL_BYTES && !value.chars().any(char::is_control)
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => false,
    }
}

fn validate_column(column: &Column) -> Result<(), PackError> {
    if column.name.is_empty()
        || !column.name.chars().enumerate().all(|(index, character)| {
            character == '_'
                || character.is_ascii_alphanumeric()
                    && (index > 0 || character.is_ascii_alphabetic())
        })
    {
        return Err(PackError::InvalidManifest(
            "invalid output column".to_owned(),
        ));
    }
    Ok(())
}

fn collect_directory(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    entry_count: &mut usize,
    depth: usize,
) -> Result<(), PackError> {
    if depth > MAX_PATH_DEPTH {
        return Err(PackError::ResourceLimit);
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| PackError::UnsafePath(path.display().to_string()))?;
        let name = relative
            .to_str()
            .ok_or_else(|| PackError::UnsafePath(relative.display().to_string()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        validate_path(&name)?;
        *entry_count += 1;
        if *entry_count > MAX_ENTRIES || name.split('/').count() > MAX_PATH_DEPTH {
            return Err(PackError::ResourceLimit);
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(PackError::Symlink(name));
        }
        if is_os_metadata(&name) {
            continue;
        }
        if metadata.is_dir() {
            collect_directory(root, &path, files, entry_count, depth + 1)?;
        } else if metadata.is_file() {
            if has_multiple_links(&path, &metadata)? {
                return Err(PackError::HardLink(name));
            }
            if metadata.len() > MAX_FILE_BYTES as u64 {
                return Err(PackError::ResourceLimit);
            }
            if files.len() >= MAX_FILES {
                return Err(PackError::ResourceLimit);
            }
            let current_size = files.values().map(Vec::len).sum::<usize>();
            if metadata.len() > (MAX_TOTAL_BYTES - current_size) as u64 {
                return Err(PackError::ResourceLimit);
            }
            files.insert(name, fs::read(path)?);
        } else {
            return Err(PackError::NonRegularFile(name));
        }
    }
    if files.values().map(Vec::len).sum::<usize>() > MAX_TOTAL_BYTES {
        return Err(PackError::ResourceLimit);
    }
    Ok(())
}

#[cfg(unix)]
fn has_multiple_links(_path: &Path, metadata: &fs::Metadata) -> Result<bool, PackError> {
    Ok(metadata.nlink() > 1)
}

// `MetadataExt::number_of_links` is nightly-only, so read the link count from the open handle.
#[cfg(windows)]
fn has_multiple_links(path: &Path, _metadata: &fs::Metadata) -> Result<bool, PackError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let file = fs::File::open(path)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` keeps the handle open for the call and `info` is a valid out-pointer.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(info.nNumberOfLinks > 1)
}

#[cfg(not(any(unix, windows)))]
fn has_multiple_links(_path: &Path, _metadata: &fs::Metadata) -> Result<bool, PackError> {
    Ok(false)
}

/// Pack paths always use `/`; `Path::join` would insert `\` on Windows and miss every file.
fn sibling_path(path: &str, file: &str) -> String {
    match path.rsplit_once('/') {
        Some((directory, _)) => format!("{directory}/{file}"),
        None => file.to_owned(),
    }
}

fn json_file<T: DeserializeOwned>(
    files: &BTreeMap<String, Vec<u8>>,
    name: &str,
) -> Result<T, PackError> {
    let bytes = files
        .get(name)
        .ok_or_else(|| PackError::MissingFile(name.to_owned()))?;
    serde_json::from_slice(bytes).map_err(|_| PackError::InvalidJson {
        file: name.to_owned(),
    })
}

fn sql_file(files: &BTreeMap<String, Vec<u8>>, name: &str) -> Result<String, PackError> {
    let bytes = files
        .get(name)
        .ok_or_else(|| PackError::MissingFile(name.to_owned()))?;
    let value = std::str::from_utf8(bytes).map_err(|_| PackError::InvalidSql(name.to_owned()))?;
    if value.trim().is_empty() || value.contains('\0') {
        return Err(PackError::InvalidSql(name.to_owned()));
    }
    Ok(value.to_owned())
}

fn check_sql(files: &BTreeMap<String, Vec<u8>>, name: &str) -> Result<(), PackError> {
    sql_file(files, name).map(|_| ())
}

/// Finder and Explorer drop these into any folder they open or compress; they are never pack
/// content, so they are skipped unread and excluded from the digest.
fn is_os_metadata(name: &str) -> bool {
    let file = name.rsplit('/').next().unwrap_or(name);
    name.split('/').next() == Some("__MACOSX")
        || file.starts_with("._")
        || [".DS_Store", "Thumbs.db", "desktop.ini"]
            .iter()
            .any(|metadata| file.eq_ignore_ascii_case(metadata))
}

fn validate_path(name: &str) -> Result<(), PackError> {
    if name.is_empty()
        || name.len() > MAX_RELATIVE_PATH_BYTES
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('/')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || (name.as_bytes().get(1) == Some(&b':'))
    {
        return Err(if name.len() > MAX_RELATIVE_PATH_BYTES {
            PackError::ResourceLimit
        } else {
            PackError::UnsafePath(name.to_owned())
        });
    }
    if name.split('/').count() > MAX_PATH_DEPTH {
        return Err(PackError::ResourceLimit);
    }
    Ok(())
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && !value.contains("--")
        && !value.ends_with('-')
}

fn is_pack_digest(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn dialect_name(dialect: Dialect) -> String {
    match dialect {
        Dialect::Postgresql => "postgresql".to_owned(),
        Dialect::Sqlserver => "sqlserver".to_owned(),
    }
}

fn is_semver(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && (part == &"0" || !part.starts_with('0'))
                && part.chars().all(|c| c.is_ascii_digit())
        })
}

/// Compares dotted numeric versions; non-numeric parts count as 0.
pub fn version_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    left.split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .cmp(
            right
                .split('.')
                .map(|part| part.parse::<u64>().unwrap_or(0)),
        )
}

fn is_schema_path(value: &str) -> bool {
    value.strip_prefix("schema/").is_some_and(|name| {
        name.ends_with(".sql") && name[..name.len() - 4].chars().all(path_character)
    })
}

fn is_problem_path(value: &str) -> bool {
    value
        .strip_prefix("problems/")
        .and_then(|value| value.strip_suffix("/problem.json"))
        .is_some_and(is_id)
}

fn is_test_path(value: &str) -> bool {
    value
        .strip_prefix("tests/")
        .and_then(|value| value.strip_suffix(".sql"))
        .is_some_and(is_id)
}

fn path_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

fn content_digest(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (name, content) in files {
        hasher.update((name.len() as u64).to_be_bytes());
        hasher.update(name.as_bytes());
        hasher.update((content.len() as u64).to_be_bytes());
        hasher.update(content);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

fn symlink_metadata_if_present(path: &Path) -> Result<Option<std::fs::Metadata>, std::io::Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::sibling_path;

    #[test]
    fn sibling_paths_use_forward_slashes_on_every_platform() {
        assert_eq!(
            sibling_path("problems/top-earners/problem.json", "tests/basic.sql"),
            "problems/top-earners/tests/basic.sql"
        );
        assert_eq!(
            sibling_path("problem.json", "reference.sql"),
            "reference.sql"
        );
    }
}
