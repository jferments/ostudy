#!/usr/bin/env python3
"""Correct generated Rust ownership/type and canonical-schema issues."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return updated


def leading_spaces(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def materialize_tail_query_results(path: str) -> None:
    """Bind block-tail query_map chains before their Statement is dropped."""

    lines = read(path).splitlines()
    output: list[str] = []
    index = 0
    changed = 0
    while index < len(lines):
        line = lines[index]
        if line.strip() == "statement" and index + 1 < len(lines):
            indent = leading_spaces(line)
            if lines[index + 1].lstrip().startswith(".query_map("):
                end = index + 1
                while end < len(lines):
                    candidate = lines[end]
                    if candidate.strip() and leading_spaces(candidate) <= indent:
                        break
                    end += 1
                if end < len(lines) and lines[end].lstrip().startswith("}"):
                    tail = end - 1
                    while tail > index and not lines[tail].strip():
                        tail -= 1
                    output.append(" " * indent + "let rows = statement")
                    output.extend(lines[index + 1 : tail])
                    output.append(lines[tail] + ";")
                    output.append(" " * indent + "rows")
                    index = end
                    changed += 1
                    continue
        output.append(line)
        index += 1

    if changed:
        write(path, "\n".join(output) + "\n")


# Imports used only by test modules should not warn in normal builds.
path = "src-tauri/src/data_repairs.rs"
text = read(path)
text = text.replace("    error::CommandError,\n", "")
text = replace_once(
    text,
    "    use crate::database::Database;",
    "    use crate::{database::Database, error::CommandError};",
    "repair test import",
)
write(path, text)

path = "src-tauri/src/library_backup.rs"
text = read(path).replace("    io::{Read, Write},\n", "    io::Write,\n")
text = replace_once(
    text,
    '''    let temporary = extract_database_to
        .map(Path::to_path_buf)
        .unwrap_or_else(|| source.with_extension(format!("validate.{}.sqlite3", std::process::id())));''',
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

# The active template writer must use only the schema-13 rich document columns.
path = "src-tauri/src/authoring/stage3_v2/templates.rs"
text = read(path)
text = replace_once(
    text,
    '''                "INSERT INTO card_template_rules (
                     id, template_id, name, front_pattern, back_pattern,
                     front_doc, back_doc, position
                 ) VALUES (?1, ?2, ?3, '[rich document]', '[rich document]', ?4, ?5, ?6)",''',
    '''                "INSERT INTO card_template_rules (
                     id, template_id, name, front_doc, back_doc, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",''',
    "canonical template insert",
)
write(path, text)

# The legacy metadata tuple needs explicit String types for both optional text
# fields; inference cannot see through map_or's later clone calls.
path = "src-tauri/src/studydeck/read.rs"
text = read(path)
text = replace_once(
    text,
    "(row.get::<_, i64>(1)? == 1, row.get(2)?, row.get(3)?),",
    "(\n                        row.get::<_, i64>(1)? == 1,\n                        row.get::<_, String>(2)?,\n                        row.get::<_, String>(3)?,\n                    ),",
    "legacy metadata types",
)
text = replace_once(
    text,
    '''            let cloze_mode = legacy_metadata.map_or(row.12, |value| value.2.clone());
            Ok(PackageNote {''',
    '''            let cloze_mode = legacy_metadata.map_or(row.12, |value| value.2.clone());
            let duplicate_hash = if row.9.is_empty() {
                duplicate_fingerprint(&validated.front_text, &validated.back_text)
            } else {
                row.9
            };
            Ok(PackageNote {''',
    "legacy duplicate fingerprint binding",
)
text = replace_once(
    text,
    '''                front_text: validated.front_text,
                back_text: validated.back_text,
                duplicate_hash: if row.9.is_empty() {
                    duplicate_fingerprint(&validated.front_text, &validated.back_text)
                } else {
                    row.9
                },''',
    '''                front_text: validated.front_text,
                back_text: validated.back_text,
                duplicate_hash,''',
    "legacy duplicate fingerprint field",
)
write(path, text)

# Every error after the first staged-file rename must return all already-renamed
# files to staging before the SQLite transaction rolls back.
path = "src-tauri/src/studydeck/import.rs"
text = read(path)
text = regex_once(
    text,
    r'''    let mut promoted = Vec::new\(\);\n(?P<body>.*?)\n    if let Err\(error\) = transaction\.commit\(\) \{''',
    '''    let mut promoted = Vec::new();
    let promotion_result = (|| -> Result<(), CommandError> {
\g<body>
        Ok(())
    })();
    if let Err(error) = promotion_result {
        compensate_promotions(&promoted);
        return Err(error);
    }

    if let Err(error) = transaction.commit() {''',
    "import promotion compensation",
)
write(path, text)

for rust_file in [
    "src-tauri/src/authoring/commands.rs",
    "src-tauri/src/authoring/stage3_v2/normalization.rs",
    "src-tauri/src/studydeck/export.rs",
    "src-tauri/src/studydeck/import.rs",
    "src-tauri/src/studydeck/read.rs",
    "src-tauri/src/library_backup.rs",
]:
    materialize_tail_query_results(rust_file)

print("Applied Stage 3.4/3.5 native compile and canonical-schema corrections.")
