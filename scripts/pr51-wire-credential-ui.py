#!/usr/bin/env python3
"""One-shot frontend wiring for explicit QR-only credential transfer."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def write(relative: str, content: str) -> None:
    (ROOT / relative).write_text(content, encoding="utf-8")


def patch_settings() -> None:
    relative = "src/screens/settings/DeviceSyncSettings.tsx"
    content = read(relative)
    if "  discardDeviceSyncPreview,\n" not in content:
        anchor = "  commitDeviceSync,\n"
        if anchor not in content:
            raise RuntimeError("commitDeviceSync import is missing")
        content = content.replace(anchor, anchor + "  discardDeviceSyncPreview,\n", 1)

    if "const [includeCredentials, setIncludeCredentials]" not in content:
        anchor = "  const [includeAiProfiles, setIncludeAiProfiles] = useState(true);\n"
        if anchor not in content:
            raise RuntimeError("AI profile state is missing")
        content = content.replace(
            anchor,
            anchor + "  const [includeCredentials, setIncludeCredentials] = useState(false);\n",
            1,
        )

    if "        includeCredentials,\n" not in content:
        anchor = "        includeAiProfiles,\n        hostOverride: hostOverride.trim() || null,\n"
        if anchor not in content:
            raise RuntimeError("QR session options are missing")
        content = content.replace(
            anchor,
            "        includeAiProfiles,\n        includeCredentials,\n"
            "        hostOverride: hostOverride.trim() || null,\n",
            1,
        )

    old_receive_toast = (
        "      showToast(\"success\", `Received encrypted changes from ${received.peer.deviceName}.`);\n"
    )
    if old_receive_toast in content:
        content = content.replace(
            old_receive_toast,
            "      showToast(\n"
            "        \"success\",\n"
            "        received.credentialCount > 0\n"
            "          ? `Received encrypted changes and ${received.credentialCount} API credential${received.credentialCount === 1 ? \"\" : \"s\"} from ${received.peer.deviceName}.`\n"
            "          : `Received encrypted changes from ${received.peer.deviceName}.`,\n"
            "      );\n",
            1,
        )

    old_commit_toast = (
        "      showToast(\n"
        "        \"success\",\n"
        "        `Synchronized ${result.upsertedRows} records, ${result.deletedRows} deletions, and ${result.writtenAssets} images.`,\n"
        "      );\n"
    )
    if old_commit_toast in content:
        content = content.replace(
            old_commit_toast,
            "      const credentialSummary =\n"
            "        result.importedCredentialCount > 0 || result.credentialErrorCount > 0\n"
            "          ? ` Imported ${result.importedCredentialCount} API credential${result.importedCredentialCount === 1 ? \"\" : \"s\"}; ${result.credentialErrorCount} failed.`\n"
            "          : \"\";\n"
            "      showToast(\n"
            "        \"success\",\n"
            "        `Synchronized ${result.upsertedRows} records, ${result.deletedRows} deletions, and ${result.writtenAssets} images.${credentialSummary}`,\n"
            "      );\n",
            1,
        )

    if "Transfer API credentials" not in content:
        anchor = (
            "          <label className={styles.optionCard}>\n"
            "            <input\n"
            "              type=\"checkbox\"\n"
            "              checked={includeAiProfiles}\n"
            "              disabled={busy}\n"
            "              onChange={(event) => setIncludeAiProfiles(event.target.checked)}\n"
            "            />\n"
            "            <span>\n"
            "              <strong>AI connection profiles</strong>\n"
            "              <small>Provider, endpoint, and model metadata only. API keys remain excluded.</small>\n"
            "            </span>\n"
            "          </label>\n"
        )
        addition = (
            "          <label className={styles.optionCard}>\n"
            "            <input\n"
            "              type=\"checkbox\"\n"
            "              checked={includeCredentials}\n"
            "              disabled={busy || !includeAiProfiles}\n"
            "              onChange={(event) => setIncludeCredentials(event.target.checked)}\n"
            "            />\n"
            "            <span>\n"
            "              <strong>Transfer API credentials</strong>\n"
            "              <small>Explicit QR/LAN sessions only. Keys stay inside authenticated encryption and are written to the destination secure store after merge approval.</small>\n"
            "            </span>\n"
            "          </label>\n"
        )
        if anchor not in content:
            raise RuntimeError("AI profile option card is missing")
        content = content.replace(anchor, anchor + addition, 1)

    old_cancel = (
        "            <button\n"
        "              type=\"button\"\n"
        "              className=\"secondary-button\"\n"
        "              disabled={busy}\n"
        "              onClick={() => setPreview(null)}\n"
        "            >\n"
        "              Cancel merge\n"
        "            </button>\n"
    )
    if old_cancel in content:
        content = content.replace(
            old_cancel,
            "            <button\n"
            "              type=\"button\"\n"
            "              className=\"secondary-button\"\n"
            "              disabled={busy}\n"
            "              onClick={() => {\n"
            "                const previewId = preview.previewId;\n"
            "                setPreview(null);\n"
            "                void discardDeviceSyncPreview(previewId).catch((error: unknown) => {\n"
            "                  setErrorMessage(getErrorMessage(error));\n"
            "                });\n"
            "              }}\n"
            "            >\n"
            "              Cancel merge\n"
            "            </button>\n",
            1,
        )
    write(relative, content)


def patch_test() -> None:
    relative = "src/screens/settings/DeviceSyncSettings.test.tsx"
    content = read(relative)
    if "discardDeviceSyncPreview: vi.fn()," not in content:
        anchor = "    commitDeviceSync: vi.fn(),\n"
        content = content.replace(anchor, anchor + "    discardDeviceSyncPreview: vi.fn(),\n", 1)
    old_result = (
        "      writtenAssets: 0,\n"
        "    });\n"
    )
    if old_result in content:
        content = content.replace(
            old_result,
            "      writtenAssets: 0,\n"
            "      importedCredentialCount: 0,\n"
            "      credentialErrorCount: 0,\n"
            "    });\n",
            1,
        )
    write(relative, content)


def remove_self() -> None:
    for relative in (
        "scripts/pr51-wire-credential-ui.py",
        ".github/workflows/pr51-wire-credential-ui.yml",
    ):
        target = ROOT / relative
        if target.exists():
            target.unlink()


def main() -> None:
    patch_settings()
    patch_test()
    remove_self()


if __name__ == "__main__":
    main()
