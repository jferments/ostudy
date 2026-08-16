use std::collections::{BTreeMap, BTreeSet, HashMap};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::{
    params_from_iter,
    types::{Value, ValueRef},
    Connection,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CommandError;

use super::store::{BaselineRecord, NewConflict, SyncStore};

#[derive(Clone, Copy)]
pub(crate) struct TableSpec {
    pub name: &'static str,
    pub key_columns: &'static [&'static str],
    pub where_clause: Option<&'static str>,
    pub excluded_columns: &'static [&'static str],
}

pub(crate) const SYNC_TABLES: &[TableSpec] = &[
    TableSpec::new("deck_folders", &["id"]),
    TableSpec::new("decks", &["id"]),
    TableSpec::new("tags", &["id"]),
    TableSpec::new("card_templates", &["id"]),
    TableSpec::new("card_template_fields", &["id"]),
    TableSpec {
        name: "assets",
        key_columns: &["id"],
        where_clause: Some("lifecycle_state IN ('ready', 'deleted')"),
        excluded_columns: &[
            "draft_id",
            "staged_relative_path",
            "quarantine_relative_path",
            "ready_at",
            "verified_at",
            "lifecycle_reason",
        ],
    },
    TableSpec::new("notes", &["id"]),
    TableSpec::new("cards", &["id"]),
    TableSpec::new("card_template_rules", &["id"]),
    TableSpec::new("note_tags", &["note_id", "tag_id"]),
    TableSpec::new("note_assets", &["note_id", "asset_id", "side"]),
    TableSpec::new("card_template_assets", &["template_id", "asset_id"]),
    TableSpec::new("card_schedules", &["card_id"]),
    TableSpec::new("practice_sessions", &["id"]),
    TableSpec::new("practice_session_cards", &["session_id", "card_id"]),
    TableSpec::new(
        "practice_session_card_snapshots",
        &["session_id", "card_id"],
    ),
    TableSpec::new(
        "practice_session_card_tag_snapshots",
        &["session_id", "card_id", "tag_id"],
    ),
    TableSpec::new("review_events", &["id"]),
    TableSpec::new("review_event_attributions", &["review_event_id"]),
    TableSpec::new(
        "review_event_tag_attributions",
        &["review_event_id", "tag_id"],
    ),
    TableSpec {
        name: "ai_provider_profiles",
        key_columns: &["id"],
        where_clause: None,
        excluded_columns: &["credential_storage", "credential_epoch"],
    },
    TableSpec::new("ai_task_defaults", &["task_kind"]),
];

impl TableSpec {
    const fn new(name: &'static str, key_columns: &'static [&'static str]) -> Self {
        Self {
            name,
            key_columns,
            where_clause: None,
            excluded_columns: &[],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

pub(crate) type RowPayload = BTreeMap<String, CellValue>;
type TableRows = BTreeMap<String, RowPayload>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeSummary {
    pub changed_row_count: usize,
    pub conflict_count: usize,
    pub conflict_ids: Vec<String>,
}

struct PlannedConflict {
    table_name: &'static str,
    row_key: String,
    left_payload_json: Option<String>,
    right_payload_json: Option<String>,
    selected_payload_json: Option<String>,
    conflicting_fields: Vec<String>,
}

struct TablePlan {
    spec: &'static TableSpec,
    local: TableRows,
    merged: BTreeMap<String, Option<RowPayload>>,
    conflicts: Vec<PlannedConflict>,
}

pub(crate) fn merge_remote_database(
    local: &mut Connection,
    remote: &Connection,
    store: &SyncStore,
    local_device_id: &str,
    remote_device_id: &str,
) -> Result<MergeSummary, CommandError> {
    let mut plans = Vec::with_capacity(SYNC_TABLES.len());
    for spec in SYNC_TABLES {
        if !table_exists(local, spec.name)? || !table_exists(remote, spec.name)? {
            continue;
        }
        let local_rows = load_table(local, spec)?;
        let remote_rows = load_table(remote, spec)?;
        let baselines = store.load_baselines(remote_device_id, spec.name)?;
        plans.push(plan_three_way_table(
            spec,
            local_rows,
            remote_rows,
            baselines,
            local_device_id,
            remote_device_id,
        )?);
    }

    let changed_row_count = apply_plans(local, &plans)?;
    let mut conflict_ids = Vec::new();
    for plan in &plans {
        for conflict in &plan.conflicts {
            conflict_ids.push(store.record_conflict(NewConflict {
                peer_id: remote_device_id,
                table_name: conflict.table_name,
                row_key: &conflict.row_key,
                left_device_id: local_device_id,
                left_payload_json: conflict.left_payload_json.as_deref(),
                right_device_id: remote_device_id,
                right_payload_json: conflict.right_payload_json.as_deref(),
                selected_payload_json: conflict.selected_payload_json.as_deref(),
                conflicting_fields: &conflict.conflicting_fields,
            })?);
        }
    }
    replace_baselines(store, remote_device_id, &plans)?;
    Ok(MergeSummary {
        changed_row_count,
        conflict_count: conflict_ids.len(),
        conflict_ids,
    })
}

pub(crate) fn apply_conflict_payload(
    connection: &mut Connection,
    table_name: &str,
    row_key: &str,
    payload_json: Option<&str>,
) -> Result<(), CommandError> {
    let spec = table_spec(table_name).ok_or_else(|| {
        CommandError::new(
            "device_sync_table_unsupported",
            "That synchronization entity is not supported by this build.",
        )
    })?;
    let transaction = connection.transaction().map_err(CommandError::database)?;
    match payload_json {
        Some(json) => {
            let payload: RowPayload = serde_json::from_str(json).map_err(|error| {
                CommandError::new(
                    "device_sync_conflict_invalid",
                    format!("The saved conflict payload is invalid: {error}"),
                )
            })?;
            upsert_row(&transaction, spec, &payload)?;
        }
        None => delete_row(&transaction, spec, row_key)?,
    }
    verify_foreign_keys(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}

pub(crate) fn table_spec(name: &str) -> Option<&'static TableSpec> {
    SYNC_TABLES.iter().find(|spec| spec.name == name)
}

pub(crate) fn payload_hash(payload: Option<&RowPayload>) -> Result<String, CommandError> {
    let json = payload_json(payload)?;
    Ok(format!("{:x}", Sha256::digest(json.as_bytes())))
}

pub(crate) fn payload_json(payload: Option<&RowPayload>) -> Result<String, CommandError> {
    serde_json::to_string(&payload).map_err(CommandError::serialization)
}

fn plan_three_way_table(
    spec: &'static TableSpec,
    local: TableRows,
    remote: TableRows,
    baselines: HashMap<String, BaselineRecord>,
    local_device_id: &str,
    remote_device_id: &str,
) -> Result<TablePlan, CommandError> {
    let mut keys = BTreeSet::new();
    keys.extend(local.keys().cloned());
    keys.extend(remote.keys().cloned());
    keys.extend(baselines.keys().cloned());
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();

    for key in keys {
        let left = local.get(&key);
        let right = remote.get(&key);
        let base = if let Some(record) = baselines.get(&key) {
            let parsed = record
                .payload_json
                .as_deref()
                .map(serde_json::from_str::<RowPayload>)
                .transpose()
                .map_err(|error| {
                    CommandError::new(
                        "device_sync_baseline_invalid",
                        format!("A saved synchronization baseline is invalid: {error}"),
                    )
                })?;
            if payload_hash(parsed.as_ref())? != record.payload_hash {
                return Err(CommandError::new(
                    "device_sync_baseline_invalid",
                    "A saved synchronization baseline failed its content hash check.",
                ));
            }
            parsed
        } else {
            None
        };
        let (selected, conflicting_fields) = merge_rows(
            base.as_ref(),
            left,
            right,
            spec.key_columns,
            local_device_id,
            remote_device_id,
        );
        if !conflicting_fields.is_empty() {
            if spec.name == "assets" {
                return Err(CommandError::new(
                    "device_sync_asset_conflict",
                    "Two devices contain different bytes or metadata for the same managed image identity.",
                ));
            }
            conflicts.push(PlannedConflict {
                table_name: spec.name,
                row_key: key.clone(),
                left_payload_json: optional_payload_json(left)?,
                right_payload_json: optional_payload_json(right)?,
                selected_payload_json: optional_payload_json(selected.as_ref())?,
                conflicting_fields,
            });
        }
        merged.insert(key, selected);
    }

    Ok(TablePlan {
        spec,
        local,
        merged,
        conflicts,
    })
}

fn merge_rows(
    base: Option<&RowPayload>,
    left: Option<&RowPayload>,
    right: Option<&RowPayload>,
    key_columns: &[&str],
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    if left == right {
        return (left.cloned(), Vec::new());
    }
    if base == left {
        return (right.cloned(), Vec::new());
    }
    if base == right {
        return (left.cloned(), Vec::new());
    }
    match (left, right) {
        (None, None) => (None, Vec::new()),
        (Some(payload), None) | (None, Some(payload)) => {
            // Preserve the edited/remaining row and require an explicit delete
            // decision. This prevents delete-vs-edit races from losing content.
            (Some(payload.clone()), vec!["$row".to_string()])
        }
        (Some(left), Some(right)) => merge_present_rows(
            base,
            left,
            right,
            key_columns,
            left_device_id,
            right_device_id,
        ),
    }
}

fn merge_present_rows(
    base: Option<&RowPayload>,
    left: &RowPayload,
    right: &RowPayload,
    key_columns: &[&str],
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    let prefer_left = left_device_id <= right_device_id;
    let mut columns = BTreeSet::new();
    columns.extend(left.keys().cloned());
    columns.extend(right.keys().cloned());
    if let Some(base) = base {
        columns.extend(base.keys().cloned());
    }
    let mut output = RowPayload::new();
    let mut conflicts = Vec::new();
    for column in columns {
        let left_value = left.get(&column);
        let right_value = right.get(&column);
        let base_value = base.and_then(|payload| payload.get(&column));
        let selected = if left_value == right_value {
            left_value.or(right_value)
        } else if key_columns.contains(&column.as_str()) {
            conflicts.push(column.clone());
            if prefer_left {
                left_value
            } else {
                right_value
            }
        } else {
            let left_changed = left_value != base_value;
            let right_changed = right_value != base_value;
            match (left_changed, right_changed) {
                (false, true) => right_value,
                (true, false) => left_value,
                (false, false) => left_value.or(right_value),
                (true, true) => {
                    conflicts.push(column.clone());
                    if prefer_left {
                        left_value
                    } else {
                        right_value
                    }
                }
            }
        };
        if let Some(value) = selected {
            output.insert(column, value.clone());
        }
    }
    (Some(output), conflicts)
}

fn apply_plans(connection: &mut Connection, plans: &[TablePlan]) -> Result<usize, CommandError> {
    let transaction = connection.transaction().map_err(CommandError::database)?;
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .map_err(CommandError::database)?;
    let mut changed = 0_usize;

    for plan in plans.iter().rev() {
        for (key, payload) in &plan.merged {
            if payload.is_none() && plan.local.contains_key(key) {
                delete_row(&transaction, plan.spec, key)?;
                changed = changed.saturating_add(1);
            }
        }
    }
    for plan in plans {
        for (key, payload) in &plan.merged {
            if let Some(payload) = payload {
                if plan.local.get(key) != Some(payload) {
                    upsert_row(&transaction, plan.spec, payload)?;
                    changed = changed.saturating_add(1);
                }
            }
        }
    }
    if table_exists(&transaction, "library_organization")? && changed > 0 {
        transaction
            .execute(
                "UPDATE library_organization
                 SET revision = revision + 1, updated_at = ?1
                 WHERE singleton_id = 1",
                [crate::util::unix_time_millis()?],
            )
            .map_err(CommandError::database)?;
    }
    verify_foreign_keys(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(changed)
}

fn replace_baselines(
    store: &SyncStore,
    peer_id: &str,
    plans: &[TablePlan],
) -> Result<(), CommandError> {
    for plan in plans {
        let mut rows = Vec::with_capacity(plan.merged.len());
        for (key, payload) in &plan.merged {
            let payload_json = optional_payload_json(payload.as_ref())?;
            let hash = payload_hash(payload.as_ref())?;
            rows.push((key.clone(), payload_json, hash));
        }
        store.replace_table_baselines(peer_id, plan.spec.name, &rows)?;
    }
    Ok(())
}

fn load_table(connection: &Connection, spec: &TableSpec) -> Result<TableRows, CommandError> {
    let where_sql = spec
        .where_clause
        .map(|clause| format!(" WHERE {clause}"))
        .unwrap_or_default();
    let sql = format!("SELECT * FROM {}{where_sql}", quote_identifier(spec.name)?);
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let columns = statement
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    for key in spec.key_columns {
        if !columns.iter().any(|column| column == key) {
            return Err(CommandError::new(
                "device_sync_schema_mismatch",
                format!(
                    "Table '{}' is missing synchronization key '{key}'.",
                    spec.name
                ),
            ));
        }
    }
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    let mut output = TableRows::new();
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let mut payload = RowPayload::new();
        for (index, column) in columns.iter().enumerate() {
            if spec.excluded_columns.contains(&column.as_str()) {
                continue;
            }
            payload.insert(
                column.clone(),
                cell_from_ref(row.get_ref(index).map_err(CommandError::database)?)?,
            );
        }
        let key = row_key(spec, &payload)?;
        if output.insert(key, payload).is_some() {
            return Err(CommandError::new(
                "device_sync_schema_mismatch",
                format!(
                    "Table '{}' contains duplicate synchronization keys.",
                    spec.name
                ),
            ));
        }
    }
    Ok(output)
}

fn upsert_row(
    connection: &Connection,
    spec: &TableSpec,
    payload: &RowPayload,
) -> Result<(), CommandError> {
    for key in spec.key_columns {
        if !payload.contains_key(*key) {
            return Err(CommandError::new(
                "device_sync_row_invalid",
                format!("A synchronized '{}' row is missing key '{key}'.", spec.name),
            ));
        }
    }
    let columns = payload.keys().cloned().collect::<Vec<_>>();
    let quoted_columns = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?;
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let conflict_columns = spec
        .key_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let update_columns = columns
        .iter()
        .filter(|column| !spec.key_columns.contains(&column.as_str()))
        .map(|column| {
            let quoted = quote_identifier(column)?;
            Ok(format!("{quoted} = excluded.{quoted}"))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let action = if update_columns.is_empty() {
        "DO NOTHING".to_string()
    } else {
        format!("DO UPDATE SET {}", update_columns.join(", "))
    };
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) {}",
        quote_identifier(spec.name)?,
        quoted_columns.join(", "),
        placeholders,
        conflict_columns,
        action,
    );
    let values = columns
        .iter()
        .map(|column| cell_to_sql(payload.get(column).expect("column came from payload")))
        .collect::<Result<Vec<_>, _>>()?;
    connection
        .execute(&sql, params_from_iter(values))
        .map_err(CommandError::database)?;
    Ok(())
}

fn delete_row(
    connection: &Connection,
    spec: &TableSpec,
    row_key_json: &str,
) -> Result<(), CommandError> {
    let values: Vec<CellValue> = serde_json::from_str(row_key_json).map_err(|error| {
        CommandError::new(
            "device_sync_row_key_invalid",
            format!("A synchronized row key is invalid: {error}"),
        )
    })?;
    if values.len() != spec.key_columns.len() {
        return Err(CommandError::new(
            "device_sync_row_key_invalid",
            "A synchronized row key has the wrong number of fields.",
        ));
    }
    let predicates = spec
        .key_columns
        .iter()
        .enumerate()
        .map(|(index, column)| Ok(format!("{} = ?{}", quote_identifier(column)?, index + 1)))
        .collect::<Result<Vec<_>, CommandError>>()?;
    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_identifier(spec.name)?,
        predicates.join(" AND "),
    );
    let parameters = values
        .iter()
        .map(cell_to_sql)
        .collect::<Result<Vec<_>, _>>()?;
    connection
        .execute(&sql, params_from_iter(parameters))
        .map_err(CommandError::database)?;
    Ok(())
}

fn row_key(spec: &TableSpec, payload: &RowPayload) -> Result<String, CommandError> {
    let values = spec
        .key_columns
        .iter()
        .map(|column| {
            payload.get(*column).cloned().ok_or_else(|| {
                CommandError::new(
                    "device_sync_row_invalid",
                    format!(
                        "A synchronized '{}' row is missing key '{column}'.",
                        spec.name
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&values).map_err(CommandError::serialization)
}

fn cell_from_ref(value: ValueRef<'_>) -> Result<CellValue, CommandError> {
    match value {
        ValueRef::Null => Ok(CellValue::Null),
        ValueRef::Integer(value) => Ok(CellValue::Integer(value)),
        ValueRef::Real(value) if value.is_finite() => Ok(CellValue::Real(value)),
        ValueRef::Real(_) => Err(CommandError::new(
            "device_sync_row_invalid",
            "A synchronized numeric field is not finite.",
        )),
        ValueRef::Text(value) => std::str::from_utf8(value)
            .map(|value| CellValue::Text(value.to_string()))
            .map_err(|_| {
                CommandError::new(
                    "device_sync_row_invalid",
                    "A synchronized text field is not valid UTF-8.",
                )
            }),
        ValueRef::Blob(value) => Ok(CellValue::Blob(BASE64_STANDARD.encode(value))),
    }
}

fn cell_to_sql(value: &CellValue) -> Result<Value, CommandError> {
    match value {
        CellValue::Null => Ok(Value::Null),
        CellValue::Integer(value) => Ok(Value::Integer(*value)),
        CellValue::Real(value) if value.is_finite() => Ok(Value::Real(*value)),
        CellValue::Real(_) => Err(CommandError::new(
            "device_sync_row_invalid",
            "A synchronized numeric field is not finite.",
        )),
        CellValue::Text(value) => Ok(Value::Text(value.clone())),
        CellValue::Blob(value) => BASE64_STANDARD.decode(value).map(Value::Blob).map_err(|_| {
            CommandError::new(
                "device_sync_row_invalid",
                "A synchronized binary field is not valid base64.",
            )
        }),
    }
}

fn optional_payload_json(payload: Option<&RowPayload>) -> Result<Option<String>, CommandError> {
    payload
        .map(|payload| serde_json::to_string(payload).map_err(CommandError::serialization))
        .transpose()
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = ?1
             )",
            [table_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value == 1)
        .map_err(CommandError::database)
}

fn verify_foreign_keys(connection: &Connection) -> Result<(), CommandError> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    if count == 0 {
        Ok(())
    } else {
        Err(CommandError::new(
            "device_sync_integrity_failed",
            "The synchronized library would violate relational integrity, so no changes were saved.",
        ))
    }
}

fn quote_identifier(identifier: &str) -> Result<String, CommandError> {
    if identifier.is_empty()
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(CommandError::new(
            "device_sync_schema_mismatch",
            "A synchronized SQLite identifier is invalid.",
        ));
    }
    Ok(format!("\"{identifier}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entries: &[(&str, CellValue)]) -> RowPayload {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn disjoint_field_edits_merge_without_conflict() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
            ("description", CellValue::Text(String::new())),
        ]);
        let mut left = base.clone();
        left.insert("name".to_string(), CellValue::Text("Local".to_string()));
        let mut right = base.clone();
        right.insert(
            "description".to_string(),
            CellValue::Text("Remote description".to_string()),
        );
        let (merged, conflicts) = merge_rows(
            Some(&base),
            Some(&left),
            Some(&right),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        let merged = merged.expect("row should survive");
        assert!(conflicts.is_empty());
        assert_eq!(merged.get("name"), left.get("name"));
        assert_eq!(merged.get("description"), right.get("description"));
    }

    #[test]
    fn same_field_edits_create_deterministic_conflict() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
        ]);
        let mut left = base.clone();
        left.insert("name".to_string(), CellValue::Text("Local".to_string()));
        let mut right = base.clone();
        right.insert("name".to_string(), CellValue::Text("Remote".to_string()));
        let (merged, conflicts) = merge_rows(
            Some(&base),
            Some(&left),
            Some(&right),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert_eq!(conflicts, vec!["name"]);
        assert_eq!(
            merged.expect("row should survive").get("name"),
            left.get("name")
        );
    }

    #[test]
    fn delete_versus_edit_preserves_the_row_for_resolution() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
        ]);
        let mut edited = base.clone();
        edited.insert("name".to_string(), CellValue::Text("Edited".to_string()));
        let (merged, conflicts) = merge_rows(
            Some(&base),
            None,
            Some(&edited),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert!(merged.is_some());
        assert_eq!(conflicts, vec!["$row"]);
    }
}
