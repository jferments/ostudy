#!/usr/bin/env python3
"""Native Stage 3.13 responsive/overlay smoke using the W3C WebDriver protocol."""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

import smoke as webdriver

ESCAPE_KEY = "\ue00c"
TAB_KEY = "\ue004"


def set_window_rect(session_id: str, width: int, height: int) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/window/rect",
        {"width": width, "height": height},
    )


def execute(session_id: str, script: str, args: list[Any] | None = None) -> Any:
    return webdriver.request(
        "POST",
        f"/session/{session_id}/execute/sync",
        {"script": script, "args": args or []},
    )


def send_key(session_id: str, value: str) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/actions",
        {
            "actions": [
                {
                    "type": "key",
                    "id": "keyboard",
                    "actions": [
                        {"type": "keyDown", "value": value},
                        {"type": "keyUp", "value": value},
                    ],
                }
            ]
        },
    )
    try:
        webdriver.request("DELETE", f"/session/{session_id}/actions")
    except Exception:
        # Some WebDriver builds release actions automatically after the sequence.
        pass


def wait_for_absent(
    session_id: str,
    using: str,
    value: str,
    deadline: float,
) -> None:
    while time.monotonic() < deadline:
        result = webdriver.request(
            "POST",
            f"/session/{session_id}/elements",
            {"using": using, "value": value},
        )
        if isinstance(result, list) and len(result) == 0:
            return
        time.sleep(0.1)
    raise TimeoutError(f"Element remained present: {using}={value}")


def wait_for_focus(session_id: str, element_id: str, deadline: float) -> None:
    """Wait for React's queued post-showModal initial-focus step to settle."""
    while time.monotonic() < deadline:
        focused = execute(
            session_id,
            "return document.activeElement === arguments[0];",
            [{webdriver.ELEMENT_KEY: element_id}],
        )
        if focused is True:
            return
        time.sleep(0.05)
    raise AssertionError("Create-deck dialog did not place initial focus in the name input")


def wait_for_active_label(session_id: str, expected: str, deadline: float) -> None:
    """Wait for the queued overlay-unmount focus restoration to complete."""
    last_label = ""
    while time.monotonic() < deadline:
        last_label = str(
            execute(
                session_id,
                "return document.activeElement ? (document.activeElement.getAttribute('aria-label') || document.activeElement.textContent || '').trim() : '';",
            )
        )
        if expected in last_label:
            return
        time.sleep(0.05)
    raise AssertionError(
        f"Dialog focus was not restored to {expected}: {last_label!r}"
    )


def assert_no_horizontal_overflow(session_id: str, label: str) -> None:
    measurements = execute(
        session_id,
        """
        return {
          rootClient: document.documentElement.clientWidth,
          rootScroll: document.documentElement.scrollWidth,
          bodyClient: document.body.clientWidth,
          bodyScroll: document.body.scrollWidth
        };
        """,
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Unexpected overflow measurement for {label}: {measurements!r}")
    root_client = int(measurements.get("rootClient", 0))
    root_scroll = int(measurements.get("rootScroll", 0))
    body_client = int(measurements.get("bodyClient", 0))
    body_scroll = int(measurements.get("bodyScroll", 0))
    if root_scroll > root_client + 1 or body_scroll > body_client + 1:
        raise AssertionError(
            f"Horizontal overflow in {label}: root={root_scroll}/{root_client}, "
            f"body={body_scroll}/{body_client}"
        )


def verify_compact_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 390, 700)
    more = webdriver.find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[normalize-space(.)='•••More' or contains(normalize-space(.), 'More')]",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "compact navigation")

    webdriver.click(session_id, more)
    webdriver.find_element(
        session_id,
        "xpath",
        "//*[@role='dialog' and @aria-modal='true']//*[normalize-space(.)='More']",
        deadline,
    )
    removed_destinations = execute(
        session_id,
        """
        const labels = [...document.querySelectorAll('[role=dialog] button')]
          .map((button) => (button.textContent || '').trim());
        return labels.filter((label) => label.includes('Templates') || label.includes('Search')).length;
        """,
    )
    if int(removed_destinations) != 0:
        raise AssertionError("Compact More sheet still exposed Search/Filter or Templates as top-level destinations")

    settings = webdriver.find_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[contains(normalize-space(.), 'Settings')]",
        deadline,
    )
    webdriver.click(session_id, settings)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Settings",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "compact Settings screen")

    decks = webdriver.find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[contains(normalize-space(.), 'Decks')]",
        deadline,
    )
    webdriver.click(session_id, decks)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Decks", deadline)
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Manage templates']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[normalize-space(.)='Search and filter cards']",
        deadline,
    )


def verify_dialog_focus(session_id: str, deadline: float) -> None:
    new_deck = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='New deck']",
        deadline,
    )
    webdriver.click(session_id, new_deck)
    name_input = webdriver.find_element(
        session_id,
        "css selector",
        "input[placeholder='NCLEX Pharmacology']",
        deadline,
    )
    wait_for_focus(session_id, name_input, deadline)

    send_key(session_id, TAB_KEY)
    inside_dialog = execute(
        session_id,
        "return Boolean(document.activeElement && document.activeElement.closest('[role=dialog]'));",
    )
    if inside_dialog is not True:
        raise AssertionError("Tab moved focus outside the active dialog")

    send_key(session_id, ESCAPE_KEY)
    wait_for_absent(session_id, "css selector", "[role='dialog']", deadline)
    wait_for_active_label(session_id, "New deck", deadline)


def verify_wide_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1100, 720)
    webdriver.find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[contains(normalize-space(.), 'Settings')]",
        deadline,
    )
    more_count = execute(
        session_id,
        "return [...document.querySelectorAll(\"nav[aria-label='Main navigation'] button\")].filter((button) => button.textContent.includes('More')).length;",
    )
    if int(more_count) != 0:
        raise AssertionError("Wide navigation still displayed the compact More destination")
    removed_count = execute(
        session_id,
        """
        return [...document.querySelectorAll("nav[aria-label='Main navigation'] button")]
          .filter((button) => {
            const label = button.textContent || '';
            return label.includes('Templates') || label.includes('Search/Filter');
          }).length;
        """,
    )
    if int(removed_count) != 0:
        raise AssertionError("Wide navigation still exposed Search/Filter or Templates as top-level destinations")
    assert_no_horizontal_overflow(session_id, "wide application shell")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True, type=Path)
    args = parser.parse_args()
    application = args.application.resolve()
    if not application.is_file():
        raise FileNotFoundError(application)

    session_id: str | None = None
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False) as driver_log:
        log_path = Path(driver_log.name)
        driver = subprocess.Popen(
            ["tauri-driver"],
            stdout=driver_log,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )

    try:
        webdriver.wait_for_driver(time.monotonic() + 20)
        session_id = webdriver.create_session(application)
        deadline = time.monotonic() + 60
        webdriver.verify_shell(session_id, deadline)
        verify_compact_navigation(session_id, deadline)
        verify_dialog_focus(session_id, deadline)
        verify_wide_navigation(session_id, deadline)
        print(
            "Native UI platform smoke passed: compact/wide navigation, Decks workspace "
            "destinations, no horizontal overflow, dialog focus containment, Escape "
            "dismissal, and focus restoration."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver UI-platform log ---")
        try:
            print(log_path.read_text(encoding="utf-8"))
        except OSError:
            pass
        if session_id:
            try:
                source = webdriver.request("GET", f"/session/{session_id}/source")
                print("\n--- current WebView source (truncated) ---")
                print(str(source)[:12_000])
            except Exception:
                pass
        raise
    finally:
        if session_id:
            try:
                webdriver.delete_session(session_id)
            except Exception:
                pass
        try:
            os.killpg(driver.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            driver.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(driver.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            driver.wait(timeout=5)
        try:
            log_path.unlink()
        except OSError:
            pass


if __name__ == "__main__":
    raise SystemExit(main())
