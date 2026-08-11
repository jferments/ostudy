from pathlib import Path

path = Path('.github/pr41-final-polish.py')
text = path.read_text(encoding='utf-8')

# The main applicator is now self-contained; make its Edit-card source marker
# match the clean product tree rather than an already-transformed intermediate.
old = '''replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button practice-header-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button practice-header-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
)
'''
new = '''replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
    """          <button\\n            type=\\"button\\"\\n            className=\\"secondary-button practice-header-button\\"\\n            disabled={busy}\\n            onClick={onEdit}\\n""",
)
'''
if text.count(old) != 1:
    raise SystemExit(f'edit marker count={text.count(old)}')
text = text.replace(old, new, 1)

# Insert the reversible assertion into the clean test after asynchronous card
# hydration; do not require that assertion to already exist in the source.
old = '''replace_once(
    "src/screens/DecksScreen.test.tsx",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n    expect(await screen.findByLabelText(\\"Reversible\\")).toBeTruthy();\\n    expect(screen.queryByText(/card variants/u)).toBeNull();\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n    expect(await screen.findByLabelText(\\"Reversible\\")).toBeTruthy();\\n    expect(screen.queryByText(/card variants/u)).toBeNull();\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
)
'''
new = '''replace_once(
    "src/screens/DecksScreen.test.tsx",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n    expect(await screen.findByLabelText(\\"Reversible\\")).toBeTruthy();\\n    expect(screen.queryByText(/card variants/u)).toBeNull();\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
)
'''
if text.count(old) != 1:
    raise SystemExit(f'reversible test marker count={text.count(old)}')
text = text.replace(old, new, 1)

path.write_text(text, encoding='utf-8')
