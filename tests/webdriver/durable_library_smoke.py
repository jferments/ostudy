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

    webdriver.progress("Navigating to the Library Workbench to verify deck and card persistence")
    deadline = webdriver.deadline_after()
    webdriver.open_library_view(session_id, "decks", deadline)
    webdriver.open_workbench_deck(session_id, deck_name, deadline)
    webdriver.find_card_row(session_id, question, deadline)

    webdriver.progress("Verifying persisted review history in Progress after restart")
    deadline = webdriver.deadline_after(webdriver.ANALYTICS_TIMEOUT_SECONDS)
    progress_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-progress']",
        deadline,
    )
    webdriver.click(session_id, progress_button)
    webdriver.wait_for_main_heading(session_id, "Progress", deadline)
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

    webdriver.progress("Verifying post-restart navigation to Settings in the header")
    deadline = webdriver.deadline_after()
    settings_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='app-settings']",
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
