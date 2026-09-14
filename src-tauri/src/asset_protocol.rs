//! Asset-ID-only WebView protocol with bounded media range serving.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

use rusqlite::OptionalExtension;
use tauri::{
    http::{header, Method, Request, Response, StatusCode},
    Manager, Runtime, UriSchemeContext,
};

use crate::{database::Database, paths::AppPaths, util::validate_entity_id};

// Browser media stacks commonly issue open-ended ranges. Bound every partial
// response so a single WebView request cannot allocate an entire media asset.
const MAX_RANGE_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

pub fn respond<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if context.webview_label() != "main" {
        return response(StatusCode::FORBIDDEN, "text/plain", b"forbidden".to_vec());
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::ALLOW, "GET, HEAD")
            .header(header::CACHE_CONTROL, "private, max-age=300")
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }
    let asset_id = request.uri().path().trim_matches('/');
    if validate_entity_id(asset_id, "asset").is_err() {
        return response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            b"invalid asset".to_vec(),
        );
    }
    let database = context.app_handle().state::<Database>();
    let paths = context.app_handle().state::<AppPaths>();
    // Resolve ledger state while holding SQLite, then release it before any file
    // reads. Slow media reads must never retain the managed database mutex.
    let resolved = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT a.mime_type, a.byte_size,
                        CASE
                          WHEN a.lifecycle_state = 'ready' THEN a.relative_path
                          WHEN a.lifecycle_state = 'staged'
                               AND EXISTS (
                                 SELECT 1 FROM asset_drafts d
                                 WHERE d.id = a.draft_id
                                   AND d.committed_at IS NULL
                                   AND d.canceled_at IS NULL
                               )
                          THEN a.staged_relative_path
                          ELSE NULL
                        END
                 FROM assets a
                 WHERE a.id = ?1 AND a.deleted_at IS NULL",
                [asset_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(crate::error::CommandError::database)
    });
    let Ok(Some((mime_type, expected_size, Some(relative)))) = resolved else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"asset not found".to_vec(),
        );
    };
    if !safe_relative_path(&relative) {
        return response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            b"invalid asset path".to_vec(),
        );
    }
    let Ok(expected_size) = u64::try_from(expected_size) else {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    };
    let path = paths.resolve_relative(&relative);
    let Ok(metadata) = fs::metadata(&path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"asset missing".to_vec(),
        );
    };
    if !metadata.is_file() || metadata.len() != expected_size || expected_size == 0 {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }

    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let range = match parse_single_range(range_header, expected_size) {
        Ok(range) => range,
        Err(()) => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{expected_size}"))
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "private, max-age=300")
                .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
    };
    let head_only = request.method() == Method::HEAD;
    match range {
        Some((start, end)) => {
            ranged_response(&path, &mime_type, expected_size, start, end, head_only)
        }
        None => full_response(&path, &mime_type, expected_size, head_only),
    }
}

fn full_response(path: &Path, mime_type: &str, size: u64, head_only: bool) -> Response<Vec<u8>> {
    let body = if head_only {
        Vec::new()
    } else {
        let Ok(bytes) = fs::read(path) else {
            return response(
                StatusCode::NOT_FOUND,
                "text/plain",
                b"asset missing".to_vec(),
            );
        };
        if u64::try_from(bytes.len()).ok() != Some(size) {
            return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
        }
        bytes
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_LENGTH, size.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn ranged_response(
    path: &Path,
    mime_type: &str,
    total: u64,
    start: u64,
    end: u64,
    head_only: bool,
) -> Response<Vec<u8>> {
    let length = end.saturating_sub(start).saturating_add(1);
    let body = if head_only {
        Vec::new()
    } else {
        let Ok(mut file) = File::open(path) else {
            return response(
                StatusCode::NOT_FOUND,
                "text/plain",
                b"asset missing".to_vec(),
            );
        };
        if file.seek(SeekFrom::Start(start)).is_err() {
            return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
        }
        let Ok(capacity) = usize::try_from(length) else {
            return response(StatusCode::RANGE_NOT_SATISFIABLE, "text/plain", Vec::new());
        };
        let mut bytes = Vec::with_capacity(capacity);
        if file.take(length).read_to_end(&mut bytes).is_err()
            || u64::try_from(bytes.len()).ok() != Some(length)
        {
            return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
        }
        bytes
    };
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        )
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn parse_single_range(value: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    if size == 0 || value.contains(',') {
        return Err(());
    }
    let spec = value.trim().strip_prefix("bytes=").ok_or(())?;
    let (start, end) = spec.split_once('-').ok_or(())?;
    let cap_end = |start: u64, requested_end: u64| {
        requested_end
            .min(size - 1)
            .min(start.saturating_add(MAX_RANGE_RESPONSE_BYTES - 1))
    };
    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let length = suffix.min(size).min(MAX_RANGE_RESPONSE_BYTES);
        return Ok(Some((size - length, size - 1)));
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= size {
        return Err(());
    }
    if end.is_empty() {
        return Ok(Some((start, cap_end(start, size - 1))));
    }
    let requested_end = end.parse::<u64>().map_err(|_| ())?;
    if requested_end < start {
        return Err(());
    }
    Ok(Some((start, cap_end(start, requested_end))))
}

fn response(status: StatusCode, mime_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        && (value.starts_with("assets/images/") || value.starts_with("assets/staging/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_bounds_single_byte_ranges() {
        assert_eq!(parse_single_range(None, 10).unwrap(), None);
        assert_eq!(
            parse_single_range(Some("bytes=0-4"), 10).unwrap(),
            Some((0, 4))
        );
        assert_eq!(
            parse_single_range(Some("bytes=5-"), 10).unwrap(),
            Some((5, 9))
        );
        assert_eq!(
            parse_single_range(Some("bytes=-3"), 10).unwrap(),
            Some((7, 9))
        );
        let huge = MAX_RANGE_RESPONSE_BYTES * 4;
        assert_eq!(
            parse_single_range(Some("bytes=0-"), huge).unwrap(),
            Some((0, MAX_RANGE_RESPONSE_BYTES - 1)),
        );
    }

    #[test]
    fn rejects_multiple_invalid_and_unsatisfiable_ranges() {
        for value in [
            "items=0-1",
            "bytes=",
            "bytes=4-3",
            "bytes=100-",
            "bytes=0-1,3-4",
            "bytes=-0",
        ] {
            assert!(parse_single_range(Some(value), 10).is_err(), "{value}");
        }
    }

    #[test]
    fn keeps_protocol_paths_inside_managed_asset_roots() {
        assert!(safe_relative_path("assets/images/a.mp3"));
        assert!(safe_relative_path("assets/staging/d-a.mp4.stage"));
        assert!(!safe_relative_path("assets/../library.sqlite3"));
        assert!(!safe_relative_path("documents/source.pdf"));
    }
}
