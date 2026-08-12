//! Asset-ID-only WebView protocol.

use std::{
    fs,
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
    let path = paths.resolve_relative(&relative);
    let Ok(bytes) = fs::read(&path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"asset missing".to_vec(),
        );
    };
    if i64::try_from(bytes.len()).unwrap_or(i64::MAX) != expected_size {
        return response(StatusCode::GONE, "text/plain", b"asset changed".to_vec());
    }
    response(StatusCode::OK, &mime_type, bytes)
}

fn response(status: StatusCode, mime_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
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
