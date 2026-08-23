use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::error::CommandError;

const HASH_BUFFER_BYTES: usize = 128 * 1024;

pub(crate) fn normalize_optional_semantic_text(
    value: Option<&str>,
    maximum: usize,
) -> Option<String> {
    value
        .map(|value| normalize_semantic_text(value, maximum))
        .filter(|value| !value.is_empty())
}

pub(crate) fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn format_source_units(unit_numbers: &[i64]) -> String {
    match unit_numbers {
        [] => "unit".to_string(),
        [unit] => format!("unit {unit}"),
        units => format!(
            "units {}–{}",
            units.first().copied().unwrap_or(1),
            units.last().copied().unwrap_or(1)
        ),
    }
}

pub(crate) fn hash_file(path: &Path) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    let byte_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    if byte_size <= 0 {
        return Err(CommandError::new(
            "document_source_empty",
            "The managed source document is empty.",
        ));
    }
    let mut file = File::open(path).map_err(CommandError::io)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok((byte_size, format!("{:x}", digest.finalize())))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn require_one(
    changed: usize,
    code: &str,
    message: &str,
) -> Result<(), CommandError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(CommandError::new(code, message))
    }
}
