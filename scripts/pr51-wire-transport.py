#!/usr/bin/env python3
"""One-shot wiring for encrypted QR/LAN synchronization transport."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def insert_after(content: str, anchor: str, insertion: str, label: str) -> str:
    if insertion.strip() in content:
        return content
    if content.count(anchor) != 1:
        raise RuntimeError(f"Expected one {label} anchor, found {content.count(anchor)}")
    return content.replace(anchor, anchor + insertion, 1)


def patch_device_sync_helpers() -> None:
    path = "src-tauri/src/device_sync.rs"
    content = read(path)
    if "pub(crate) fn preview_remote_snapshot" not in content:
        if "fn preview_remote_snapshot(" in content:
            content = content.replace(
                "fn preview_remote_snapshot(\n",
                "pub(crate) fn preview_remote_snapshot(\n",
                1,
            )
        else:
            anchor = "pub(crate) fn capture_sync_snapshot_for_transport(\n"
            helper = r'''
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
            if content.count(anchor) != 1:
                raise RuntimeError("Could not locate sync transport helper anchor")
            content = content.replace(anchor, helper + anchor, 1)

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
    let path = base_path(paths, peer_device_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

'''
    if "pub(crate) fn sync_identity(" not in content:
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate capture_sync_snapshot_for_transport")
        content = content.replace(anchor, helpers + anchor, 1)
    write(path, content)


def patch_transport_cleanup() -> None:
    path = "src-tauri/src/device_sync_transport.rs"
    content = read(path)
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
    write(path, content)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    content = read(path)
    content = insert_after(
        content,
        "mod device_sync;\n",
        "mod device_sync_transport;\n",
        "device sync transport module",
    )
    command_anchor = "            device_sync::share_device_sync_file,\n"
    if command_anchor not in content:
        command_anchor = "            device_sync::commit_device_sync,\n"
    commands = (
        "            device_sync_transport::start_device_sync_session,\n"
        "            device_sync_transport::receive_device_sync_invitation,\n"
        "            device_sync_transport::get_device_sync_session_status,\n"
        "            device_sync_transport::cancel_device_sync_session,\n"
        "            device_sync_transport::list_trusted_sync_devices,\n"
        "            device_sync_transport::remove_trusted_sync_device,\n"
    )
    if "device_sync_transport::start_device_sync_session" not in content:
        if content.count(command_anchor) != 1:
            raise RuntimeError("Could not locate device-sync command registry anchor")
        content = content.replace(command_anchor, command_anchor + commands, 1)
    write(path, content)


def patch_cargo() -> None:
    path = "src-tauri/Cargo.toml"
    content = read(path)
    dependencies = {
        'qrcode = { version = "=0.14.1", default-features = false }\n': "qrcode",
        'ring = "=0.17.14"\n': "ring",
    }
    anchor = "[dependencies]\n"
    if content.count(anchor) != 1:
        raise RuntimeError("Could not locate Cargo dependency table")
    insertion = "".join(line for line, marker in dependencies.items() if f"{marker} =" not in content)
    if insertion:
        content = content.replace(anchor, anchor + insertion, 1)
    write(path, content)


def remove_one_shot_files() -> None:
    for relative in (
        "scripts/pr51-wire-transport.py",
        ".github/workflows/pr51-wire-transport.yml",
    ):
        path = ROOT / relative
        if path.exists():
            path.unlink()


def main() -> None:
    patch_device_sync_helpers()
    patch_transport_cleanup()
    patch_lib()
    patch_cargo()
    remove_one_shot_files()


if __name__ == "__main__":
    main()
