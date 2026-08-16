#!/usr/bin/env python3
"""One-shot wiring for the Android synchronization QR plugin."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def patch_models() -> None:
    path = "plugins/ostudy-platform/src/models.rs"
    content = read(path)
    addition = r'''

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanQrCodeResult {
    pub cancelled: bool,
    pub value: Option<String>,
}
'''
    if "pub struct ScanQrCodeResult" not in content:
        content = content.rstrip() + addition + "\n"
    write(path, content)


def patch_mobile() -> None:
    path = "plugins/ostudy-platform/src/mobile.rs"
    content = read(path)
    registration_anchor = (
        "    let documents = api.register_android_plugin(PLUGIN_IDENTIFIER, \"OstudyDocumentPlugin\")?;\n"
    )
    registration = (
        "    #[cfg(target_os = \"android\")]\n"
        "    let qr = api.register_android_plugin(PLUGIN_IDENTIFIER, \"OstudyQrPlugin\")?;\n"
    )
    if "OstudyQrPlugin" not in content:
        if content.count(registration_anchor) != 1:
            raise RuntimeError("Could not locate Android document plugin registration")
        content = content.replace(registration_anchor, registration_anchor + registration, 1)

    if "        qr,\n" not in content:
        constructor_anchor = "        documents,\n"
        if content.count(constructor_anchor) != 1:
            raise RuntimeError("Could not locate mobile plugin constructor")
        content = content.replace(constructor_anchor, constructor_anchor + "        qr,\n", 1)

    if "    qr: PluginHandle<R>,\n" not in content:
        field_anchor = "    documents: PluginHandle<R>,\n"
        if content.count(field_anchor) != 1:
            raise RuntimeError("Could not locate mobile document plugin field")
        content = content.replace(field_anchor, field_anchor + "    qr: PluginHandle<R>,\n", 1)

    method = r'''

    pub fn scan_qr_code(&self) -> Result<ScanQrCodeResult> {
        self.qr
            .run_mobile_plugin("scanQrCode", ())
            .map_err(Into::into)
    }
'''
    if "pub fn scan_qr_code" not in content:
        close = content.rfind("\n}")
        if close < 0:
            raise RuntimeError("Could not locate mobile plugin implementation end")
        content = content[:close] + method + content[close:]
    write(path, content)


def patch_kotlin() -> None:
    path = "plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"
    content = read(path)
    if "import android.net.Uri\n" not in content:
        content = content.replace(
            "import android.content.Intent\n",
            "import android.content.Intent\nimport android.net.Uri\n",
            1,
        )
    content = content.replace(
        "scanner.process(InputImage.fromFilePath(activity, capture.toUri()))",
        "scanner.process(InputImage.fromFilePath(activity, Uri.fromFile(capture)))",
    )
    content = content.replace(
        ".mapNotNull(Barcode::getRawValue)",
        ".mapNotNull { barcode -> barcode.rawValue }",
    )
    content = content.replace(
        "import com.google.mlkit.vision.barcode.common.Barcode\n",
        "import com.google.mlkit.vision.barcode.common.Barcode\n",
    )
    write(path, content)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    content = read(path)
    if "mod device_sync_qr;" not in content:
        anchor = "mod device_sync_transport;\n"
        if anchor not in content:
            anchor = "mod device_sync;\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate device sync module declarations")
        content = content.replace(anchor, anchor + "mod device_sync_qr;\n", 1)
    if "device_sync_qr::scan_device_sync_qr" not in content:
        anchor = "            device_sync_transport::remove_trusted_sync_device,\n"
        if anchor not in content:
            anchor = "            device_sync::commit_device_sync,\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate device sync command registry")
        content = content.replace(
            anchor,
            anchor + "            device_sync_qr::scan_device_sync_qr,\n",
            1,
        )
    write(path, content)


def remove_one_shot_files() -> None:
    for relative in ("scripts/pr51-wire-qr.py", ".github/workflows/pr51-wire-qr.yml"):
        path = ROOT / relative
        if path.exists():
            path.unlink()


def main() -> None:
    patch_models()
    patch_mobile()
    patch_kotlin()
    patch_lib()
    remove_one_shot_files()


if __name__ == "__main__":
    main()
