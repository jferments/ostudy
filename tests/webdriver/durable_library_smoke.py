#!/usr/bin/env python3
"""Restoration-aware entry point for the durable native WebDriver smoke."""

from __future__ import annotations

import json

import smoke as webdriver


def verify_persistence_after_restart(
    session_id: str,
    deck_name: str,
    question: str,
) -> None:
    """Verify A2 UI restoration, then navigate explicitly to durable library state."""
    webdriver.progress("Verifying restored Search workspace checkpoint after restart")
    deadline = webdriver.deadline_after(webdriver.ANALYTICS_TIMEOUT_SECONDS)
    webdriver.verify_shell(session_id, deadline)
    webdriver.verify_search_workspace_selected(session_id, deadline)

    webdriver.progress("Navigating to Flashcards > Decks to verify deck and card persistence")
    deadline = webdriver.deadline_after()
    decks_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, decks_button)
    webdriver.find_element(
        session_id,
        "css selector",
        "main [role='tablist'][aria-label='Flashcards workspaces']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tab' and @aria-selected='true'][contains(normalize-space(.), 'Decks')]",
        deadline,
    )
    # Deck rows are article-owned now. The clickable/openable surface is the
    # accessible role=button child, not a native <button>. Keep the smoke tied
    # to the same accessibility contract keyboard users depend on rather than
    # to the element tag used by an older deck-library implementation.
    deck_button = webdriver.find_element(
        session_id,
        "xpath",
        f"//main//*[@role='button' and @aria-label={json.dumps(f'Open {deck_name}')}]",
        deadline,
    )
    webdriver.click(session_id, deck_button)
    webdriver.wait_for_main_heading(session_id, deck_name, deadline)
    webdriver.find_element(
        session_id,
        "xpath",
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    webdriver.progress("Verifying persisted review analytics after restart")
    deadline = webdriver.deadline_after(webdriver.ANALYTICS_TIMEOUT_SECONDS)
    analytics_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-analytics']",
        deadline,
    )
    webdriver.click(session_id, analytics_button)
    webdriver.find_element(
        session_id,
        "css selector",
        webdriver.ANALYTICS_SCREEN_SELECTOR,
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//section[.//h2[normalize-space(.)='Recent reviews']]"
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    webdriver.progress("Verifying post-restart navigation")
    deadline = webdriver.deadline_after()
    settings_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
        deadline,
    )
    webdriver.click(session_id, settings_button)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Settings",
        deadline,
    )


if __name__ == "__main__":
    webdriver.verify_persistence_after_restart = verify_persistence_after_restart
    raise SystemExit(webdriver.main())
