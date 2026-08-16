#!/usr/bin/env python3
"""One-shot insertion of the Android QR camera control into Device Sync settings."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    settings_path = ROOT / "src/screens/settings/DeviceSyncSettings.tsx"
    content = settings_path.read_text(encoding="utf-8")
    if 'import { AndroidSyncQrScanner } from "./AndroidSyncQrScanner";' not in content:
        anchor = 'import { SyncQrCode } from "../../components/SyncQrCode";\n'
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate SyncQrCode import")
        content = content.replace(
            anchor,
            anchor + 'import { AndroidSyncQrScanner } from "./AndroidSyncQrScanner";\n',
            1,
        )
    scanner = '''          <AndroidSyncQrScanner\n            disabled={busy}\n            onScanned={setReceivedInvitation}\n            onError={setErrorMessage}\n          />\n'''
    if "<AndroidSyncQrScanner" not in content:
        anchor = '''          <button\n            type="button"\n            className="primary-button"\n            disabled={busy || !receivedInvitation.trim() || !deviceName.trim()}\n            onClick={() => void receiveQrInvitation()}\n          >\n            Receive encrypted sync\n          </button>\n'''
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate receive invitation button")
        content = content.replace(anchor, scanner + anchor, 1)
    settings_path.write_text(content, encoding="utf-8")

    css_path = ROOT / "src/screens/settings/DeviceSyncSettings.module.css"
    css = css_path.read_text(encoding="utf-8")
    css = css.replace(
        "  grid-template-columns: minmax(0, 1fr) auto;\n",
        "  grid-template-columns: minmax(0, 1fr) repeat(2, auto);\n",
        1,
    )
    css_path.write_text(css, encoding="utf-8")

    for relative in ("scripts/pr51-wire-qr-ui.py", ".github/workflows/pr51-wire-qr-ui.yml"):
        path = ROOT / relative
        if path.exists():
            path.unlink()


if __name__ == "__main__":
    main()
