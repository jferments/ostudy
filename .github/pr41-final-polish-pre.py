from pathlib import Path

path = Path('.github/pr41-final-polish.py')
text = path.read_text(encoding='utf-8')

# Make the Edit-card button selector specific enough to distinguish it from End session.
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
text = text.replace(old, new, 1)

# Check the reversible marker only after the asynchronously loaded card page exists.
old = '''# Add a UI regression assertion for the reversible indicator.
replace_once(
    "src/screens/DecksScreen.test.tsx",
    """    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id, \\"detail\\");\\n  });\\n""",
    """    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id, \\"detail\\");\\n    expect(screen.getByLabelText(\\"Reversible\\")).toBeTruthy();\\n    expect(screen.queryByText(/card variants/u)).toBeNull();\\n  });\\n""",
)
'''
new = '''# Add a UI regression assertion for the reversible indicator after card hydration.
replace_once(
    "src/screens/DecksScreen.test.tsx",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
    """    await waitFor(() => {\\n      expect(screen.getByRole(\\"heading\\", { name: \\"Dog facts\\" })).toBeTruthy();\\n    });\\n    expect(await screen.findByLabelText(\\"Reversible\\")).toBeTruthy();\\n    expect(screen.queryByText(/card variants/u)).toBeNull();\\n\\n    const detailPractice = screen.getByRole(\\"button\\", { name: \\"Practice\\" });\\n""",
)
'''
if text.count(old) != 1:
    raise SystemExit(f'applicator reversible-test marker count={text.count(old)}')
text = text.replace(old, new, 1)

# Ensure a flex-grown flip card's absolute faces fill the resulting card height.
old = '''.active-practice-screen .practice-card-surface,
.active-practice-screen .practice-flip-card {
  min-height: clamp(170px, 34vh, 420px);
  flex: 1 1 auto;
  overflow: auto;
}
'''
new = '''.active-practice-screen .practice-card-surface,
.active-practice-screen .practice-flip-card {
  min-height: clamp(170px, 34vh, 420px);
  flex: 1 1 auto;
  overflow: auto;
}

.active-practice-screen .practice-flip-inner {
  height: 100%;
  min-height: 100%;
}
'''
if text.count(old) != 1:
    raise SystemExit(f'applicator flip-height marker count={text.count(old)}')
text = text.replace(old, new, 1)

path.write_text(text, encoding='utf-8')
