//! Browser-selected image bytes enter the same opaque transfer ledger as native
//! file selections. No caller-controlled path, URL, or claimed content hash is used.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::{
    assets::detect_image_bytes,
    cards::questions::occlusion::{CanonicalImage, MAX_IMAGE_BYTES},
    database::Database,
    document_canonical::sha256_hex,
    error::CommandError,
    external_files::{ExternalFileKind, ExternalImportPurpose, StagedExternalFileDto},
    paths::AppPaths,
    util::{unix_time_millis, validate_entity_id},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageImageFileInput {
    pub operation_id: String,
    pub display_name: String,
    pub data_base64: String,
}

const BROWSER_IMAGE_CHUNK_MAX_BYTES: usize = 512 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageImageFileChunkInput {
    pub operation_id: String,
    pub display_name: String,
    pub total_bytes: i64,
    pub offset: i64,
    pub data_base64: String,
    pub final_chunk: bool,
}

#[tauri::command]
pub async fn stage_image_collection_file_chunk(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StageImageFileChunkInput,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    let database = database.inner().clone();
    let paths = paths.inner().clone();
    tauri::async_runtime::spawn_blocking(move || stage_chunk(&database, &paths, input))
        .await
        .map_err(|_| invalid("Image upload was interrupted. Retry the image."))?
}

#[tauri::command]
pub async fn stage_image_collection_file(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: StageImageFileInput,
) -> Result<StagedExternalFileDto, CommandError> {
    let database = database.inner().clone();
    let paths = paths.inner().clone();
    tauri::async_runtime::spawn_blocking(move || stage(&database, &paths, input))
        .await
        .map_err(|_| invalid("Image import was interrupted. Retry the image."))?
}

fn stage_chunk(
    database: &Database,
    paths: &AppPaths,
    input: StageImageFileChunkInput,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    validate_entity_id(&input.operation_id, "image upload")?;
    let name = validate_image_name(&input.display_name)?;
    let total_bytes = usize::try_from(input.total_bytes)
        .map_err(|_| invalid("The image upload size is invalid."))?;
    let offset = usize::try_from(input.offset)
        .map_err(|_| invalid("The image upload offset is invalid."))?;
    if total_bytes == 0 || total_bytes > MAX_IMAGE_BYTES || offset > total_bytes {
        return Err(invalid("Use a nonempty still image no larger than 25 MB."));
    }
    if let Some(existing) = completed_chunk_upload(
        database,
        paths,
        &input.operation_id,
        name,
        input.total_bytes,
    )? {
        return Ok(Some(existing));
    }
    if input.data_base64.len() > BROWSER_IMAGE_CHUNK_MAX_BYTES.div_ceil(3) * 4 + 8 {
        return Err(invalid("The image upload chunk exceeds the bounded transfer size."));
    }
    let chunk = STANDARD
        .decode(&input.data_base64)
        .map_err(|_| invalid("The image upload chunk is not valid base64."))?;
    if chunk.is_empty()
        || chunk.len() > BROWSER_IMAGE_CHUNK_MAX_BYTES
        || offset.saturating_add(chunk.len()) > total_bytes
    {
        return Err(invalid("The image upload chunk is outside the expected file bounds."));
    }

    let relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);
    let path = paths.resolve_relative(&relative);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Invalid managed image staging destination."))?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(CommandError::io)?;
    let current = usize::try_from(file.metadata().map_err(CommandError::io)?.len())
        .map_err(|_| invalid("The partial image upload is too large."))?;
    if current < offset || current > total_bytes {
        return Err(invalid("The partial image upload has an invalid byte range."));
    }

    let overlap = current.saturating_sub(offset).min(chunk.len());
    if overlap > 0 {
        file.seek(SeekFrom::Start(offset as u64))
            .map_err(CommandError::io)?;
        let mut existing = vec![0_u8; overlap];
        file.read_exact(&mut existing).map_err(CommandError::io)?;
        if existing != chunk[..overlap] {
            return Err(invalid(
                "This image-upload identity was retried with different bytes.",
            ));
        }
    }
    if overlap < chunk.len() {
        file.seek(SeekFrom::End(0)).map_err(CommandError::io)?;
        file.write_all(&chunk[overlap..])
            .map_err(CommandError::io)?;
        file.sync_data().map_err(CommandError::io)?;
    }

    let written = usize::try_from(file.metadata().map_err(CommandError::io)?.len())
        .map_err(|_| invalid("The partial image upload is too large."))?;
    if written > total_bytes {
        return Err(invalid("The partial image upload exceeds the expected size."));
    }
    if !input.final_chunk {
        return Ok(None);
    }
    if written != total_bytes {
        return Err(invalid(
            "The final image upload chunk arrived before all bytes were staged.",
        ));
    }
    drop(file);

    let bytes = fs::read(&path).map_err(CommandError::io)?;
    let staged = stage(
        database,
        paths,
        StageImageFileInput {
            operation_id: input.operation_id,
            display_name: name.to_string(),
            data_base64: STANDARD.encode(bytes),
        },
    )?;
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(CommandError::io(error)),
    }
    Ok(Some(staged))
}

fn completed_chunk_upload(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    expected_name: &str,
    expected_bytes: i64,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    let row = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT purpose, detected_kind, display_name, media_type, byte_size, sha256,
                        relative_path, state, expires_at
                 FROM staged_external_imports WHERE id = ?1 AND operation_id = ?1",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let Some((purpose, kind, name, media_type, byte_size, sha256, relative, state, expires_at)) =
        row
    else {
        return Ok(None);
    };
    if purpose != "image"
        || name != expected_name
        || byte_size != expected_bytes
        || !matches!(state.as_str(), "staged" | "consumed")
    {
        return Err(invalid(
            "This image-upload identity belongs to a different selection.",
        ));
    }
    let detected_kind = ExternalFileKind::parse(&kind)?;
    if media_type != detected_kind.media_type() {
        return Err(invalid("The staged image media type is inconsistent."));
    }
    let dto = StagedExternalFileDto {
        id: operation_id.to_string(),
        purpose: ExternalImportPurpose::Image,
        detected_kind,
        display_name: name,
        media_type,
        byte_size,
        sha256,
        expires_at,
    };
    if state == "staged" {
        verify_bytes(&paths.resolve_relative(&relative), &dto)?;
    }
    Ok(Some(dto))
}

fn validate_image_name(value: &str) -> Result<&str, CommandError> {
    let name = value.trim();
    if name.is_empty()
        || name.chars().count() > 255
        || name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(invalid("The image filename is invalid."));
    }
    Ok(name)
}

fn stage(
    database: &Database,
    paths: &AppPaths,
    input: StageImageFileInput,
) -> Result<StagedExternalFileDto, CommandError> {
    validate_entity_id(&input.operation_id, "image upload")?;
    let name = validate_image_name(&input.display_name)?;
    if input.data_base64.is_empty() || input.data_base64.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
        return Err(invalid("Use a nonempty still image no larger than 25 MB."));
    }
    let bytes = STANDARD
        .decode(&input.data_base64)
        .map_err(|_| invalid("The image transfer is not valid base64."))?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid("Use a nonempty still image no larger than 25 MB."));
    }
    let kind = match detect_image_bytes(&bytes)?.0 {
        "image/png" => ExternalFileKind::Png,
        "image/jpeg" => ExternalFileKind::Jpeg,
        "image/webp" => ExternalFileKind::Webp,
        _ => return Err(invalid("Choose a PNG, JPEG, or WebP still image.")),
    };
    // Reuse native bounded decoding, animation rejection, and EXIF admission.
    // Neither a filename nor a browser MIME type proves image validity.
    CanonicalImage::decode(&bytes)?;
    let hash = sha256_hex(&bytes);
    let relative = AppPaths::staged_import_relative_path(&input.operation_id, kind.extension());
    let now = unix_time_millis()?;
    let expires_at = now.saturating_add(7 * 24 * 60 * 60 * 1_000);
    let dto = StagedExternalFileDto {
        id: input.operation_id,
        purpose: ExternalImportPurpose::Image,
        detected_kind: kind,
        display_name: name.to_string(),
        media_type: kind.media_type().to_string(),
        byte_size: bytes.len() as i64,
        sha256: hash,
        expires_at,
    };
    if let Some(existing) = existing(database, paths, &dto, &relative, now)? {
        return Ok(existing);
    }

    let path = paths.resolve_relative(&relative);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Invalid managed image destination."))?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(&bytes).map_err(CommandError::io)?;
            file.sync_all().map_err(CommandError::io)?;
            #[cfg(unix)]
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(CommandError::io)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // A matching orphan from interrupted staging is recoverable. Never
            // overwrite a different file, symlink, or another operation's receipt.
            verify_bytes(&path, &dto)?;
        }
        Err(error) => return Err(CommandError::io(error)),
    }
    database.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO staged_external_imports (
                    id, operation_id, purpose, detected_kind, display_name, media_type,
                    byte_size, sha256, relative_path, state, created_at, updated_at, expires_at
                 ) VALUES (?1, ?1, 'image', ?2, ?3, ?4, ?5, ?6, ?7, 'staged', ?8, ?8, ?9)
                 ON CONFLICT(operation_id) DO NOTHING",
                params![
                    dto.id,
                    kind.as_str(),
                    dto.display_name,
                    dto.media_type,
                    dto.byte_size,
                    dto.sha256,
                    relative,
                    now,
                    expires_at
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })?;
    existing(database, paths, &dto, &relative, now)?
        .ok_or_else(|| invalid("The image upload receipt could not be read."))
}

fn existing(
    database: &Database,
    paths: &AppPaths,
    expected: &StagedExternalFileDto,
    relative: &str,
    now: i64,
) -> Result<Option<StagedExternalFileDto>, CommandError> {
    database.with_connection(|connection| {
        let row = connection
            .query_row(
                "SELECT purpose, detected_kind, display_name, byte_size, sha256,
                        relative_path, state, expires_at
                 FROM staged_external_imports WHERE id = ?1 AND operation_id = ?1",
                [&expected.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((purpose, kind, name, size, hash, path, state, expires_at)) = row else {
            return Ok(None);
        };
        if purpose != "image"
            || kind != expected.detected_kind.as_str()
            || name != expected.display_name
            || size != expected.byte_size
            || hash != expected.sha256
            || path != relative
            || !matches!(state.as_str(), "staged" | "consumed")
            || (state == "staged" && expires_at <= now)
        {
            return Err(invalid(
                "This image-upload identity belongs to a different or expired selection.",
            ));
        }
        if state == "staged" {
            verify_bytes(&paths.resolve_relative(relative), expected)?;
        }
        Ok(Some(StagedExternalFileDto {
            expires_at,
            ..expected.clone()
        }))
    })
}

fn verify_bytes(path: &Path, expected: &StagedExternalFileDto) -> Result<(), CommandError> {
    let metadata = fs::symlink_metadata(path).map_err(CommandError::io)?;
    if !metadata.file_type().is_file() || metadata.len() != expected.byte_size as u64 {
        return Err(invalid("The staged image no longer matches its upload."));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(CommandError::io)?
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(CommandError::io)?;
    if sha256_hex(&bytes) != expected.sha256 {
        return Err(invalid("The staged image no longer matches its upload."));
    }
    Ok(())
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_upload", message)
}

#[cfg(test)]
mod tests {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

    use super::*;
    use crate::documents::image_sets::{admission, test_support::Fixture};

    fn input(value: u8) -> StageImageFileInput {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&[value; 4 * 3 * 4], 4, 3, ExtendedColorType::Rgba8)
            .unwrap();
        StageImageFileInput {
            operation_id: "c".repeat(32),
            display_name: "Pasted image.png".to_string(),
            data_base64: STANDARD.encode(bytes),
        }
    }

    #[test]
    fn browser_chunks_resume_identical_offsets_and_finalize_once() {
        let f = Fixture::new();
        let original = input(17);
        let bytes = STANDARD.decode(&original.data_base64).unwrap();
        let split = bytes.len() / 2;
        let first = StageImageFileChunkInput {
            operation_id: original.operation_id.clone(),
            display_name: original.display_name.clone(),
            total_bytes: bytes.len() as i64,
            offset: 0,
            data_base64: STANDARD.encode(&bytes[..split]),
            final_chunk: false,
        };
        assert!(stage_chunk(&f.database, &f.paths, first).unwrap().is_none());
        let retry = StageImageFileChunkInput {
            operation_id: original.operation_id.clone(),
            display_name: original.display_name.clone(),
            total_bytes: bytes.len() as i64,
            offset: 0,
            data_base64: STANDARD.encode(&bytes[..split]),
            final_chunk: false,
        };
        assert!(stage_chunk(&f.database, &f.paths, retry).unwrap().is_none());
        let final_chunk = StageImageFileChunkInput {
            operation_id: original.operation_id.clone(),
            display_name: original.display_name.clone(),
            total_bytes: bytes.len() as i64,
            offset: split as i64,
            data_base64: STANDARD.encode(&bytes[split..]),
            final_chunk: true,
        };
        let staged = stage_chunk(&f.database, &f.paths, final_chunk).unwrap().unwrap();
        assert_eq!(staged.byte_size, bytes.len() as i64);
        assert_eq!(f.count("staged_external_imports"), 1);

        let repeated_final = StageImageFileChunkInput {
            operation_id: original.operation_id,
            display_name: original.display_name,
            total_bytes: bytes.len() as i64,
            offset: split as i64,
            data_base64: STANDARD.encode(&bytes[split..]),
            final_chunk: true,
        };
        assert_eq!(
            stage_chunk(&f.database, &f.paths, repeated_final)
                .unwrap()
                .unwrap()
                .sha256,
            staged.sha256
        );
    }

    #[test]
    fn changed_overlapping_chunk_is_rejected_without_overwriting_partial_bytes() {
        let f = Fixture::new();
        let original = input(17);
        let bytes = STANDARD.decode(&original.data_base64).unwrap();
        let split = bytes.len() / 2;
        assert!(stage_chunk(
            &f.database,
            &f.paths,
            StageImageFileChunkInput {
                operation_id: original.operation_id.clone(),
                display_name: original.display_name.clone(),
                total_bytes: bytes.len() as i64,
                offset: 0,
                data_base64: STANDARD.encode(&bytes[..split]),
                final_chunk: false,
            }
        )
        .unwrap()
        .is_none());
        let mut changed = bytes[..split].to_vec();
        changed[0] ^= 0xff;
        let error = stage_chunk(
            &f.database,
            &f.paths,
            StageImageFileChunkInput {
                operation_id: original.operation_id,
                display_name: original.display_name,
                total_bytes: bytes.len() as i64,
                offset: 0,
                data_base64: STANDARD.encode(changed),
                final_chunk: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_image_upload");
    }

    #[test]
    fn browser_bytes_use_native_admission_and_a_retry_stable_transfer() {
        let f = Fixture::new();
        let first = stage(&f.database, &f.paths, input(17)).unwrap();
        let retry = stage(&f.database, &f.paths, input(17)).unwrap();
        assert_eq!(first.id, retry.id);
        assert_eq!(first.sha256, retry.sha256);
        assert_eq!(f.count("staged_external_imports"), 1);
        let inspected = admission::inspect(&f.database, &f.paths, &[first.id]).unwrap();
        assert!(inspected.can_import);
        assert_eq!(inspected.members[0].image.as_ref().unwrap().width, 4);
    }

    #[test]
    fn changed_retry_and_invalid_input_leave_prior_selection_intact() {
        let f = Fixture::new();
        let saved = stage(&f.database, &f.paths, input(17)).unwrap();
        assert!(stage(&f.database, &f.paths, input(19)).is_err());
        let mut bad = input(17);
        bad.display_name = "../outside.png".to_string();
        assert!(stage(&f.database, &f.paths, bad).is_err());
        let mut bad = input(17);
        bad.data_base64 = STANDARD.encode(b"not an image");
        assert!(stage(&f.database, &f.paths, bad).is_err());
        assert_eq!(f.count("staged_external_imports"), 1);
        assert_eq!(
            stage(&f.database, &f.paths, input(17)).unwrap().sha256,
            saved.sha256
        );
    }

    #[test]
    fn matching_interrupted_file_is_adopted_without_overwriting_other_bytes() {
        let f = Fixture::new();
        let upload = input(17);
        let path = f
            .paths
            .resolve_relative(&AppPaths::staged_import_relative_path(
                &upload.operation_id,
                "png",
            ));
        fs::write(&path, STANDARD.decode(&upload.data_base64).unwrap()).unwrap();
        assert!(stage(&f.database, &f.paths, upload).is_ok());
        assert_eq!(f.count("staged_external_imports"), 1);
    }
}
