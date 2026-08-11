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
path.write_text(text, encoding='utf-8')
