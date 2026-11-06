//! SQLx migrations and the startup schema check.

use std::collections::BTreeSet;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub fn expected_versions() -> Vec<i64> {
    MIGRATOR.migrations.iter().map(|m| m.version).collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum SchemaStatus {
    Current,
    /// Known migrations have not run yet.
    Behind,
    /// Applied versions are not known to this build.
    Ahead,
    Missing,
}

pub fn classify(applied: &[i64]) -> SchemaStatus {
    classify_with(&expected_versions(), applied)
}

pub fn classify_with(expected: &[i64], applied: &[i64]) -> SchemaStatus {
    if applied.is_empty() {
        return SchemaStatus::Missing;
    }
    let expected: BTreeSet<i64> = expected.iter().copied().collect();
    let applied: BTreeSet<i64> = applied.iter().copied().collect();
    if !expected.is_subset(&applied) {
        SchemaStatus::Behind
    } else if applied.iter().any(|version| !expected.contains(version)) {
        SchemaStatus::Ahead
    } else {
        SchemaStatus::Current
    }
}
