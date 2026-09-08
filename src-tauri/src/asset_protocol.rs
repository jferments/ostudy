//! Asset-ID-only WebView protocol with bounded single-range media serving.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

use rusqlite::OptionalExtension;
use tauri::{
    http::{header, Request, Response, StatusCode},
    Manager, Runtime, UriSchemeContext,
};

use crate::{database::Database, paths::AppPaths, util::validate_entity_id};

pub fn respond<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if context.webview_label() != "main" {
        return response(StatusCode::FORBIDDEN, "text/plain", b"forbidden".to_vec());
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
    if !metadata.is_file() || metadata.len() != expected_size {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }

    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    match range_header {
        Some(value) => match parse_single_range(value, expected_size) {
            Ok(range) => ranged_response(&path, &mime_type, expected_size, range),
            Err(()) => range_not_satisfiable(expected_size),
        },
        None => full_response(&path, &mime_type, expected_size),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_single_range(value: &str, size: u64) -> Result<ByteRange, ()> {
    if size == 0 {
        return Err(());
    }
    let value = value.trim();
    let raw = value.strip_prefix("bytes=").ok_or(())?;
    if raw.contains(',') {
        return Err(());
    }
    let (start, end) = raw.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let length = suffix.min(size);
        return Ok(ByteRange {
            start: size - length,
            end: size - 1,
        });
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= size {
        return Err(());
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if end < start {
        return Err(());
    }
    Ok(ByteRange { start, end })
}

fn full_response(path: &Path, mime_type: &str, size: u64) -> Response<Vec<u8>> {
    let Ok(bytes) = fs::read(path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"asset missing".to_vec(),
        );
    };
    response_builder(StatusCode::OK, mime_type, bytes)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, size.to_string())
        .body()
}

fn ranged_response(path: &Path, mime_type: &str, size: u64, range: ByteRange) -> Response<Vec<u8>> {
    let length = range.end.saturating_sub(range.start).saturating_add(1);
    let Ok(length_usize) = usize::try_from(length) else {
        return range_not_satisfiable(size);
    };
    let Ok(mut file) = File::open(path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"asset missing".to_vec(),
        );
    };
    if file.seek(SeekFrom::Start(range.start)).is_err() {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }
    let mut bytes = vec![0_u8; length_usize];
    if file.read_exact(&mut bytes).is_err() {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }
    response_builder(StatusCode::PARTIAL_CONTENT, mime_type, bytes)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", range.start, range.end, size),
        )
        .header(header::CONTENT_LENGTH, length.to_string())
        .body()
}

fn range_not_satisfiable(size: u64) -> Response<Vec<u8>> {
    response_builder(StatusCode::RANGE_NOT_SATISFIABLE, "text/plain", Vec::new())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, format!("bytes */{size}"))
        .body()
}

struct ResponseBuilder {
    builder: tauri::http::response::Builder,
    body: Vec<u8>,
}

impl ResponseBuilder {
    fn header(mut self, name: header::HeaderName, value: impl ToString) -> Self {
        self.builder = self.builder.header(name, value.to_string());
        self
    }

    fn body(self) -> Response<Vec<u8>> {
        self.builder
            .body(self.body)
            .unwrap_or_else(|_| Response::new(Vec::new()))
    }
}

fn response_builder(status: StatusCode, mime_type: &str, body: Vec<u8>) -> ResponseBuilder {
    let builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    ResponseBuilder { builder, body }
}

fn response(status: StatusCode, mime_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    response_builder(status, mime_type, body).body()
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
    fn parses_single_open_closed_and_suffix_ranges() {
        assert_eq!(
            parse_single_range("bytes=2-5", 10),
            Ok(ByteRange { start: 2, end: 5 })
        );
        assert_eq!(
            parse_single_range("bytes=7-", 10),
            Ok(ByteRange { start: 7, end: 9 })
        );
        assert_eq!(
            parse_single_range("bytes=-3", 10),
            Ok(ByteRange { start: 7, end: 9 })
        );
        assert_eq!(
            parse_single_range("bytes=-99", 10),
            Ok(ByteRange { start: 0, end: 9 })
        );
    }

    #[test]
    fn rejects_multiple_malformed_or_unsatisfiable_ranges() {
        for value in [
            "items=0-1",
            "bytes=",
            "bytes=5-4",
            "bytes=10-",
            "bytes=0-1,4-5",
            "bytes=-0",
        ] {
            assert!(parse_single_range(value, 10).is_err(), "{value}");
        }
        assert!(parse_single_range("bytes=0-0", 0).is_err());
    }
}
