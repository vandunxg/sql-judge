//! Installed pack catalog (ADR-025). Rows are written only after the pack passed structural
//! and runtime validation; afterwards they are trusted local metadata like history.

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use std::collections::BTreeSet;
use std::path::Path;

use crate::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackRecord {
    pub digest: String,
    pub pack_id: String,
    pub version: String,
    pub title: String,
    pub dialect: String,
    pub schema_sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackProblemRecord {
    pub problem_id: String,
    /// The display entry the UI receives, serialized by `problem-pack`.
    pub catalog_json: String,
    pub dialect: String,
    pub reference_sql: String,
    /// The judge output policy, serialized by `problem-pack`.
    pub output_json: String,
    pub statement_timeout_ms: u32,
    pub max_rows: u32,
    pub max_bytes: u32,
    pub tests: Vec<PackTestRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackTestRecord {
    pub test_id: String,
    pub hidden: bool,
    pub fixture_sql: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackInsert {
    Inserted,
    AlreadyInstalled,
    /// Another digest is installed under the same pack ID and version.
    VersionConflict,
}

pub struct PackStore {
    connection: Connection,
}

const PACK_COLUMNS: &str = "digest, pack_id, version, title, dialect, schema_sql";

impl PackStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS packs (
                 digest TEXT PRIMARY KEY NOT NULL,
                 pack_id TEXT NOT NULL,
                 version TEXT NOT NULL,
                 title TEXT NOT NULL,
                 dialect TEXT NOT NULL,
                 schema_sql TEXT NOT NULL,
                 installed_at TEXT NOT NULL,
                 UNIQUE (pack_id, version)
             );
             CREATE TABLE IF NOT EXISTS pack_problems (
                 digest TEXT NOT NULL REFERENCES packs(digest) ON DELETE CASCADE,
                 problem_id TEXT NOT NULL,
                 position INTEGER NOT NULL,
                 catalog_json TEXT NOT NULL,
                 dialect TEXT NOT NULL,
                 reference_sql TEXT NOT NULL,
                 output_json TEXT NOT NULL,
                 statement_timeout_ms INTEGER NOT NULL,
                 max_rows INTEGER NOT NULL,
                 max_bytes INTEGER NOT NULL,
                 PRIMARY KEY (digest, problem_id)
             );
             CREATE TABLE IF NOT EXISTS pack_problem_tests (
                 digest TEXT NOT NULL,
                 problem_id TEXT NOT NULL,
                 test_id TEXT NOT NULL,
                 position INTEGER NOT NULL,
                 hidden INTEGER NOT NULL CHECK (hidden IN (0, 1)),
                 fixture_sql TEXT NOT NULL,
                 PRIMARY KEY (digest, problem_id, test_id),
                 FOREIGN KEY (digest, problem_id)
                     REFERENCES pack_problems(digest, problem_id) ON DELETE CASCADE
             );",
        )?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn insert_pack(
        &mut self,
        pack: &PackRecord,
        problems: &[PackProblemRecord],
    ) -> Result<PackInsert, StorageError> {
        validate_pack(pack, problems)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if find_pack_in(&transaction, &pack.digest)?.is_some() {
            return Ok(PackInsert::AlreadyInstalled);
        }
        let conflict = transaction
            .query_row(
                "SELECT 1 FROM packs WHERE pack_id = ?1 AND version = ?2",
                params![pack.pack_id, pack.version],
                |_| Ok(()),
            )
            .optional()?;
        if conflict.is_some() {
            return Ok(PackInsert::VersionConflict);
        }
        transaction.execute(
            "INSERT INTO packs (digest, pack_id, version, title, dialect, schema_sql, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%s', 'now'))",
            params![
                pack.digest,
                pack.pack_id,
                pack.version,
                pack.title,
                pack.dialect,
                pack.schema_sql,
            ],
        )?;
        for (position, problem) in problems.iter().enumerate() {
            transaction.execute(
                "INSERT INTO pack_problems (
                     digest, problem_id, position, catalog_json, dialect, reference_sql,
                     output_json, statement_timeout_ms, max_rows, max_bytes
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    pack.digest,
                    problem.problem_id,
                    position as i64,
                    problem.catalog_json,
                    problem.dialect,
                    problem.reference_sql,
                    problem.output_json,
                    problem.statement_timeout_ms,
                    problem.max_rows,
                    problem.max_bytes,
                ],
            )?;
            for (test_position, test) in problem.tests.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO pack_problem_tests (
                         digest, problem_id, test_id, position, hidden, fixture_sql
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        pack.digest,
                        problem.problem_id,
                        test.test_id,
                        test_position as i64,
                        test.hidden,
                        test.fixture_sql,
                    ],
                )?;
            }
        }
        transaction.commit()?;
        Ok(PackInsert::Inserted)
    }

    /// Ordered by pack ID, then newest version text first.
    pub fn list_packs(&self) -> Result<Vec<PackRecord>, StorageError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {PACK_COLUMNS} FROM packs ORDER BY pack_id, version DESC"
        ))?;
        let rows = statement.query_map([], decode_pack)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn find_pack(&self, digest: &str) -> Result<Option<PackRecord>, StorageError> {
        find_pack_in(&self.connection, digest)
    }

    /// `(digest, catalog_json)` for every installed problem, in declared order per pack.
    pub fn list_problem_catalog(&self) -> Result<Vec<(String, String)>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT digest, catalog_json FROM pack_problems ORDER BY digest, position")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn find_problem(
        &self,
        digest: &str,
        problem_id: &str,
    ) -> Result<Option<PackProblemRecord>, StorageError> {
        let problem = self
            .connection
            .query_row(
                "SELECT problem_id, catalog_json, dialect, reference_sql, output_json,
                        statement_timeout_ms, max_rows, max_bytes
                 FROM pack_problems WHERE digest = ?1 AND problem_id = ?2",
                params![digest, problem_id],
                |row| {
                    Ok(PackProblemRecord {
                        problem_id: row.get(0)?,
                        catalog_json: row.get(1)?,
                        dialect: row.get(2)?,
                        reference_sql: row.get(3)?,
                        output_json: row.get(4)?,
                        statement_timeout_ms: row.get(5)?,
                        max_rows: row.get(6)?,
                        max_bytes: row.get(7)?,
                        tests: Vec::new(),
                    })
                },
            )
            .optional()?;
        let Some(mut problem) = problem else {
            return Ok(None);
        };
        let mut statement = self.connection.prepare(
            "SELECT test_id, hidden, fixture_sql FROM pack_problem_tests
             WHERE digest = ?1 AND problem_id = ?2 ORDER BY position",
        )?;
        let tests = statement.query_map(params![digest, problem_id], |row| {
            Ok(PackTestRecord {
                test_id: row.get(0)?,
                hidden: row.get(1)?,
                fixture_sql: row.get(2)?,
            })
        })?;
        problem.tests = tests.collect::<Result<Vec<_>, _>>()?;
        Ok(Some(problem))
    }

    /// Returns whether a pack was removed; its problems and tests cascade.
    pub fn delete_pack(&mut self, digest: &str) -> Result<bool, StorageError> {
        Ok(self
            .connection
            .execute("DELETE FROM packs WHERE digest = ?1", [digest])?
            > 0)
    }
}

fn find_pack_in(connection: &Connection, digest: &str) -> Result<Option<PackRecord>, StorageError> {
    connection
        .query_row(
            &format!("SELECT {PACK_COLUMNS} FROM packs WHERE digest = ?1"),
            [digest],
            decode_pack,
        )
        .optional()
        .map_err(StorageError::from)
}

fn decode_pack(row: &Row<'_>) -> rusqlite::Result<PackRecord> {
    Ok(PackRecord {
        digest: row.get(0)?,
        pack_id: row.get(1)?,
        version: row.get(2)?,
        title: row.get(3)?,
        dialect: row.get(4)?,
        schema_sql: row.get(5)?,
    })
}

fn validate_pack(pack: &PackRecord, problems: &[PackProblemRecord]) -> Result<(), StorageError> {
    if pack.digest.len() != 64
        || !pack
            .digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StorageError::InvalidRecord("pack digest"));
    }
    for (value, field) in [
        (&pack.pack_id, "pack id"),
        (&pack.version, "pack version"),
        (&pack.title, "pack title"),
        (&pack.dialect, "pack dialect"),
    ] {
        if value.trim().is_empty() {
            return Err(StorageError::InvalidRecord(field));
        }
    }
    let mut problem_ids = BTreeSet::new();
    for problem in problems {
        if problem.problem_id.trim().is_empty() || !problem_ids.insert(&problem.problem_id) {
            return Err(StorageError::InvalidRecord("pack problem id"));
        }
        let mut test_ids = BTreeSet::new();
        for test in &problem.tests {
            if test.test_id.trim().is_empty() || !test_ids.insert(&test.test_id) {
                return Err(StorageError::InvalidRecord("pack test id"));
            }
        }
    }
    Ok(())
}
