#!/usr/bin/env python3
"""Apply incremental safety corrections to the already-generated source."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def regex_once(
    text: str,
    pattern: str,
    replacement: str | re.Callable,
    label: str,
) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return updated


# Backup validation must not require write access beside a user-selected backup.
path = "src-tauri/src/library_backup.rs"
text = read(path)
text = regex_once(
    text,
    r'''    let temporary = extract_database_to\s*
        \.map\(Path::to_path_buf\)\s*
        \.unwrap_or_else\(\|\| \{\s*
            source\.with_extension\(format!\("validate\.\{\}\.sqlite3", std::process::id\(\)\)\)\s*
        \}\);''',
    '''    let temporary = extract_database_to.map(Path::to_path_buf).unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "ostudy-backup-validation-{}-{}.sqlite3",
            std::process::id(),
            unix_time_millis().unwrap_or_default()
        ))
    });''',
    "backup validation temp path",
)
write(path, text)

# A failure at any point after the first asset rename must restore every file to
# staging before the SQLite transaction rolls back.
path = "src-tauri/src/studydeck/import.rs"
text = read(path)


def wrap_promotion(match: re.Match[str]) -> str:
    return f'''    let mut promoted = Vec::new();
    let promotion_result = (|| -> Result<(), CommandError> {{
{match.group("body")}
        Ok(())
    }})();
    if let Err(error) = promotion_result {{
        compensate_promotions(&promoted);
        return Err(error);
    }}

    if let Err(error) = transaction.commit() {{'''


text = regex_once(
    text,
    r'''    let mut promoted = Vec::new\(\);\n(?P<body>.*?)\n    if let Err\(error\) = transaction\.commit\(\) \{''',
    wrap_promotion,
    "import promotion compensation",
)
write(path, text)

print("Applied incremental Stage 3.4/3.5 safety corrections.")
