from pathlib import Path

path = Path('.github/pr41-final-fixes.py')
text = path.read_text(encoding='utf-8')

old_first = '''replace_once(
    "src-tauri/src/pdf_backend.rs",
    '    "oxidize-pdf-4.2.2-rag-native-first-ocr-v3"\\n',
    '    "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\\n',
)
'''
new_first = '''pdf_backend_path = "src-tauri/src/pdf_backend.rs"
pdf_backend_text = read(pdf_backend_path)
old_backend_id = "oxidize-pdf-4.2.2-rag-native-first-ocr-v3"
if pdf_backend_text.count(old_backend_id) != 2:
    raise SystemExit(
        f"{pdf_backend_path}: expected backend id twice, found {pdf_backend_text.count(old_backend_id)}"
    )
write(pdf_backend_path, pdf_backend_text.replace(old_backend_id, "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"))
'''
old_second = '''replace_once(
    "src-tauri/src/pdf_backend.rs",
    '            "oxidize-pdf-4.2.2-rag-native-first-ocr-v3"\\n',
    '            "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\\n',
)
'''
if text.count(old_first) != 1 or text.count(old_second) != 1:
    raise SystemExit('expected PDF backend applicator blocks not found')
text = text.replace(old_first, new_first, 1).replace(old_second, '', 1)

# Choosing a provider in an unsaved New profile is part of creating that profile,
# not navigation away from dirty work. Route it directly to the controller's
# draft initializer so the select does not trigger the discard-confirmation guard.
provider_change = '              onProviderChange={(providerKind) => void beginCreate(providerKind)}\n'
if text.count(provider_change) != 1:
    raise SystemExit(f'expected one generated provider-change callback, found {text.count(provider_change)}')
text = text.replace(provider_change, '              onProviderChange={controller.beginCreate}\n', 1)

lifetime_fixes = [
    (
'''        statement
            .query_map(params![input.session_id, input.note_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
''',
'''        let rows = statement
            .query_map(params![input.session_id, input.note_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
'''),
    (
'''        statement
            .query_map([input.note_id.as_str()], |row| {
                Ok(PlannedCard {
                    deck_id: row.get(0)?,
                    card_id: row.get(1)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
''',
'''        let rows = statement
            .query_map([input.note_id.as_str()], |row| {
                Ok(PlannedCard {
                    deck_id: row.get(0)?,
                    card_id: row.get(1)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
'''),
    (
'''        statement
            .query_map([input.session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
''',
'''        let rows = statement
            .query_map([input.session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
'''),
]
for old, new in lifetime_fixes:
    if text.count(old) != 1:
        raise SystemExit(f'expected one generated Rust lifetime fragment, found {text.count(old)}')
    text = text.replace(old, new, 1)

path.write_text(text, encoding='utf-8')
