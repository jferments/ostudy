#!/usr/bin/env python3
"""Consolidate and validate PR #51's Android/sync integration.

The GitHub contents API replaces whole files. This one-shot script applies small,
asserted, idempotent edits to large existing registry/configuration files, then
removes itself before the runner commits the integrated result.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def path(relative: str) -> Path:
    return ROOT / relative


def read(relative: str) -> str:
    return path(relative).read_text(encoding="utf-8")


def write(relative: str, content: str) -> None:
    path(relative).write_text(content, encoding="utf-8")


def insert_after(content: str, anchor: str, insertion: str, label: str) -> str:
    if insertion.strip() in content:
        return content
    count = content.count(anchor)
    if count != 1:
        raise RuntimeError(f"Expected one {label} anchor, found {count}")
    return content.replace(anchor, anchor + insertion, 1)


def replace_once(content: str, old: str, new: str, label: str) -> str:
    if new in content:
        return content
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"Expected one {label} target, found {count}")
    return content.replace(old, new, 1)


def patch_lib() -> None:
    relative = "src-tauri/src/lib.rs"
    content = read(relative)
    declarations = [
        ("mod decks;\n", "mod device_sync;\n"),
        ("mod device_sync;\n", "mod device_sync_transport;\n"),
        ("mod device_sync_transport;\n", "mod device_sync_qr;\n"),
    ]
    for anchor, declaration in declarations:
        if declaration.strip() not in content:
            if content.count(anchor) != 1:
                raise RuntimeError(f"Could not locate module anchor {anchor!r}")
            content = content.replace(anchor, anchor + declaration, 1)
    if "mod document_background;" not in content:
        for anchor in (
            "mod document_backup;\n",
            "mod document_checkpoints;\n",
            "mod document_domain;\n",
        ):
            if content.count(anchor) == 1:
                content = content.replace(anchor, anchor + "mod document_background;\n", 1)
                break
        else:
            raise RuntimeError("Could not locate document module declarations")

    commands = [
        "device_sync::get_device_sync_identity",
        "device_sync::prepare_device_sync_snapshot",
        "device_sync::preview_device_sync",
        "device_sync::commit_device_sync",
        "device_sync::acquire_device_sync_file",
        "device_sync::export_device_sync_file",
        "device_sync::share_device_sync_file",
        "device_sync_transport::start_device_sync_session",
        "device_sync_transport::receive_device_sync_invitation",
        "device_sync_transport::get_device_sync_session_status",
        "device_sync_transport::cancel_device_sync_session",
        "device_sync_transport::list_trusted_sync_devices",
        "device_sync_transport::remove_trusted_sync_device",
        "device_sync_qr::scan_device_sync_qr",
    ]
    missing = [command for command in commands if f"{command}," not in content]
    if missing:
        anchor = "tauri::generate_handler![\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate the unique Tauri command registry")
        insertion = "".join(f"            {command},\n" for command in missing)
        content = content.replace(anchor, anchor + insertion, 1)
    write(relative, content)


def patch_document_execution() -> None:
    relative = "src-tauri/src/document_operations.rs"
    content = read(relative)
    if "document_background::DocumentExecutionLease" not in content:
        anchor = "    database::Database,\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate document operation imports")
        content = content.replace(
            anchor,
            anchor + "    document_background::DocumentExecutionLease,\n",
            1,
        )
    signature = (
        "pub async fn run_pdf_flashcard_generation(\n"
        "    database: State<'_, Database>,\n"
    )
    if signature in content:
        content = content.replace(
            signature,
            "pub async fn run_pdf_flashcard_generation(\n"
            "    app: AppHandle,\n"
            "    database: State<'_, Database>,\n",
            1,
        )
    if "DocumentExecutionLease::begin" not in content:
        anchor = (
            "    let document_id = input.document_id.clone();\n"
            "    let database_after = database.clone();\n"
        )
        replacement = (
            "    let document_id = input.document_id.clone();\n"
            "    let execution_lease = DocumentExecutionLease::begin(\n"
            "        app,\n"
            "        format!(\"generation-{document_id}\"),\n"
            "        \"Generating study material\",\n"
            "        \"Processing the selected PDF\",\n"
            "    )\n"
            "    .map_err(|detail| {\n"
            "        CommandError::new(\n"
            "            \"document_background_unavailable\",\n"
            "            format!(\"Android could not start foreground document processing: {detail}\"),\n"
            "        )\n"
            "    })?;\n"
            "    execution_lease.update(\"Preparing document extraction\", Some(0));\n"
            "    let database_after = database.clone();\n"
        )
        content = replace_once(content, anchor, replacement, "document execution lease")
    write(relative, content)


def patch_android_capability() -> None:
    relative = "src-tauri/src/platform/android.rs"
    content = read(relative)
    old = (
        "                background_execution: ServiceCapability::unavailable(\n"
        "                    \"android_background_execution_not_implemented\",\n"
        "                ),\n"
    )
    new = "                background_execution: ServiceCapability::available(),\n"
    if new not in content:
        content = replace_once(content, old, new, "Android background capability")
    write(relative, content)


def patch_package_scripts() -> None:
    package_path = path("package.json")
    package = json.loads(package_path.read_text(encoding="utf-8"))
    package.setdefault("scripts", {}).update(
        {
            "android:build:debug": "node scripts/android-package.mjs build-debug",
            "android:install:debug": "node scripts/android-package.mjs install-debug",
            "android:build:release": "node scripts/android-package.mjs build-release",
            "android:bundle:release": "node scripts/android-package.mjs bundle-release",
            "android:artifacts": "node scripts/android-package.mjs artifacts",
        }
    )
    package_path.write_text(json.dumps(package, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def patch_cargo() -> None:
    relative = "src-tauri/Cargo.toml"
    content = read(relative)
    anchor = "[dependencies]\n"
    if content.count(anchor) != 1:
        raise RuntimeError("Could not locate Cargo dependencies")
    additions = []
    if not re.search(r"^qrcode\s*=", content, re.MULTILINE):
        additions.append('qrcode = { version = "=0.14.1", default-features = false }\n')
    if not re.search(r"^ring\s*=", content, re.MULTILINE):
        additions.append('ring = "=0.17.14"\n')
    if additions:
        content = content.replace(anchor, anchor + "".join(additions), 1)
    write(relative, content)


def patch_qr_plugin_bridge() -> None:
    models_relative = "plugins/ostudy-platform/src/models.rs"
    models = read(models_relative)
    if "pub struct ScanQrCodeResult" not in models:
        models = models.rstrip() + (
            "\n\n#[derive(Clone, Debug, Deserialize)]\n"
            "#[serde(rename_all = \"camelCase\")]\n"
            "pub struct ScanQrCodeResult {\n"
            "    pub cancelled: bool,\n"
            "    pub value: Option<String>,\n"
            "}\n"
        )
    write(models_relative, models)

    mobile_relative = "plugins/ostudy-platform/src/mobile.rs"
    mobile = read(mobile_relative)
    registration_anchor = (
        "    let documents = api.register_android_plugin(PLUGIN_IDENTIFIER, \"OstudyDocumentPlugin\")?;\n"
    )
    if "OstudyQrPlugin" not in mobile:
        mobile = insert_after(
            mobile,
            registration_anchor,
            "    #[cfg(target_os = \"android\")]\n"
            "    let qr = api.register_android_plugin(PLUGIN_IDENTIFIER, \"OstudyQrPlugin\")?;\n",
            "QR plugin registration",
        )
    if "        qr,\n" not in mobile:
        mobile = insert_after(mobile, "        documents,\n", "        qr,\n", "QR constructor")
    if "    qr: PluginHandle<R>,\n" not in mobile:
        mobile = insert_after(
            mobile,
            "    documents: PluginHandle<R>,\n",
            "    qr: PluginHandle<R>,\n",
            "QR handle field",
        )
    if "pub fn scan_qr_code" not in mobile:
        method = (
            "\n    pub fn scan_qr_code(&self) -> Result<ScanQrCodeResult> {\n"
            "        self.qr\n"
            "            .run_mobile_plugin(\"scanQrCode\", ())\n"
            "            .map_err(Into::into)\n"
            "    }\n"
        )
        close = mobile.rfind("\n}")
        if close < 0:
            raise RuntimeError("Could not locate mobile plugin implementation end")
        mobile = mobile[:close] + method + mobile[close:]
    write(mobile_relative, mobile)


def patch_android_native_sources() -> None:
    qr_relative = "plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"
    qr = read(qr_relative)
    if "import android.net.Uri\n" not in qr:
        qr = qr.replace("import android.content.Intent\n", "import android.content.Intent\nimport android.net.Uri\n", 1)
    qr = qr.replace("capture.toUri()", "Uri.fromFile(capture)")
    qr = qr.replace(".mapNotNull(Barcode::getRawValue)", ".mapNotNull { barcode -> barcode.rawValue }")
    write(qr_relative, qr)

    for relative in (
        "plugins/ostudy-platform/android/src/main/java/OstudyDocumentPlugin.kt",
        "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidOcrSmokeTest.kt",
    ):
        content = read(relative)
        content = content.replace(
            'Class.forName("cz.adaptech.tesseract4android.TessBaseAPI")',
            'Class.forName("com.googlecode.tesseract.android.TessBaseAPI")',
        )
        write(relative, content)


def patch_qr_settings_ui() -> None:
    relative = "src/screens/settings/DeviceSyncSettings.tsx"
    content = read(relative)
    scanner_import = 'import { AndroidSyncQrScanner } from "./AndroidSyncQrScanner";\n'
    if scanner_import.strip() not in content:
        content = insert_after(
            content,
            'import { SyncQrCode } from "../../components/SyncQrCode";\n',
            scanner_import,
            "QR settings import",
        )
    if "<AndroidSyncQrScanner" not in content:
        button = (
            "          <button\n"
            "            type=\"button\"\n"
            "            className=\"primary-button\"\n"
            "            disabled={busy || !receivedInvitation.trim() || !deviceName.trim()}\n"
            "            onClick={() => void receiveQrInvitation()}\n"
            "          >\n"
            "            Receive encrypted sync\n"
            "          </button>\n"
        )
        scanner = (
            "          <AndroidSyncQrScanner\n"
            "            disabled={busy}\n"
            "            onScanned={setReceivedInvitation}\n"
            "            onError={setErrorMessage}\n"
            "          />\n"
        )
        content = replace_once(content, button, scanner + button, "QR receive button")
    write(relative, content)

    css_relative = "src/screens/settings/DeviceSyncSettings.module.css"
    css = read(css_relative)
    css = css.replace(
        "  grid-template-columns: minmax(0, 1fr) auto;\n",
        "  grid-template-columns: minmax(0, 1fr) repeat(2, auto);\n",
        1,
    )
    write(css_relative, css)


def patch_sync_test_mocks() -> None:
    relative = "src/screens/settings/DeviceSyncSettings.test.tsx"
    content = read(relative)
    if "listTrustedSyncDevices: vi.fn()," not in content:
        content = insert_after(
            content,
            "    commitDeviceSync: vi.fn(),\n",
            "    listTrustedSyncDevices: vi.fn(),\n",
            "trusted peer mock",
        )
    if "vi.mocked(listTrustedSyncDevices).mockResolvedValue([]);" not in content:
        content = content.replace(
            "  beforeEach(() => {\n    vi.clearAllMocks();\n",
            "  beforeEach(() => {\n    vi.clearAllMocks();\n"
            "    vi.mocked(listTrustedSyncDevices).mockResolvedValue([]);\n",
            1,
        )
    if "listTrustedSyncDevices," not in content.split('from "../../ipc/deviceSync";')[0]:
        content = content.replace(
            "  getDeviceSyncIdentity,\n",
            "  getDeviceSyncIdentity,\n  listTrustedSyncDevices,\n",
            1,
        )
    write(relative, content)


def patch_sync_file_transfer() -> None:
    relative = "src-tauri/src/device_sync.rs"
    content = read(relative)
    if "use tauri::{AppHandle, State};" not in content:
        content = content.replace("use tauri::State;", "use tauri::{AppHandle, State};", 1)
    platform_import = (
        "    platform::{\n"
        "        ExternalExportSource, ExternalFileRequest, ExternalImportTarget, FileFilter,\n"
        "        PlatformRuntime, ShareSource,\n"
        "    },\n"
    )
    if "ExternalExportSource" not in content:
        content = insert_after(content, "    paths::AppPaths,\n", platform_import, "sync platform imports")

    if "pub struct AcquireDeviceSyncFileInput" not in content:
        anchor = (
            "pub struct CommitDeviceSyncResultDto {\n"
            "    pub remote_device_id: String,\n"
            "    pub upserted_rows: usize,\n"
            "    pub deleted_rows: usize,\n"
            "    pub written_assets: usize,\n"
            "}\n"
        )
        addition = r'''

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
        content = insert_after(content, anchor, addition, "sync file DTOs")

    if "pub async fn acquire_device_sync_file" not in content:
        anchor = "pub(crate) fn capture_sync_snapshot_for_transport(\n"
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
            "The selected synchronization snapshot changed while it was copied.",
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
                path: destination,
                display_name: filename,
                media_type: "application/vnd.ostudy.sync+json".to_string(),
            },
        )
        .await?;
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

pub(crate) fn preview_remote_snapshot(
    database: &Database,
    paths: &AppPaths,
    operation_id: &str,
    peer_device_id: &str,
    remote: DeviceSyncSnapshot,
) -> Result<DeviceSyncPreviewDto, CommandError> {
    validate_entity_id(operation_id, "device sync preview")?;
    validate_entity_id(peer_device_id, "paired device")?;
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
        content = insert_after(content, anchor, commands, "path-free sync commands")

    if "pub(crate) fn sync_identity(" not in content:
        anchor = "pub(crate) fn capture_sync_snapshot_for_transport(\n"
        helpers = r'''
pub(crate) fn sync_identity(
    database: &Database,
    paths: &AppPaths,
    device_name: String,
) -> Result<SyncIdentityDto, CommandError> {
    get_or_create_identity(database, paths, validate_device_name(device_name)?)
}

pub(crate) fn acknowledge_transport_base(
    paths: &AppPaths,
    peer_device_id: &str,
    snapshot: &DeviceSyncSnapshot,
) -> Result<(), CommandError> {
    validate_entity_id(peer_device_id, "paired device")?;
    validate_snapshot(snapshot, true)?;
    let mut base = snapshot.clone();
    for asset in &mut base.assets {
        asset.data_base64 = None;
    }
    write_json_atomic(&base_path(paths, peer_device_id), &base)
}

pub(crate) fn remove_transport_base(
    paths: &AppPaths,
    peer_device_id: &str,
) -> Result<(), CommandError> {
    validate_entity_id(peer_device_id, "paired device")?;
    match fs::remove_file(base_path(paths, peer_device_id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

'''
        content = insert_after(content, anchor, helpers, "transport sync helpers")

    # Share files must live in the already-audited prepared-export directory so
    # Android FileProvider can grant them to the Sharesheet.
    transport_pattern = re.compile(
        r"pub\(crate\) fn write_transport_snapshot\(.*?\n}\n",
        re.DOTALL,
    )
    replacement = r'''pub(crate) fn write_transport_snapshot(
    paths: &AppPaths,
    snapshot: &DeviceSyncSnapshot,
    operation_id: &str,
) -> Result<PathBuf, CommandError> {
    validate_entity_id(operation_id, "device sync transport operation")?;
    validate_snapshot(snapshot, true)?;
    let relative = AppPaths::prepared_export_relative_path(operation_id, "json");
    let path = paths.resolve_relative(&relative);
    write_json_atomic(&path, snapshot)?;
    Ok(path)
}
'''
    if transport_pattern.search(content):
        content = transport_pattern.sub(replacement, content, count=1)

    # Make asset rollback aware of which paths were actually promoted.
    content = content.replace(
        "struct AssetWriteGuard {\n    temporary: PathBuf,\n    destination: PathBuf,\n    backup: Option<PathBuf>,\n}",
        "struct AssetWriteGuard {\n    temporary: PathBuf,\n    destination: PathBuf,\n    backup: Option<PathBuf>,\n    promoted: bool,\n}",
    )
    content = content.replace(
        "            backup: None,\n        });",
        "            backup: None,\n            promoted: false,\n        });",
    )
    content = content.replace(
        "        if let Err(error) = fs::rename(&guard.temporary, &guard.destination) {\n"
        "            if let Some(backup) = &guard.backup {\n"
        "                let _ = fs::rename(backup, &guard.destination);\n"
        "            }\n"
        "            return Err(CommandError::io(error));\n"
        "        }\n"
        "        sync_parent(&guard.destination)?;",
        "        if let Err(error) = fs::rename(&guard.temporary, &guard.destination) {\n"
        "            if let Some(backup) = guard.backup.take() {\n"
        "                let _ = fs::rename(backup, &guard.destination);\n"
        "            }\n"
        "            return Err(CommandError::io(error));\n"
        "        }\n"
        "        guard.promoted = true;\n"
        "        sync_parent(&guard.destination)?;",
    )
    rollback_pattern = re.compile(
        r"fn rollback_asset_guards\(guards: &\[AssetWriteGuard\]\) \{.*?\n}\n",
        re.DOTALL,
    )
    rollback = r'''fn rollback_asset_guards(guards: &[AssetWriteGuard]) {
    for guard in guards.iter().rev() {
        let _ = fs::remove_file(&guard.temporary);
        if !guard.promoted {
            if let Some(backup) = &guard.backup {
                if !guard.destination.exists() {
                    let _ = fs::rename(backup, &guard.destination);
                }
            }
            continue;
        }
        let _ = fs::remove_file(&guard.destination);
        if let Some(backup) = &guard.backup {
            let _ = fs::rename(backup, &guard.destination);
        }
    }
}
'''
    if rollback_pattern.search(content):
        content = rollback_pattern.sub(rollback, content, count=1)

    # A failed base-snapshot bookkeeping write after the SQLite commit must not
    # report that the already-committed user data was rolled back.
    base_write = (
        "    write_json_atomic(&base_path(paths.inner(), &preview.peer_device_id), &base)?;\n"
        "    let _ = fs::remove_file(path);\n"
    )
    base_write_replacement = (
        "    if let Err(error) = write_json_atomic(\n"
        "        &base_path(paths.inner(), &preview.peer_device_id),\n"
        "        &base,\n"
        "    ) {\n"
        "        crate::logging::info(\n"
        "            \"device_sync_base_write_failed\",\n"
        "            format!(\"peer={} detail={}\", preview.peer_device_id, error.message),\n"
        "        );\n"
        "    }\n"
        "    let _ = fs::remove_file(path);\n"
    )
    if base_write in content:
        content = content.replace(base_write, base_write_replacement, 1)

    # Reject snapshots whose opaque row key disagrees with the primary-key
    # values actually carried by the row.
    validation_anchor = (
        "        let mut seen_rows = BTreeSet::new();\n"
        "        for row in &table.rows {\n"
        "            if !seen_rows.insert(&row.key) {\n"
    )
    validation_replacement = (
        "        if table.primary_key.is_empty()\n"
        "            || table.primary_key.iter().any(|column| !table.columns.contains(column))\n"
        "        {\n"
        "            return Err(CommandError::new(\n"
        "                \"sync_snapshot_invalid\",\n"
        "                \"A synchronization table has an invalid primary-key schema.\",\n"
        "            ));\n"
        "        }\n"
        "        let mut seen_rows = BTreeSet::new();\n"
        "        for row in &table.rows {\n"
        "            if row.values.keys().any(|column| !table.columns.contains(column))\n"
        "                || row_key(&table.primary_key, &row.values)? != row.key\n"
        "            {\n"
        "                return Err(CommandError::new(\n"
        "                    \"sync_snapshot_invalid\",\n"
        "                    \"A synchronization row does not match its declared schema or identity.\",\n"
        "                ));\n"
        "            }\n"
        "            if !seen_rows.insert(&row.key) {\n"
    )
    if validation_anchor in content and validation_replacement not in content:
        content = content.replace(validation_anchor, validation_replacement, 1)
    write(relative, content)


def patch_transport() -> None:
    relative = "src-tauri/src/device_sync_transport.rs"
    content = read(relative)
    content = content.replace(
        "    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},\n",
        "    net::{IpAddr, Ipv4Addr, UdpSocket},\n",
    )
    content = content.replace(
        "        self, DeviceSyncPreviewDto, DeviceSyncSnapshot, SyncIdentityDto,\n",
        "        self, DeviceSyncPreviewDto, DeviceSyncSnapshot,\n",
    )
    content = content.replace(
        "    platform::{PlatformRuntime, SecretBytes, SecretLocator, SecretStoreAvailability},\n",
        "    platform::{PlatformRuntime, SecretLocator, SecretStoreAvailability},\n",
    )
    content = content.replace(
        ".map_err(|error| error.message)?;",
        ".map_err(|error| format!(\"{error:?}\"))?;",
    )

    # Pass and verify the exact snapshot identity in the encrypted ACK.
    content = content.replace(
        "        encrypted_bytes,\n    ));",
        "        encrypted_bytes,\n        invitation_snapshot_id(&invitation),\n    ));",
        1,
    )
    content = content.replace(
        "    encrypted_snapshot: Vec<u8>,\n) {",
        "    encrypted_snapshot: Vec<u8>,\n    expected_snapshot_id: String,\n) {",
        1,
    )
    content = content.replace(
        "                &encrypted_snapshot,\n            )",
        "                &encrypted_snapshot,\n                &expected_snapshot_id,\n            )",
        1,
    )
    content = content.replace(
        "    encrypted_snapshot: &[u8],\n) -> Result<ConnectionOutcome, String> {",
        "    encrypted_snapshot: &[u8],\n    expected_snapshot_id: &str,\n) -> Result<ConnectionOutcome, String> {",
        1,
    )
    ack_anchor = (
        "        validate_entity_id(&acknowledgement.receiver_device_id, \"paired receiver\")\n"
        "            .map_err(|error| format!(\"{error:?}\"))?;\n"
    )
    ack_check = (
        "        if acknowledgement.received_snapshot_id != expected_snapshot_id {\n"
        "            return Err(\"The receiver acknowledged a different snapshot identity.\".to_string());\n"
        "        }\n"
    )
    if ack_check.strip() not in content and ack_anchor in content:
        content = content.replace(ack_anchor, ack_anchor + ack_check, 1)

    # Zero the in-memory pairing key once it has been persisted to the secure
    # store and the sent snapshot has become the acknowledged merge base.
    finalized_anchor = (
        "            session.status.state = \"acknowledged\".to_string();\n"
        "            session.finalized = true;\n"
    )
    if "session.key.fill(0);" not in content and finalized_anchor in content:
        content = content.replace(
            finalized_anchor,
            "            session.status.state = \"acknowledged\".to_string();\n"
            "            session.key.fill(0);\n"
            "            session.finalized = true;\n",
            1,
        )

    if "fn invitation_snapshot_id(" not in content:
        anchor = "fn snapshot_aad(session_id: &str, source_device_id: &str) -> Vec<u8> {\n"
        helper = (
            "fn invitation_snapshot_id(invitation: &SyncInvitation) -> String {\n"
            "    ACTIVE_SNAPSHOT_IDS\n"
            "        .lock()\n"
            "        .ok()\n"
            "        .and_then(|ids| ids.get(&invitation.session_id).cloned())\n"
            "        .unwrap_or_default()\n"
            "}\n\n"
        )
        # Avoid a global side table: replace the temporary helper with direct
        # snapshot-id capture below instead of inserting it.
        del helper

    # The previous textual insertion needs the snapshot id captured before the
    # snapshot moves into the session map. Normalize that block directly.
    content = content.replace(
        "    ACTIVE_SESSIONS.lock().await.insert(\n",
        "    let expected_snapshot_id = snapshot.snapshot_id.clone();\n    ACTIVE_SESSIONS.lock().await.insert(\n",
        1,
    ) if "let expected_snapshot_id = snapshot.snapshot_id.clone();" not in content else content
    content = content.replace(
        "        encrypted_bytes,\n        invitation_snapshot_id(&invitation),\n    ));",
        "        encrypted_bytes,\n        expected_snapshot_id,\n    ));",
        1,
    )
    write(relative, content)


def remove_temporary_files() -> None:
    candidates = [
        "scripts/pr51-wire.py",
        ".github/workflows/pr51-wire.yml",
        "scripts/pr51-sync-files.py",
        ".github/workflows/pr51-sync-files.yml",
        "scripts/pr51-wire-transport.py",
        ".github/workflows/pr51-wire-transport.yml",
        "scripts/pr51-wire-qr.py",
        ".github/workflows/pr51-wire-qr.yml",
        "scripts/pr51-wire-qr-ui.py",
        ".github/workflows/pr51-wire-qr-ui.yml",
        "scripts/pr51-integrate.py",
        ".github/workflows/pr51-integrate.yml",
        "docs/development/.probe",
    ]
    for relative in candidates:
        target = path(relative)
        if target.exists():
            target.unlink()


def main() -> None:
    patch_lib()
    patch_document_execution()
    patch_android_capability()
    patch_package_scripts()
    patch_cargo()
    patch_qr_plugin_bridge()
    patch_android_native_sources()
    patch_qr_settings_ui()
    patch_sync_test_mocks()
    patch_sync_file_transfer()
    patch_transport()
    remove_temporary_files()


if __name__ == "__main__":
    main()
