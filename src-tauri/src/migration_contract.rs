//! Native-owned, deterministic database lineage metadata for maintenance tools.
//!
//! Runtime admission remains owned by database/data-repair modules. This module
//! only renders their immutable identities into a checked-in, build-independent
//! JSON manifest so Python/Node maintenance tools do not scrape Rust source.

use serde::Serialize;

use crate::{data_repairs, database};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseContract {
    schema_version: u32,
    public: PublicContract,
    repairs: Vec<RepairContract>,
    release: ReleaseContract,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicContract {
    lineage: &'static str,
    legacy_public_schema_version: i64,
    latest_schema_version: i64,
    migrations: Vec<PublicMigrationContract>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicMigrationContract {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    sql_file: String,
    rebuilds_tables: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairContract {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    phase: RepairPhaseContract,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RepairPhaseKind {
    AfterMigration,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairPhaseContract {
    kind: RepairPhaseKind,
    version: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseContract {
    lineage: &'static str,
    baseline_version: i64,
    version: i64,
    name: &'static str,
    checksum: &'static str,
    migrations: Vec<ReleaseMigrationContract>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseMigrationContract {
    version: i64,
    name: &'static str,
    checksum: &'static str,
}

fn contract() -> DatabaseContract {
    let migrations = database::public_migration_contract_rows()
        .into_iter()
        .map(
            |(version, name, checksum, rebuilds_tables)| PublicMigrationContract {
                version,
                name,
                checksum,
                sql_file: format!("{version:04}_{name}.sql"),
                rebuilds_tables,
            },
        )
        .collect();
    let repairs = data_repairs::repair_contract_rows()
        .into_iter()
        .map(
            |(version, name, checksum, migration_version)| RepairContract {
                version,
                name,
                checksum,
                phase: RepairPhaseContract {
                    kind: RepairPhaseKind::AfterMigration,
                    version: migration_version,
                },
            },
        )
        .collect();
    let release = database::release_contract_metadata();
    DatabaseContract {
        schema_version: 1,
        public: PublicContract {
            lineage: database::legacy_public_schema_lineage(),
            legacy_public_schema_version: database::LEGACY_PUBLIC_SCHEMA_VERSION,
            latest_schema_version: database::LATEST_SCHEMA_VERSION,
            migrations,
        },
        repairs,
        release: ReleaseContract {
            lineage: release.lineage,
            baseline_version: release.baseline_version,
            version: release.version,
            name: release.name,
            checksum: release.checksum,
            migrations: release.migrations
                .into_iter()
                .map(|(version, name, checksum)| ReleaseMigrationContract {
                    version,
                    name,
                    checksum,
                })
                .collect(),
        },
    }
}

pub fn render_json() -> String {
    let mut output =
        serde_json::to_string_pretty(&contract()).expect("database contract serialization");
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATED: &str = include_str!("../../scripts/generated/database-contract.json");

    #[test]
    fn checked_in_database_contract_matches_native_owners() {
        assert_eq!(GENERATED, render_json());
    }

    #[test]
    fn public_release_and_repair_identities_remain_distinct() {
        let contract = contract();
        assert_ne!(contract.public.lineage, contract.release.lineage);
        assert_eq!(
            contract.public.latest_schema_version,
            contract.public.migrations.last().unwrap().version
        );
        assert_eq!(
            contract.release.version,
            contract.release.migrations.last().unwrap().version
        );
        assert!(!contract.repairs.is_empty());
        assert!(contract
            .repairs
            .iter()
            .all(|repair| repair.phase.version <= contract.public.latest_schema_version));
    }
}
