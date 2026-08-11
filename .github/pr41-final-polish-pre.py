from pathlib import Path

path = Path('.github/pr41-final-polish.py')
text = path.read_text(encoding='utf-8')
old = '''replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """            className=\\"secondary-button\\"\\n""",
    """            className=\\"secondary-button practice-header-button\\"\\n""",
)
'''
new = '''replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button practice-header-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
)
'''
if text.count(old) != 1:
    raise SystemExit(f'applicator edit-button marker count={text.count(old)}')
path.write_text(text.replace(old, new, 1), encoding='utf-8')
