use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::{
    anydoc_document_adapter, assets::detect_image_bytes, document_domain::DocumentFormat,
    error::CommandError, paths::AppPaths,
};

const MAX_SOURCE_DOCUMENT_BYTES: usize = 512 * 1024 * 1024;
const MAX_PAGE_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_SOURCE_ASSET_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn export_document_files(
    snapshot: &Connection,
    paths: &AppPaths,
    transaction: &Transaction<'_>,
) -> Result<i64, CommandError> {
    if !table_exists(snapshot, "documents")? {
        return Ok(0);
    }

    let mut exported = 0_i64;
    let mut document_statement = snapshot
        .prepare(
            "SELECT id, source_relative_path, source_format, source_media_type,
                    byte_size, sha256
             FROM documents
             ORDER BY id",
        )
        .map_err(CommandError::database)?;
    let documents = document_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (document_id, relative_path, source_format, mime_type, byte_size, sha256) in documents {
        validate_document_path(&document_id, None, "source_document", &relative_path)?;
        let data = read_managed_file(
            &paths.resolve_relative(&relative_path),
            byte_size,
            &sha256,
            MAX_SOURCE_DOCUMENT_BYTES,
            "A managed source document is missing or does not match its database record.",
        )?;
        let format = DocumentFormat::from_db(&source_format)?;
        validate_source_bytes(format, &relative_path, &data, "invalid_document_file")?;
        transaction
            .execute(
                "INSERT INTO document_files (
                     file_key, document_id, page_number, file_kind,
                     relative_path, mime_type, byte_size, sha256, data
                 ) VALUES (?1, ?2, NULL, 'source_document', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("source:{document_id}"),
                    document_id,
                    relative_path,
                    mime_type,
                    byte_size,
                    sha256,
                    data,
                ],
            )
            .map_err(CommandError::database)?;
        exported = exported.saturating_add(1);
    }

    if table_exists(snapshot, "document_pages")? {
        let mut page_statement = snapshot
            .prepare(
                "SELECT document_id, page_number, render_relative_path,
                        render_mime_type, render_byte_size, render_sha256
                 FROM document_pages
                 ORDER BY document_id, page_number",
            )
            .map_err(CommandError::database)?;
        let pages = page_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for (document_id, page_number, relative_path, mime_type, byte_size, sha256) in pages {
            if mime_type != "image/png" {
                return Err(CommandError::new(
                    "invalid_document_file",
                    "A rendered PDF page uses an unsupported file type.",
                ));
            }
            validate_document_path(
                &document_id,
                Some(page_number),
                "page_render",
                &relative_path,
            )?;
            let data = read_managed_file(
                &paths.resolve_relative(&relative_path),
                byte_size,
                &sha256,
                MAX_PAGE_RENDER_BYTES,
                "A rendered PDF page is missing or does not match its database record.",
            )?;
            if !is_png(&data) {
                return Err(CommandError::new(
                    "invalid_document_file",
                    "A rendered PDF page no longer has a valid PNG signature.",
                ));
            }
            transaction
                .execute(
                    "INSERT INTO document_files (
                         file_key, document_id, page_number, file_kind,
                         relative_path, mime_type, byte_size, sha256, data
                     ) VALUES (?1, ?2, ?3, 'page_render', ?4, ?5, ?6, ?7, ?8)",
                    params![
                        format!("page:{document_id}:{page_number:06}"),
                        document_id,
                        page_number,
                        relative_path,
                        mime_type,
                        byte_size,
                        sha256,
                        data,
                    ],
                )
                .map_err(CommandError::database)?;
            exported = exported.saturating_add(1);
        }
    }

    if table_exists(snapshot, "document_source_assets")? {
        let mut asset_statement = snapshot
            .prepare(
                "SELECT document_id, asset_id, media_type, relative_path,
                        byte_size, sha256
                 FROM document_source_assets
                 WHERE role <> 'unit_render'
                 ORDER BY document_id, asset_id",
            )
            .map_err(CommandError::database)?;
        let assets = asset_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        for (document_id, asset_id, mime_type, relative_path, byte_size, sha256) in assets {
            validate_document_path(&document_id, None, "source_asset", &relative_path)?;
            let data = read_managed_file(
                &paths.resolve_relative(&relative_path),
                byte_size,
                &sha256,
                MAX_SOURCE_ASSET_BYTES,
                "A managed document image is missing or does not match its database record.",
            )?;
            validate_source_asset_bytes(&data, &mime_type, "invalid_document_file")?;
            transaction
                .execute(
                    "INSERT INTO document_files (
                         file_key, document_id, page_number, file_kind,
                         relative_path, mime_type, byte_size, sha256, data
                     ) VALUES (?1, ?2, NULL, 'source_asset', ?3, ?4, ?5, ?6, ?7)",
                    params![
                        format!("asset:{document_id}:{asset_id}"),
                        document_id,
                        relative_path,
                        mime_type,
                        byte_size,
                        sha256,
                        data,
                    ],
                )
                .map_err(CommandError::database)?;
            exported = exported.saturating_add(1);
        }
    }
    Ok(exported)
}

pub(crate) fn validate_document_files(
    package: &Connection,
    snapshot: &Connection,
    extraction_root: Option<&Path>,
) -> Result<i64, CommandError> {
    let has_documents = table_exists(snapshot, "documents")?;
    let expected_sources = if has_documents {
        snapshot
            .query_row("SELECT COUNT(*) FROM documents", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(CommandError::database)?
    } else {
        0
    };
    let expected_pages = if table_exists(snapshot, "document_pages")? {
        snapshot
            .query_row("SELECT COUNT(*) FROM document_pages", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(CommandError::database)?
    } else {
        0
    };
    let expected_assets = if table_exists(snapshot, "document_source_assets")? {
        snapshot
            .query_row(
                "SELECT COUNT(*) FROM document_source_assets WHERE role <> 'unit_render'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::database)?
    } else {
        0
    };
    let expected = expected_sources
        .saturating_add(expected_pages)
        .saturating_add(expected_assets);
    let actual: i64 = package
        .query_row("SELECT COUNT(*) FROM document_files", [], |row| row.get(0))
        .map_err(CommandError::database)?;
    if actual != expected {
        return Err(CommandError::new(
            "invalid_backup",
            "The backup does not contain every managed document source, source image, and PDF page render.",
        ));
    }

    let mut statement = package
        .prepare(
            "SELECT document_id, page_number, file_kind, relative_path,
                    mime_type, byte_size, sha256, data
             FROM document_files
             ORDER BY file_key",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Vec<u8>>(7)?,
            ))
        })
        .map_err(CommandError::database)?;

    for row in rows {
        let (
            document_id,
            page_number,
            file_kind,
            relative_path,
            mime_type,
            byte_size,
            sha256,
            data,
        ) = row.map_err(CommandError::database)?;
        validate_document_path(&document_id, page_number, &file_kind, &relative_path)?;

        let (stored, source_format) = match file_kind.as_str() {
            "source_pdf" => (
                snapshot
                    .query_row(
                        "SELECT source_relative_path, 'application/pdf', byte_size, sha256
                         FROM documents WHERE id = ?1",
                        [document_id.as_str()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(CommandError::database)?,
                Some(DocumentFormat::Pdf),
            ),
            "source_document" => {
                let row = snapshot
                    .query_row(
                        "SELECT source_relative_path, source_media_type, byte_size, sha256,
                                source_format
                         FROM documents WHERE id = ?1",
                        [document_id.as_str()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(CommandError::database)?;
                match row {
                    Some((path, media, size, hash, format)) => (
                        Some((path, media, size, hash)),
                        Some(DocumentFormat::from_db(&format)?),
                    ),
                    None => (None, None),
                }
            }
            "page_render" => (
                snapshot
                    .query_row(
                        "SELECT render_relative_path, render_mime_type,
                                render_byte_size, render_sha256
                         FROM document_pages
                         WHERE document_id = ?1 AND page_number = ?2",
                        params![document_id, page_number],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(CommandError::database)?,
                None,
            ),
            "source_asset" => (
                if table_exists(snapshot, "document_source_assets")? {
                    snapshot
                        .query_row(
                            "SELECT relative_path, media_type, byte_size, sha256
                             FROM document_source_assets
                             WHERE document_id = ?1 AND relative_path = ?2
                               AND role <> 'unit_render'",
                            params![document_id, relative_path],
                            |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, i64>(2)?,
                                    row.get::<_, String>(3)?,
                                ))
                            },
                        )
                        .optional()
                        .map_err(CommandError::database)?
                } else {
                    None
                },
                None,
            ),
            _ => (None, None),
        };
        if stored
            != Some((
                relative_path.clone(),
                mime_type.clone(),
                byte_size,
                sha256.clone(),
            ))
        {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged document file does not match the SQLite document ledger.",
            ));
        }
        let maximum = match file_kind.as_str() {
            "source_pdf" | "source_document" => MAX_SOURCE_DOCUMENT_BYTES,
            "page_render" | "source_asset" => MAX_PAGE_RENDER_BYTES,
            _ => 0,
        };
        if maximum == 0
            || data.is_empty()
            || data.len() > maximum
            || i64::try_from(data.len()).unwrap_or(i64::MAX) != byte_size
            || sha256_hex(&data) != sha256
        {
            return Err(CommandError::new(
                "invalid_backup",
                "A packaged document file failed its size or hash validation.",
            ));
        }
        match file_kind.as_str() {
            "source_pdf" | "source_document" => validate_source_bytes(
                source_format.unwrap_or(DocumentFormat::Pdf),
                &relative_path,
                &data,
                "invalid_backup",
            )?,
            "page_render" => {
                if !is_png(&data) || mime_type != "image/png" {
                    return Err(CommandError::new(
                        "invalid_backup",
                        "A packaged rendered page has an invalid PNG signature.",
                    ));
                }
            }
            "source_asset" => {
                validate_source_asset_bytes(&data, &mime_type, "invalid_backup")?;
            }
            _ => unreachable!("file kind was rejected above"),
        }
        if let Some(root) = extraction_root {
            let destination = root.join(&relative_path);
            write_new_file(&destination, &data)?;
            if let Some(parent) = destination.parent() {
                sync_directory(parent)?;
            }
        }
    }
    Ok(actual)
}

pub(crate) fn table_exists(connection: &Connection, table: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn read_managed_file(
    path: &Path,
    expected_size: i64,
    expected_sha256: &str,
    maximum_size: usize,
    failure_message: &str,
) -> Result<Vec<u8>, CommandError> {
    let data =
        fs::read(path).map_err(|_| CommandError::new("missing_document_file", failure_message))?;
    if data.is_empty()
        || data.len() > maximum_size
        || i64::try_from(data.len()).unwrap_or(i64::MAX) != expected_size
        || sha256_hex(&data) != expected_sha256
    {
        return Err(CommandError::new("invalid_document_file", failure_message));
    }
    Ok(data)
}

fn validate_source_bytes(
    format: DocumentFormat,
    relative_path: &str,
    data: &[u8],
    error_code: &'static str,
) -> Result<(), CommandError> {
    let filename = Path::new(relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if anydoc_document_adapter::detected_document_format(data, filename).ok() == Some(format) {
        Ok(())
    } else {
        Err(CommandError::new(
            error_code,
            "A managed source document does not match its recorded document format.",
        ))
    }
}

fn validate_source_asset_bytes(
    data: &[u8],
    mime_type: &str,
    error_code: &'static str,
) -> Result<(), CommandError> {
    let detected = detect_image_bytes(data).map_err(|_| {
        CommandError::new(
            error_code,
            "A managed document image has an invalid or unsupported image signature.",
        )
    })?;
    if detected.0 == mime_type {
        Ok(())
    } else {
        Err(CommandError::new(
            error_code,
            "A managed document image does not match its recorded image type.",
        ))
    }
}

fn validate_document_path(
    document_id: &str,
    page_number: Option<i64>,
    file_kind: &str,
    relative_path: &str,
) -> Result<(), CommandError> {
    if document_id.len() != 32
        || document_id
            .chars()
            .any(|character| !character.is_ascii_hexdigit())
    {
        return Err(CommandError::new(
            "invalid_backup",
            "A packaged document has an invalid identity.",
        ));
    }
    let components = Path::new(relative_path).components().collect::<Vec<_>>();
    let valid = match (file_kind, page_number) {
        ("source_pdf", None) => {
            relative_path == AppPaths::document_source_relative_path(document_id)
        }
        ("source_document", None) => valid_source_document_path(document_id, relative_path),
        ("source_asset", None) => valid_source_asset_path(document_id, &components),
        ("page_render", Some(page)) if page >= 1 => {
            components.len() == 4
                && components[0] == Component::Normal("documents".as_ref())
                && components[1] == Component::Normal(document_id.as_ref())
                && components[2] == Component::Normal("pages".as_ref())
                && components[3] == Component::Normal(format!("page-{page:06}.png").as_ref())
        }
        _ => false,
    };
    if !valid {
        return Err(CommandError::new(
            "invalid_backup",
            "A packaged document path escapes the managed document directory.",
        ));
    }
    Ok(())
}

fn valid_source_document_path(document_id: &str, relative_path: &str) -> bool {
    let path = Path::new(relative_path);
    let extension = path.extension().and_then(|value| value.to_str());
    extension.is_some_and(|extension| {
        !extension.is_empty()
            && extension.len() <= 16
            && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            && relative_path
                == AppPaths::document_source_relative_path_with_extension(document_id, extension)
    })
}

fn valid_source_asset_path(document_id: &str, components: &[Component<'_>]) -> bool {
    if components.len() != 4
        || components[0] != Component::Normal("documents".as_ref())
        || components[1] != Component::Normal(document_id.as_ref())
        || components[2] != Component::Normal("assets".as_ref())
    {
        return false;
    }
    let Component::Normal(file_name) = components[3] else {
        return false;
    };
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    stem.strip_prefix("asset-").is_some_and(|ordinal| {
        !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
    }) && !extension.is_empty()
        && extension.len() <= 16
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_png(data: &[u8]) -> bool {
    data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn write_new_file(path: &Path, data: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(CommandError::io)?;
    file.write_all(data).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), CommandError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(CommandError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_document_paths_that_do_not_match_the_ledger_identity() {
        assert!(validate_document_path(
            &"a".repeat(32),
            None,
            "source_document",
            "documents/other/source.docx"
        )
        .is_err());
        assert!(validate_document_path(
            &"a".repeat(32),
            Some(2),
            "page_render",
            &format!("documents/{}/pages/page-000002.png", "a".repeat(32))
        )
        .is_ok());
        assert!(validate_document_path(
            &"a".repeat(32),
            None,
            "source_asset",
            &format!("documents/{}/assets/asset-000002.png", "a".repeat(32))
        )
        .is_ok());
    }
}
