#!/usr/bin/env python3
"""Add the required credentialCount field to device-sync receive fixtures."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ANCHORS = (
    "receiveDeviceSyncInvitation).mockResolvedValue(",
    "receiveDeviceSyncInvitation).mockResolvedValueOnce(",
    ": ReceivedDeviceSync = ",
    "satisfies ReceivedDeviceSync",
)


def matching_brace(text: str, start: int) -> int:
    depth = 0
    quote: str | None = None
    escaped = False
    index = start
    while index < len(text):
        character = text[index]
        if quote is not None:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
            index += 1
            continue
        if character in ('"', "'", "`"):
            quote = character
        elif character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise RuntimeError("Unbalanced object fixture")


def patch_file(path: Path) -> bool:
    text = path.read_text(encoding="utf-8")
    changed = False
    search_from = 0
    while True:
        matches = [(text.find(anchor, search_from), anchor) for anchor in ANCHORS]
        matches = [(index, anchor) for index, anchor in matches if index >= 0]
        if not matches:
            break
        anchor_index, anchor = min(matches)
        object_start = text.find("{", anchor_index + len(anchor))
        if object_start < 0:
            search_from = anchor_index + len(anchor)
            continue
        object_end = matching_brace(text, object_start)
        body = text[object_start:object_end]
        if "credentialCount:" not in body and ("preview:" in body or anchor.startswith(":")):
            line_start = text.rfind("\n", 0, object_end) + 1
            closing_indent = text[line_start:object_end]
            insertion = f"{closing_indent}  credentialCount: 0,\n"
            text = text[:line_start] + insertion + text[line_start:]
            changed = True
            search_from = object_end + len(insertion) + 1
        else:
            search_from = object_end + 1
    if changed:
        path.write_text(text, encoding="utf-8")
    return changed


def main() -> None:
    changed = []
    for path in (ROOT / "src").rglob("*.ts*"):
        if patch_file(path):
            changed.append(path.relative_to(ROOT).as_posix())
    if not changed:
        raise RuntimeError("No ReceivedDeviceSync fixture requiring credentialCount was found")
    for relative in changed:
        print(relative)
    for relative in (
        "scripts/pr51-fix-credential-fixtures.py",
        ".github/workflows/pr51-fix-credential-fixtures.yml",
    ):
        path = ROOT / relative
        if path.exists():
            path.unlink()


if __name__ == "__main__":
    main()
