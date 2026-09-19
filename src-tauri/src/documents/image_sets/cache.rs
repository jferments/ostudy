//! Owner-checked, persistent local OCR reuse. SQL never encloses OCR or file IO.
//! The cache contains retained source evidence, not provider interpretations.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database, document_canonical::sha256_hex, error::CommandError,
    pdf_ocr_backend::regions::RegionOcrResult,
};

use super::{admission::PIXEL_CONTRACT, manifest::ImageMember};

const MAX_RESULT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Identity {
    version: u32,
    pixel_sha256: String,
    pixel_contract: String,
    quarter_turns: u8,
    engine_contract: String,
}

impl Identity {
    fn for_member(member: &ImageMember, contract: &str) -> Result<Self, CommandError> {
        let result = Self {
            version: 1,
            pixel_sha256: member.pixel_sha256.clone(),
            pixel_contract: member.pixel_contract.clone(),
            quarter_turns: member.quarter_turns,
            engine_contract: contract.to_string(),
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<(), CommandError> {
        super::manifest::validate_hash(&self.pixel_sha256)?;
        if self.version != 1
            || self.pixel_contract != PIXEL_CONTRACT
            || self.quarter_turns > 3
            || self.engine_contract.is_empty()
            || self.engine_contract.len() > 512
            || self.engine_contract.chars().any(char::is_control)
        {
            return Err(invalid("Unsupported retained image extraction identity."));
        }
        Ok(())
    }

    fn key(&self) -> Result<String, CommandError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(CommandError::serialization)?;
        Ok(sha256_hex(&bytes))
    }
}

pub(crate) struct CachedExtraction {
    pub result: RegionOcrResult,
    pub reused: bool,
}

/// An unavailable runtime fingerprint disables reuse, not OCR. The caller must
/// supply a contract from the native OCR boundary, never from frontend input.
pub(crate) fn get_or_extract(
    database: Option<&Database>,
    member: &ImageMember,
    contract: Option<&str>,
    extract: impl FnOnce() -> Result<RegionOcrResult, CommandError>,
) -> Result<CachedExtraction, CommandError> {
    let Some((database, contract)) = database.zip(contract) else {
        let result = extract()?;
        validate_result(member, &result)?;
        return Ok(CachedExtraction {
            result,
            reused: false,
        });
    };
    let identity = Identity::for_member(member, contract)?;
    let key = identity.key()?;
    let cached = database.with_connection(|connection| {
        require_member(connection, member)?;
        load(connection, &key, &identity)
    })?;
    let reused = cached.is_some();
    let result = match cached {
        Some(result) => result,
        None => extract()?,
    };
    validate_result(member, &result)?;
    let result = database
        .with_connection(|connection| publish(connection, member, &key, &identity, result))?;
    Ok(CachedExtraction { result, reused })
}

fn publish(
    connection: &mut Connection,
    member: &ImageMember,
    key: &str,
    identity: &Identity,
    result: RegionOcrResult,
) -> Result<RegionOcrResult, CommandError> {
    let json = serde_json::to_string(&result).map_err(CommandError::serialization)?;
    if json.len() > MAX_RESULT_BYTES {
        return Err(invalid(
            "The OCR result exceeds the retained evidence limit.",
        ));
    }
    let hash = sha256_hex(json.as_bytes());
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    require_member(&transaction, member)?;
    // A concurrent native extraction may have won publication. Never replace
    // immutable text under its existing identity; revalidate the winning result.
    let saved = load(&transaction, key, identity)?;
    if let Some(saved) = &saved {
        validate_result(member, saved)?;
    } else {
        transaction
            .execute(
                "INSERT INTO document_image_extractions (
                     extraction_key, version, pixel_sha256, pixel_contract,
                     quarter_turns, engine_contract, result_json, result_sha256
                 ) VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    key,
                    identity.pixel_sha256,
                    identity.pixel_contract,
                    identity.quarter_turns,
                    identity.engine_contract,
                    json,
                    hash
                ],
            )
            .map_err(CommandError::database)?;
    }
    transaction
        .execute(
            "INSERT OR IGNORE INTO document_image_extraction_owners (member_id, extraction_key)
             VALUES (?1, ?2)",
            params![member.id, key],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    Ok(saved.unwrap_or(result))
}

fn require_member(connection: &Connection, member: &ImageMember) -> Result<(), CommandError> {
    let matches: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_image_members
             WHERE id = ?1 AND pixel_sha256 = ?2 AND pixel_contract = ?3
               AND quarter_turns = ?4 AND sha256 = ?5 AND width = ?6 AND height = ?7)",
            params![
                member.id,
                member.pixel_sha256,
                member.pixel_contract,
                member.quarter_turns,
                member.sha256,
                member.width,
                member.height
            ],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !matches {
        return Err(CommandError::new(
            "image_set_changed",
            "The image source changed during extraction. Retained sibling work was preserved.",
        ));
    }
    Ok(())
}

fn validate_result(member: &ImageMember, result: &RegionOcrResult) -> Result<(), CommandError> {
    result
        .validate()
        .map_err(|_| invalid("The OCR result is incomplete or invalid."))?;
    let dimensions = if member.quarter_turns % 2 == 0 {
        (member.width, member.height)
    } else {
        (member.height, member.width)
    };
    if (result.width, result.height) != dimensions {
        return Err(invalid(
            "The OCR geometry does not match the retained image.",
        ));
    }
    Ok(())
}

fn load(
    connection: &Connection,
    key: &str,
    expected: &Identity,
) -> Result<Option<RegionOcrResult>, CommandError> {
    let saved = connection
        .query_row(
            "SELECT version, pixel_sha256, pixel_contract, quarter_turns, engine_contract,
                    result_json, result_sha256
             FROM document_image_extractions WHERE extraction_key = ?1",
            [key],
            |row| {
                Ok((
                    Identity {
                        version: row.get(0)?,
                        pixel_sha256: row.get(1)?,
                        pixel_contract: row.get(2)?,
                        quarter_turns: row.get(3)?,
                        engine_contract: row.get(4)?,
                    },
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    saved
        .map(|(identity, json, hash)| {
            if identity != *expected || identity.key()? != key {
                return Err(invalid(
                    "The retained extraction identity does not match its key.",
                ));
            }
            decode(&json, &hash)
        })
        .transpose()
}

fn decode(json: &str, hash: &str) -> Result<RegionOcrResult, CommandError> {
    if json.len() > MAX_RESULT_BYTES || sha256_hex(json.as_bytes()) != hash {
        return Err(invalid(
            "Retained image extraction failed its size or hash check.",
        ));
    }
    let result: RegionOcrResult =
        serde_json::from_str(json).map_err(|_| invalid("Unsupported retained OCR result."))?;
    result
        .validate()
        .map_err(|_| invalid("Invalid retained OCR evidence or geometry."))?;
    Ok(result)
}

/// Read-only startup/restore admission. Unknown result versions and truncated or
/// misbound evidence stop admission before migrations, repairs or journal writes.
pub(crate) fn validate(connection: &Connection) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT extraction_key, version, pixel_sha256, pixel_contract, quarter_turns,
                    engine_contract, result_json, result_sha256 FROM document_image_extractions",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let key: String = row.get(0).map_err(CommandError::database)?;
        let identity = Identity {
            version: row.get(1).map_err(CommandError::database)?,
            pixel_sha256: row.get(2).map_err(CommandError::database)?,
            pixel_contract: row.get(3).map_err(CommandError::database)?,
            quarter_turns: row.get(4).map_err(CommandError::database)?,
            engine_contract: row.get(5).map_err(CommandError::database)?,
        };
        if identity.key()? != key {
            return Err(invalid("A retained extraction key was altered."));
        }
        let json: String = row.get(6).map_err(CommandError::database)?;
        let hash: String = row.get(7).map_err(CommandError::database)?;
        let result = decode(&json, &hash)?;
        validate_owners(connection, &key, &identity, &result)?;
    }
    let dangling: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_image_extraction_owners AS o
             LEFT JOIN document_image_members AS m ON m.id = o.member_id
             LEFT JOIN document_image_extractions AS e ON e.extraction_key = o.extraction_key
             WHERE m.id IS NULL OR e.extraction_key IS NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if dangling {
        return Err(invalid(
            "An image extraction association has no retained source or result.",
        ));
    }
    Ok(())
}

fn validate_owners(
    connection: &Connection,
    key: &str,
    identity: &Identity,
    result: &RegionOcrResult,
) -> Result<(), CommandError> {
    let mut owners = connection
        .prepare(
            "SELECT m.pixel_sha256, m.pixel_contract, m.width, m.height
             FROM document_image_extraction_owners AS o
             JOIN document_image_members AS m ON m.id = o.member_id
             WHERE o.extraction_key = ?1",
        )
        .map_err(CommandError::database)?;
    let mut owners = owners.query([key]).map_err(CommandError::database)?;
    let mut count = 0;
    while let Some(owner) = owners.next().map_err(CommandError::database)? {
        count += 1;
        let hash: String = owner.get(0).map_err(CommandError::database)?;
        let contract: String = owner.get(1).map_err(CommandError::database)?;
        let width: u32 = owner.get(2).map_err(CommandError::database)?;
        let height: u32 = owner.get(3).map_err(CommandError::database)?;
        let dimensions = if identity.quarter_turns % 2 == 0 {
            (width, height)
        } else {
            (height, width)
        };
        if hash != identity.pixel_sha256
            || contract != identity.pixel_contract
            || dimensions != (result.width, result.height)
        {
            return Err(invalid(
                "Retained extraction belongs to different source pixels.",
            ));
        }
    }
    if count == 0 {
        return Err(invalid("Retained image extraction has no source owner."));
    }
    Ok(())
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_extraction", message)
}

#[cfg(test)]
mod tests;
