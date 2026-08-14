#!/usr/bin/env python3
"""Restoration-aware entry point for the durable native WebDriver smoke."""

from __future__ import annotations

import json

import smoke as webdriver


_original_click = webdriver.click


def dismiss_optional_tutorial(session_id: str) -> bool:
    """Dismiss the optional first-run guide when it would intercept smoke actions."""
    result = webdriver.request(
        "POST",
        f"/session/{session_id}/elements",
        {
            "using": "css selector",
            "value": "button[aria-label='Close tutorial']",
        },
    )
    if not isinstance(result, list):
        return False
    close_button = next(
        (
            str(item[webdriver.ELEMENT_KEY])
            for item in result
            if isinstance(item, dict) and webdriver.ELEMENT_KEY in item
        ),
        None,
    )
    if close_button is None:
        return False
    _original_click(session_id, close_button)
    return True


def tutorial_aware_click(session_id: str, element_id: str) -> None:
    """Click the requested control without letting an optional tutorial race the test."""
    dismiss_optional_tutorial(session_id)
    try:
        _original_click(session_id, element_id)
    except Exception:
        # A contextual tutorial can auto-start 500-700 ms after its anchor
        # appears, including after this function's proactive check. Recover
        # only when that overlay actually appeared; unrelated click failures
        # remain visible to the smoke test.
        if not dismiss_optional_tutorial(session_id):
            raise
        _original_click(session_id, element_id)


def verify_persistence_after_restart(
    session_id: str,
    deck_name: str,
    question: str,
) -> None:
    """Verify A2 UI restoration, then navigate explicitly to durable library state."""
    webdriver.progress("Verifying restored Analytics checkpoint after restart")
    deadline = webdriver.deadline_after(webdriver.ANALYTICS_TIMEOUT_SECONDS)
    webdriver.verify_shell(session_id, deadline)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Analytics",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//section[contains(@class, 'recent-review-panel')]"
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    webdriver.progress("Navigating to Flashcards > Decks to verify deck and card persistence")
    deadline = webdriver.deadline_after()
    decks_button = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, decks_button)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Flashcards",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tab' and @aria-selected='true'][contains(normalize-space(.), 'Decks')]",
        deadline,
    )
    deck_button = webdriver.find_element(
        session_id,
        "xpath",
        f"//main//button[.//strong[normalize-space(.)={json.dumps(deck_name)}]]",
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
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Analytics",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//section[contains(@class, 'recent-review-panel')]"
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
    webdriver.click = tutorial_aware_click
    webdriver.verify_persistence_after_restart = verify_persistence_after_restart
    raise SystemExit(webdriver.main())
