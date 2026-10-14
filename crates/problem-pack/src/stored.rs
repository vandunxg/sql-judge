//! Conversion between validated packs and the SQLite catalog records (ADR-025).

use app_storage::{PackProblemRecord, PackRecord, PackTestRecord};
use judge_core::{Dialect, OutputPolicy, QueryLimits};

use crate::{
    PackCatalogEntry, PackError, ProblemCatalogEntry, ValidatedPack, ValidatedProblem,
    ValidatedTest, dialect_name,
};

/// Flattens a fully validated pack into the rows the catalog stores.
pub fn pack_records(
    pack: &ValidatedPack,
) -> Result<(PackRecord, Vec<PackProblemRecord>), PackError> {
    let catalog = pack.catalog_entry()?;
    let record = PackRecord {
        digest: catalog.digest.clone(),
        pack_id: catalog.pack_id.clone(),
        version: catalog.version.clone(),
        title: catalog.title.clone(),
        dialect: catalog.dialect.clone(),
        schema_sql: pack.schema_sql()?,
    };
    let problems = catalog
        .problems
        .iter()
        .map(|entry| {
            let problem = pack
                .problem(&entry.id)?
                .ok_or_else(|| PackError::MissingFile(entry.id.clone()))?;
            Ok(PackProblemRecord {
                problem_id: problem.id,
                catalog_json: serde_json::to_string(entry)
                    .map_err(|_| PackError::InvalidStoredRecord("catalog entry"))?,
                dialect: dialect_name(problem.dialect),
                reference_sql: problem.reference_sql,
                output_json: serde_json::to_string(&problem.output)
                    .map_err(|_| PackError::InvalidStoredRecord("output policy"))?,
                statement_timeout_ms: problem.limits.statement_timeout_ms,
                max_rows: problem.limits.max_rows,
                max_bytes: problem.limits.max_bytes,
                tests: problem
                    .tests
                    .into_iter()
                    .map(|test| PackTestRecord {
                        test_id: test.id,
                        hidden: test.hidden,
                        fixture_sql: test.fixture_sql,
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, PackError>>()?;
    Ok((record, problems))
}

/// Rebuilds the judge input for one installed problem. Every problem uses the pack schema.
pub fn problem_from_record(
    pack: &PackRecord,
    record: &PackProblemRecord,
) -> Result<ValidatedProblem, PackError> {
    let dialect = match record.dialect.as_str() {
        "postgresql" => Dialect::Postgresql,
        "sqlserver" => Dialect::Sqlserver,
        _ => return Err(PackError::InvalidStoredRecord("problem dialect")),
    };
    let output = serde_json::from_str::<OutputPolicy>(&record.output_json)
        .map_err(|_| PackError::InvalidStoredRecord("output policy"))?;
    Ok(ValidatedProblem {
        id: record.problem_id.clone(),
        dialect,
        schema_sql: pack.schema_sql.clone(),
        reference_sql: record.reference_sql.clone(),
        output,
        limits: QueryLimits::new(
            record.statement_timeout_ms,
            record.max_rows,
            record.max_bytes,
        ),
        tests: record
            .tests
            .iter()
            .map(|test| ValidatedTest {
                id: test.test_id.clone(),
                hidden: test.hidden,
                fixture_sql: test.fixture_sql.clone(),
            })
            .collect(),
    })
}

/// Rebuilds the UI catalog entry for one installed pack from its stored problem entries.
pub fn catalog_from_records(
    pack: &PackRecord,
    catalog_json: &[&str],
) -> Result<PackCatalogEntry, PackError> {
    let problems = catalog_json
        .iter()
        .map(|json| {
            serde_json::from_str::<ProblemCatalogEntry>(json)
                .map_err(|_| PackError::InvalidStoredRecord("catalog entry"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PackCatalogEntry {
        pack_id: pack.pack_id.clone(),
        version: pack.version.clone(),
        title: pack.title.clone(),
        dialect: pack.dialect.clone(),
        digest: pack.digest.clone(),
        problems,
    })
}
