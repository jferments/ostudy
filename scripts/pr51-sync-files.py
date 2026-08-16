#!/usr/bin/env python3
"""One-shot wiring for path-free synchronization file transfer commands."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(content: str, old: str, new: str, label: str) -> str:
    if new in content:
        return content
    if content.count(old) != 1:
        raise RuntimeError(f"Expected one {label} target, found {content.count(old)}")
    return content.replace(old, new, 1)


def patch_sync_module() -> None:
    path = "src-tauri/src/device_sync.rs"
    content = read(path)
    content = replace_once(
        content,
        "use tauri::State;\n",
        "use tauri::{AppHandle, State};\n",
        "Tauri imports",
    )
    import_anchor = "    paths::AppPaths,\n"
    platform_import = (
        "    platform::{\n"
        "        ExternalExportSource, ExternalFileRequest, ExternalImportTarget, FileFilter,\n"
        "        PlatformRuntime, ShareSource,\n"
        "    },\n"
    )
    if platform_import.strip() not in content:
        if content.count(import_anchor) != 1:
            raise RuntimeError("Could not locate AppPaths import")
        content = content.replace(import_anchor, import_anchor + platform_import, 1)

    structs_anchor = (
        "pub struct CommitDeviceSyncResultDto {\n"
        "    pub remote_device_id: String,\n"
        "    pub upserted_rows: usize,\n"
        "    pub deleted_rows: usize,\n"
        "    pub written_assets: usize,\n"
        "}\n"
    )
    structs = r'''

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcquireDeviceSyncFileInput {
    pub operation_id: String,
    pub peer_device_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportDeviceSyncFileInput {
    pub operation_id: String,
    pub peer_device_id: String,
    pub device_name: String,
    pub include_settings: bool,
    pub include_ai_profiles: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncFileTransferDto {
    pub cancelled: bool,
    pub display_name: Option<String>,
    pub destination_kind: Option<String>,
    pub snapshot_id: Option<String>,
    pub row_count: usize,
    pub asset_count: usize,
}
'''
    if "pub struct AcquireDeviceSyncFileInput" not in content:
        if content.count(structs_anchor) != 1:
            raise RuntimeError("Could not locate sync result DTO anchor")
        content = content.replace(structs_anchor, structs_anchor + structs, 1)

    commands_anchor = "pub(crate) fn capture_sync_snapshot_for_transport(\n"
    commands = r'''
#[tauri::command]
pub async fn acquire_device_sync_file(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: AcquireDeviceSyncFileInput,
) -> Result<Option<DeviceSyncPreviewDto>, CommandError> {
    validate_entity_id(&input.operation_id, "device sync file acquisition")?;
    validate_entity_id(&input.peer_device_id, "paired device")?;
    let destination = sync_root(paths.inner())
        .join("incoming")
        .join(format!("{}.json", input.operation_id));
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let _ = fs::remove_file(&destination);
    let acquired = runtime
        .external_files()
        .acquire_import(
            app,
            ExternalFileRequest {
                title: "Choose an ostudy synchronization snapshot".to_string(),
                suggested_filename: None,
                filters: vec![FileFilter {
                    name: "ostudy synchronization snapshot".to_string(),
                    extensions: vec!["json".to_string()],
                }],
            },
            ExternalImportTarget {
                path: destination.clone(),
                maximum_bytes: MAX_SYNC_BYTES,
            },
        )
        .await?;
    let Some(acquired) = acquired else {
        return Ok(None);
    };
    let metadata = fs::metadata(&destination).map_err(CommandError::io)?;
    if metadata.len() != acquired.copied_bytes {
        let _ = fs::remove_file(destination);
        return Err(CommandError::new(
            "sync_transfer_mismatch",
            "The selected synchronization snapshot changed while Android copied it.",
        ));
    }
    let remote = read_snapshot(&destination)?;
    let preview = preview_remote_snapshot(
        database.inner(),
        paths.inner(),
        &input.operation_id,
        &input.peer_device_id,
        remote,
    );
    let _ = fs::remove_file(destination);
    preview.map(Some)
}

#[tauri::command]
pub async fn export_device_sync_file(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: ExportDeviceSyncFileInput,
) -> Result<DeviceSyncFileTransferDto, CommandError> {
    validate_entity_id(&input.operation_id, "device sync export operation")?;
    validate_entity_id(&input.peer_device_id, "paired device")?;
    let snapshot = capture_sync_snapshot_for_transport(
        database.inner(),
        paths.inner(),
        input.device_name,
        input.include_settings,
        input.include_ai_profiles,
    )?;
    let destination = write_transport_snapshot(paths.inner(), &snapshot, &input.operation_id)?;
    let filename = format!(
        "ostudy-sync-{}.json",
        portable_filename(&snapshot.source_device_name)
    );
    let row_count = snapshot.tables.iter().map(|table| table.rows.len()).sum();
    let published = runtime
        .external_files()
        .publish_export(
            app,
            ExternalFileRequest {
                title: "Save ostudy synchronization snapshot".to_string(),
                suggested_filename: Some(filename.clone()),
                filters: vec![FileFilter {
                    name: "ostudy synchronization snapshot".to_string(),
                    extensions: vec!["json".to_string()],
                }],
            },
            ExternalExportSource {
                path: destination.clone(),
                display_name: filename,
                media_type: "application/vnd.ostudy.sync+json".to_string(),
                maximum_bytes: MAX_SYNC_BYTES,
            },
        )
        .await?;
    let _ = fs::remove_file(destination);
    Ok(match published {
        Some(published) => DeviceSyncFileTransferDto {
            cancelled: false,
            display_name: Some(published.display_name),
            destination_kind: Some(published.destination_kind),
            snapshot_id: Some(snapshot.snapshot_id),
            row_count,
            asset_count: snapshot.assets.len(),
        },
        None => DeviceSyncFileTransferDto {
            cancelled: true,
            display_name: None,
            destination_kind: None,
            snapshot_id: None,
            row_count: 0,
            asset_count: 0,
        },
    })
}

#[tauri::command]
pub async fn share_device_sync_file(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: ExportDeviceSyncFileInput,
) -> Result<DeviceSyncFileTransferDto, CommandError> {
    validate_entity_id(&input.operation_id, "device sync share operation")?;
    validate_entity_id(&input.peer_device_id, "paired device")?;
    let snapshot = capture_sync_snapshot_for_transport(
        database.inner(),
        paths.inner(),
        input.device_name,
        input.include_settings,
        input.include_ai_profiles,
    )?;
    let destination = write_transport_snapshot(paths.inner(), &snapshot, &input.operation_id)?;
    let filename = format!(
        "ostudy-sync-{}.json",
        portable_filename(&snapshot.source_device_name)
    );
    let row_count = snapshot.tables.iter().map(|table| table.rows.len()).sum();
    let shared = runtime
        .system_share()
        .share_file(
            app,
            ShareSource {
                title: "Share ostudy synchronization snapshot".to_string(),
                path: destination.clone(),
                display_name: filename,
                media_type: "application/vnd.ostudy.sync+json".to_string(),
            },
        )
        .await?;
    let _ = fs::remove_file(destination);
    Ok(DeviceSyncFileTransferDto {
        cancelled: !shared.launched,
        display_name: None,
        destination_kind: Some(shared.destination_kind),
        snapshot_id: shared.launched.then_some(snapshot.snapshot_id),
        row_count: if shared.launched { row_count } else { 0 },
        asset_count: if shared.launched {
            snapshot.assets.len()
        } else {
            0
        },
    })
}

fn preview_remote_snapshot(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    peer_device_id: &str,
    remote: DeviceSyncSnapshot,
) -> Result<DeviceSyncPreviewDto, CommandError> {
    if remote.source_device_id != peer_device_id {
        return Err(CommandError::new(
            "sync_peer_mismatch",
            "This synchronization snapshot belongs to a different paired device.",
        ));
    }
    let identity = get_or_create_identity(database, paths, default_device_name())?;
    let local = database.with_connection(|connection| {
        capture_snapshot(connection, paths, &identity, true, true, false)
    })?;
    let base = read_base_snapshot(paths, peer_device_id)?
        .unwrap_or_else(|| empty_base_snapshot(&identity, &remote));
    let plan = merge_snapshots(&base, &local, &remote)?;
    let preview = StoredSyncPreview {
        preview_id: operation_id.to_string(),
        peer_device_id: peer_device_id.to_string(),
        remote_device_id: remote.source_device_id.clone(),
        remote_device_name: remote.source_device_name.clone(),
        local_fingerprint: snapshot_content_fingerprint(&local)?,
        local_snapshot: local,
        remote_snapshot: remote,
        merged_snapshot: plan.merged,
        conflicts: plan.conflicts.clone(),
        upsert_count: plan.upsert_count,
        delete_count: plan.delete_count,
        asset_write_count: plan.asset_write_count,
    };
    write_json_atomic(&preview_path(paths, operation_id), &preview)?;
    Ok(DeviceSyncPreviewDto {
        preview_id: preview.preview_id,
        remote_device_id: preview.remote_device_id,
        remote_device_name: preview.remote_device_name,
        upsert_count: preview.upsert_count,
        delete_count: preview.delete_count,
        asset_write_count: preview.asset_write_count,
        conflicts: preview.conflicts,
    })
}

'''
    if "pub async fn acquire_device_sync_file" not in content:
        if content.count(commands_anchor) != 1:
            raise RuntimeError("Could not locate sync transport helper anchor")
        content = content.replace(commands_anchor, commands + commands_anchor, 1)
    write(path, content)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    content = read(path)
    anchor = "            device_sync::commit_device_sync,\n"
    insertion = (
        "            device_sync::acquire_device_sync_file,\n"
        "            device_sync::export_device_sync_file,\n"
        "            device_sync::share_device_sync_file,\n"
    )
    if "device_sync::acquire_device_sync_file" not in content:
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate device sync command registry anchor")
        content = content.replace(anchor, anchor + insertion, 1)
    write(path, content)


def remove_one_shot_files() -> None:
    for relative in ("scripts/pr51-sync-files.py", ".github/workflows/pr51-sync-files.yml"):
        path = ROOT / relative
        if path.exists():
            path.unlink()


def main() -> None:
    patch_sync_module()
    patch_lib()
    remove_one_shot_files()


if __name__ == "__main__":
    main()
