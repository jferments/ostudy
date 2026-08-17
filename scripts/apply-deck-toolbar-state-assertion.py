from pathlib import Path

path = Path("tests/webdriver/deck_toolbar_layout_smoke.py")
source = path.read_text(encoding="utf-8")
old = '''    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)
    verify_compact_tabs(session_id, width)
    decks = ui.find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, decks)
    folders_heading = webdriver.find_element(
'''
new = '''    # Each viewport check begins and ends on Flashcards > Decks. Resolve that
    # already-established state after resizing instead of caching elements and
    # issuing redundant clicks while React may be replacing the tab/navigation
    # nodes during responsive layout work.
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks'][aria-current='page']",
        deadline,
    )
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)
    verify_compact_tabs(session_id, width)
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tab' and @aria-selected='true' and normalize-space(.)='Decks']",
        deadline,
    )
    folders_heading = webdriver.find_element(
'''
count = source.count(old)
if count != 1:
    raise SystemExit(f"expected one redundant Flashcards/Decks click sequence, found {count}")
path.write_text(source.replace(old, new, 1), encoding="utf-8")
