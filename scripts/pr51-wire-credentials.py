#!/usr/bin/env python3
"""One-shot wiring for explicit encrypted credential transfer and atomic schedules."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def write(relative: str, content: str) -> None:
    (ROOT / relative).write_text(content, encoding="utf-8")


def insert_after(content: str, anchor: str, insertion: str, label: str) -> str:
    if insertion.strip() in content:
        return content
    if content.count(anchor) != 1:
        raise RuntimeError(f"Expected one {label} anchor, found {content.count(anchor)}")
    return content.replace(anchor, anchor + insertion, 1)


def patch_lib() -> None:
    relative = "src-tauri/src/lib.rs"
    content = read(relative)
    if "mod device_sync_credentials;" not in content:
        anchor = "mod device_sync;\n"
        if anchor not in content:
            raise RuntimeError("device_sync module is not registered")
        content = content.replace(anchor, anchor + "mod device_sync_credentials;\n", 1)
    command = "device_sync::discard_device_sync_preview,\n"
    if command.strip() not in content:
        anchor = "            device_sync::commit_device_sync,\n"
        if anchor not in content:
            raise RuntimeError("commit_device_sync is not registered")
        content = content.replace(anchor, anchor + "            " + command, 1)
    write(relative, content)


def patch_transport() -> None:
    relative = "src-tauri/src/device_sync_transport.rs"
    content = read(relative)
    credential_import = (
        "    device_sync_credentials::{\n"
        "        collect_transferred_credentials, stage_received_credentials, TransferredCredential,\n"
        "    },\n"
    )
    if "collect_transferred_credentials" not in content:
        anchor = "    device_sync::{\n"
        position = content.find(anchor)
        if position < 0:
            raise RuntimeError("device_sync transport import group is missing")
        content = content[:position] + credential_import + content[position:]

    if "pub include_credentials: bool," not in content:
        anchor = "    pub include_ai_profiles: bool,\n"
        content = insert_after(content, anchor, "    pub include_credentials: bool,\n", "credential input")

    if "pub credential_count: usize," not in content:
        anchor = "    pub preview: DeviceSyncPreviewDto,\n"
        content = insert_after(content, anchor, "    pub credential_count: usize,\n", "received credential count")

    if "credentials: Vec<TransferredCredential>," not in content:
        pattern = re.compile(
            r"struct SyncTransportPayload \{\n\s+snapshot: DeviceSyncSnapshot,\n\}",
            re.MULTILINE,
        )
        replacement = (
            "struct SyncTransportPayload {\n"
            "    snapshot: DeviceSyncSnapshot,\n"
            "    #[serde(default)]\n"
            "    credentials: Vec<TransferredCredential>,\n"
            "}"
        )
        content, count = pattern.subn(replacement, content, count=1)
        if count != 1:
            raise RuntimeError("SyncTransportPayload layout is missing")

    old_payload = (
        "    let payload = SyncTransportPayload {\n"
        "        snapshot: snapshot.clone(),\n"
        "    };\n"
    )
    if old_payload in content:
        new_payload = (
            "    if input.include_credentials && !input.include_ai_profiles {\n"
            "        return Err(CommandError::new(\n"
            "            \"sync_credential_profiles_required\",\n"
            "            \"Enable AI connection profiles before transferring their credentials.\",\n"
            "        ));\n"
            "    }\n"
            "    let credentials = if input.include_credentials {\n"
            "        collect_transferred_credentials(database.inner(), runtime.inner())?\n"
            "    } else {\n"
            "        Vec::new()\n"
            "    };\n"
            "    let payload = SyncTransportPayload {\n"
            "        snapshot: snapshot.clone(),\n"
            "        credentials,\n"
            "    };\n"
        )
        content = content.replace(old_payload, new_payload, 1)

    # Stage credentials only after the encrypted acknowledgement succeeds, so
    # any earlier network or peer-registry failure drops them with the payload.
    old_return = "    Ok(ReceivedDeviceSyncDto { peer, preview })\n"
    if old_return in content:
        new_return = (
            "    let credential_count = stage_received_credentials(\n"
            "        &preview.preview_id,\n"
            "        payload.credentials,\n"
            "    )?;\n"
            "    Ok(ReceivedDeviceSyncDto {\n"
            "        peer,\n"
            "        preview,\n"
            "        credential_count,\n"
            "    })\n"
        )
        content = content.replace(old_return, new_return, 1)

    # Authenticated decryption overwrites ciphertext with plaintext; zero that
    # buffer after deserializing instead of leaving the JSON allocation intact.
    old_decrypt = (
        "    serde_json::from_slice(plaintext).map_err(|error| {\n"
        "        transport_serialization_error(\"The decrypted synchronization payload is invalid.\", error)\n"
        "    })\n"
    )
    if old_decrypt in content:
        new_decrypt = (
            "    let decoded = serde_json::from_slice(plaintext).map_err(|error| {\n"
            "        transport_serialization_error(\n"
            "            \"The decrypted synchronization payload is invalid.\",\n"
            "            error,\n"
            "        )\n"
            "    });\n"
            "    ciphertext.fill(0);\n"
            "    decoded\n"
        )
        content = content.replace(old_decrypt, new_decrypt, 1)
    write(relative, content)


def patch_sync_core() -> None:
    relative = "src-tauri/src/device_sync.rs"
    content = read(relative)
    if "ATOMIC_CONCURRENT_TABLES" not in content:
        anchor = 'const APPEND_ONLY_TABLES: &[&str] = &["review_events", "practice_sessions"];\n'
        content = insert_after(
            content,
            anchor,
            'const ATOMIC_CONCURRENT_TABLES: &[&str] = &["schedules"];\n',
            "atomic sync tables",
        )

    # Extend the commit result without changing existing JSON field names.
    if "pub imported_credential_count: usize," not in content:
        anchor = "    pub written_assets: usize,\n"
        addition = (
            "    pub imported_credential_count: usize,\n"
            "    pub credential_error_count: usize,\n"
        )
        content = insert_after(content, anchor, addition, "credential commit result")

    signature = (
        "pub fn commit_device_sync(\n"
        "    database: State<'_, Database>,\n"
    )
    if signature in content:
        content = content.replace(
            signature,
            "pub fn commit_device_sync(\n"
            "    database: State<'_, Database>,\n"
            "    runtime: State<'_, PlatformRuntime>,\n",
            1,
        )

    if "promote_received_credentials" not in content:
        anchor = "    commit_asset_guards(&asset_guards);\n"
        addition = (
            "    let credential_result =\n"
            "        crate::device_sync_credentials::promote_received_credentials(\n"
            "            &input.preview_id,\n"
            "            database.inner(),\n"
            "            runtime.inner(),\n"
            "        );\n"
        )
        content = insert_after(content, anchor, addition, "credential promotion")

    old_result = (
        "        written_assets: preview.asset_write_count,\n"
        "    })\n"
    )
    if old_result in content:
        new_result = (
            "        written_assets: preview.asset_write_count,\n"
            "        imported_credential_count: credential_result.imported,\n"
            "        credential_error_count: credential_result.failed,\n"
            "    })\n"
        )
        content = content.replace(old_result, new_result, 1)

    if "pub fn discard_device_sync_preview(" not in content:
        anchor = "pub(crate) fn sync_identity(\n"
        if anchor not in content:
            anchor = "pub(crate) fn capture_sync_snapshot_for_transport(\n"
        command = (
            "#[tauri::command]\n"
            "pub fn discard_device_sync_preview(\n"
            "    paths: State<'_, AppPaths>,\n"
            "    preview_id: String,\n"
            ") -> Result<(), CommandError> {\n"
            "    validate_entity_id(&preview_id, \"device sync preview\")?;\n"
            "    crate::device_sync_credentials::discard_received_credentials(&preview_id);\n"
            "    match fs::remove_file(preview_path(paths.inner(), &preview_id)) {\n"
            "        Ok(()) => Ok(()),\n"
            "        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),\n"
            "        Err(error) => Err(CommandError::io(error)),\n"
            "    }\n"
            "}\n\n"
        )
        if anchor not in content:
            raise RuntimeError("sync helper insertion anchor is missing")
        content = content.replace(anchor, command + anchor, 1)

    # Concurrent schedule edits are resolved as one coherent row; field-level
    # merging could combine stability/interval/revision values that never
    # existed together on either device.
    concurrent_insert = (
        "        (None, Some(local), Some(remote)) => {\n"
        "            if APPEND_ONLY_TABLES.contains(&table) {\n"
    )
    if concurrent_insert in content and "ATOMIC_CONCURRENT_TABLES.contains(&table)" not in content:
        replacement = (
            "        (None, Some(local), Some(remote)) => {\n"
            "            if ATOMIC_CONCURRENT_TABLES.contains(&table) {\n"
            "                conflicts.push(conflict(\n"
            "                    table,\n"
            "                    row_key,\n"
            "                    None,\n"
            "                    SyncConflictKind::ConcurrentInsert,\n"
            "                    None,\n"
            "                    None,\n"
            "                    None,\n"
            "                ));\n"
            "                return Ok(Some(local.clone()));\n"
            "            }\n"
            "            if APPEND_ONLY_TABLES.contains(&table) {\n"
        )
        content = content.replace(concurrent_insert, replacement, 1)

    concurrent_update = (
        "        (Some(base), Some(local), Some(remote)) => {\n"
        "            if APPEND_ONLY_TABLES.contains(&table) {\n"
    )
    if concurrent_update in content:
        replacement = (
            "        (Some(base), Some(local), Some(remote)) => {\n"
            "            if ATOMIC_CONCURRENT_TABLES.contains(&table) {\n"
            "                conflicts.push(conflict(\n"
            "                    table,\n"
            "                    row_key,\n"
            "                    None,\n"
            "                    SyncConflictKind::ConcurrentFieldEdit,\n"
            "                    None,\n"
            "                    None,\n"
            "                    None,\n"
            "                ));\n"
            "                return Ok(Some(local.clone()));\n"
            "            }\n"
            "            if APPEND_ONLY_TABLES.contains(&table) {\n"
        )
        content = content.replace(concurrent_update, replacement, 1)

    if "concurrent_schedule_edits_remain_atomic" not in content:
        test_anchor = "    #[test]\n    fn topological_order_places_parent_before_child() {\n"
        test = (
            "    #[test]\n"
            "    fn concurrent_schedule_edits_remain_atomic() {\n"
            "        let base = row(\"schedule\", \"Base\", \"1\", 1);\n"
            "        let local = row(\"schedule\", \"Local\", \"2\", 2);\n"
            "        let remote = row(\"schedule\", \"Remote\", \"3\", 2);\n"
            "        let mut conflicts = Vec::new();\n"
            "        let merged = merge_row(\n"
            "            \"schedules\",\n"
            "            \"schedule\",\n"
            "            Some(&base),\n"
            "            Some(&local),\n"
            "            Some(&remote),\n"
            "            &mut conflicts,\n"
            "        )\n"
            "        .expect(\"schedule merge should be represented\")\n"
            "        .expect(\"schedule should remain\");\n"
            "        assert_eq!(merged, local);\n"
            "        assert_eq!(conflicts.len(), 1);\n"
            "        assert!(conflicts[0].column.is_none());\n"
            "    }\n\n"
        )
        if test_anchor in content:
            content = content.replace(test_anchor, test + test_anchor, 1)
    write(relative, content)


def remove_self() -> None:
    for relative in (
        "scripts/pr51-wire-credentials.py",
        ".github/workflows/pr51-wire-credentials.yml",
    ):
        target = ROOT / relative
        if target.exists():
            target.unlink()


def main() -> None:
    patch_lib()
    patch_transport()
    patch_sync_core()
    remove_self()


if __name__ == "__main__":
    main()
