use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::CommandError,
    knowledge::contracts::{hash, invalid},
    util::{unix_time_millis, validate_entity_id},
};

pub(crate) fn request_hash<T: Serialize>(action: &str, input: &T) -> Result<String, CommandError> {
    let payload = serde_json::to_string(input).map_err(CommandError::serialization)?;
    Ok(hash(&format!("{action}\0{payload}")))
}

pub(crate) fn replay<T: DeserializeOwned>(
    connection: &Connection,
    operation_id: &str,
    action: &str,
    request_hash: &str,
) -> Result<Option<T>, CommandError> {
    validate_entity_id(operation_id, "Tutor organization operation")?;
    let saved = connection
        .query_row(
            "SELECT action,request_hash,result_json FROM tutor_organization_operations WHERE operation_id=?1",
            [operation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((saved_action, saved_hash, result_json)) = saved else {
        return Ok(None);
    };
    if saved_action != action || saved_hash != request_hash {
        return Err(invalid(
            "This Tutor organization operation was already used for different input.",
        ));
    }
    serde_json::from_str(&result_json)
        .map(Some)
        .map_err(CommandError::serialization)
}

pub(crate) fn record<T: Serialize>(
    connection: &Connection,
    operation_id: &str,
    action: &str,
    target_kind: &str,
    target_id: &str,
    request_hash: &str,
    result: &T,
) -> Result<(), CommandError> {
    validate_entity_id(operation_id, "Tutor organization operation")?;
    validate_entity_id(target_id, "Tutor organization target")?;
    let result_json = serde_json::to_string(result).map_err(CommandError::serialization)?;
    connection
        .execute(
            "INSERT INTO tutor_organization_operations(
                 operation_id,action,target_kind,target_id,request_hash,result_json,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                operation_id,
                action,
                target_kind,
                target_id,
                request_hash,
                result_json,
                unix_time_millis()?
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
