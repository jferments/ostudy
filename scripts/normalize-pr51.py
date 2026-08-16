#!/usr/bin/env python3
"""Apply deterministic one-time repairs before cargo fmt/Cargo.lock generation.

This script exists only while PR #51 is under active development. It keeps the
workflow YAML small and makes every textual repair independently auditable.
Remove it together with normalize-pr51-rust.yml before marking the PR ready.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: Path, old: str, new: str, *, optional: bool = False) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        if optional:
            return
        raise RuntimeError(f"expected normalization pattern missing in {path}: {old[:120]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def normalize_sync_module() -> None:
    path = ROOT / "src-tauri/src/device_sync/mod.rs"
    text = path.read_text(encoding="utf-8")

    if "pub qr_svg: String," not in text:
        text = text.replace(
            "    pub invitation: String,\n    pub endpoint: String,\n",
            "    pub invitation: String,\n    pub qr_svg: String,\n    pub endpoint: String,\n",
            1,
        )
    if "let qr_svg = render_pairing_qr(&invitation)?;" not in text:
        marker = '    store.create_session(&session_id, "host", &endpoint, expires_at)?;\n'
        if marker not in text:
            raise RuntimeError("pairing session marker missing")
        text = text.replace(
            marker,
            "    let qr_svg = render_pairing_qr(&invitation)?;\n" + marker,
            1,
        )
    if "        qr_svg,\n        endpoint," not in text:
        text = text.replace(
            "        invitation,\n        endpoint,\n",
            "        invitation,\n        qr_svg,\n        endpoint,\n",
            1,
        )

    if "fn render_pairing_qr(" not in text:
        helper = '''fn render_pairing_qr(invitation: &str) -> Result<String, CommandError> {
    use qrcode::{render::svg, QrCode};

    let code = QrCode::new(invitation.as_bytes()).map_err(|_| {
        CommandError::new(
            "device_sync_invitation_too_large",
            "The pairing invitation is too large to render as a QR code.",
        )
    })?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(320, 320)
        .max_dimensions(640, 640)
        .build())
}

'''
        marker = "fn parse_invitation(value: &str) -> Result<PairingInvitation, CommandError> {"
        if marker not in text:
            raise RuntimeError("invitation parser marker missing")
        text = text.replace(marker, helper + marker, 1)

    if "pub fn scan_device_pairing_qr(" not in text:
        command = '''#[tauri::command]
pub fn scan_device_pairing_qr(app: AppHandle) -> Result<Option<String>, CommandError> {
    #[cfg(mobile)]
    {
        use tauri_plugin_ostudy_platform::{OstudyPlatformExt, ScanQrCodeRequest};

        let result = app
            .ostudy_platform()
            .scan_qr_code(ScanQrCodeRequest {
                prompt: "Scan the ostudy pairing code shown on the other device".to_string(),
            })
            .map_err(|error| {
                CommandError::new(
                    "device_sync_qr_scan_failed",
                    format!("The native QR scanner failed: {error}"),
                )
            })?;
        Ok(if result.cancelled { None } else { result.contents })
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err(CommandError::new(
            "device_sync_qr_scan_unavailable",
            "Use the pairing invitation text or scan it with the receiving mobile device.",
        ))
    }
}

'''
        marker = "#[tauri::command]\npub fn remove_device_sync_peer("
        if marker not in text:
            raise RuntimeError("remove-peer command marker missing")
        text = text.replace(marker, command + marker, 1)

    old = '''        "merged" => input.merged_payload_json.as_deref().ok_or_else(|| {
            CommandError::new(
                "device_sync_resolution_invalid",
                "A merged conflict resolution must include the merged row payload.",
            )
        })?,
'''
    new = '''        "merged" => Some(input.merged_payload_json.as_deref().ok_or_else(|| {
            CommandError::new(
                "device_sync_resolution_invalid",
                "A merged conflict resolution must include the merged row payload.",
            )
        })?),
'''
    if old in text:
        text = text.replace(old, new, 1)

    old = '''    let unsigned_server = UnsignedServerHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        identity_fingerprint: &identity_key.fingerprint(),
'''
    new = '''    let identity_fingerprint = identity_key.fingerprint();
    let unsigned_server = UnsignedServerHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        identity_fingerprint: &identity_fingerprint,
'''
    if old in text:
        text = text.replace(old, new, 1)
    text = text.replace(
        "            identity_fingerprint: identity_key.fingerprint(),\n",
        "            identity_fingerprint: identity_fingerprint.clone(),\n",
        1,
    )

    old = "    let unsigned = UnsignedClientHello {\n"
    if "let ephemeral_public_key = BASE64_STANDARD.encode(&ephemeral.public_key);" not in text:
        if old not in text:
            raise RuntimeError("unsigned client marker missing")
        text = text.replace(
            old,
            "    let ephemeral_public_key = BASE64_STANDARD.encode(&ephemeral.public_key);\n" + old,
            1,
        )
    text = text.replace(
        "        ephemeral_public_key: &BASE64_STANDARD.encode(&ephemeral.public_key),\n",
        "        ephemeral_public_key: &ephemeral_public_key,\n",
        1,
    )
    text = text.replace(
        "    let ephemeral_public_key = unsigned.ephemeral_public_key.to_string();\n",
        "",
        1,
    )

    path.write_text(text, encoding="utf-8")


def normalize_merge_test() -> None:
    path = ROOT / "src-tauri/src/device_sync/merge.rs"
    text = path.read_text(encoding="utf-8")
    old = '''        let row = row(&[("id", CellValue::Text("a".repeat(32)))]);
        let (merged, conflicts) = merge_rows(
            Some(&row),
            None,
            Some(&row),
'''
    new = '''        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
        ]);
        let mut edited = base.clone();
        edited.insert(
            "name".to_string(),
            CellValue::Text("Edited".to_string()),
        );
        let (merged, conflicts) = merge_rows(
            Some(&base),
            None,
            Some(&edited),
'''
    if old in text:
        text = text.replace(old, new, 1)
    path.write_text(text, encoding="utf-8")


def normalize_android_capabilities() -> None:
    path = ROOT / "src-tauri/src/platform/android.rs"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        'ServiceCapability::unavailable("android_ml_kit_ocr_unavailable")',
        'ServiceCapability::unavailable("android_tesseract_and_ml_kit_ocr_unavailable")',
    )
    text = text.replace(
        'camera: ServiceCapability::unavailable("android_camera_adapter_not_implemented"),\n'
        '                qr_scan: ServiceCapability::unavailable("android_qr_adapter_not_implemented"),',
        "camera: ServiceCapability::available(),\n"
        "                qr_scan: ServiceCapability::available(),",
    )
    text = text.replace(
        'background_execution: ServiceCapability::unavailable(\n'
        '                    "android_background_execution_not_implemented",\n'
        "                ),",
        "background_execution: ServiceCapability::available(),",
    )
    path.write_text(text, encoding="utf-8")


def register_qr_command() -> None:
    path = ROOT / "src-tauri/src/lib.rs"
    text = path.read_text(encoding="utf-8")
    if "device_sync::scan_device_pairing_qr," not in text:
        marker = "            device_sync::join_device_pairing,\n"
        if marker not in text:
            raise RuntimeError("join-device command registration marker missing")
        text = text.replace(
            marker,
            marker + "            device_sync::scan_device_pairing_qr,\n",
            1,
        )
    path.write_text(text, encoding="utf-8")


def fix_android_install_output() -> None:
    path = ROOT / "scripts/android-install.sh"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        '"${SERIAL:-${devices[0]} }"',
        '"${SERIAL:-${devices[0]}}"',
    )
    path.write_text(text, encoding="utf-8")


def main() -> None:
    normalize_sync_module()
    normalize_merge_test()
    normalize_android_capabilities()
    register_qr_command()
    fix_android_install_output()


if __name__ == "__main__":
    main()
