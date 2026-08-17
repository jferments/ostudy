from pathlib import Path

path = Path("tests/webdriver/ui_platform_smoke.py")
source = path.read_text(encoding="utf-8")
old = '''    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)

    create_cards = find_workspace_tab(session_id, "Create Cards", deadline)
'''
new = '''    # The preceding compact-navigation scenario already returns to the
    # Flashcards route. Assert that invariant with read-only lookups instead of
    # sending a redundant navigation click whose response could be lost after
    # the route was already active.
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks'][aria-current='page']",
        deadline,
    )
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)

    create_cards = find_workspace_tab(session_id, "Create Cards", deadline)
'''
count = source.count(old)
if count != 1:
    raise SystemExit(f"expected one medium-layout redundant Flashcards click, found {count}")
path.write_text(source.replace(old, new, 1), encoding="utf-8")
