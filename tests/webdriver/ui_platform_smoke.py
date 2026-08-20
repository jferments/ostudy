#!/usr/bin/env python3
"""Native responsive/overlay smoke using the W3C WebDriver protocol."""

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
        pass


def wait_for_absent(session_id: str, using: str, value: str, deadline: float) -> None:
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


def overlay_snapshot(session_id: str) -> dict[str, Any]:
    snapshot = execute(
        session_id,
        """
        const dialogs = [...document.querySelectorAll("dialog[open], [role='dialog'][aria-modal='true']")]
          .map((dialog) => ({
            label: (dialog.getAttribute('aria-label') || dialog.textContent || '').trim().slice(0, 160),
            tag: dialog.tagName,
          }));
        const root = document.getElementById('root');
        return {
          overlayActive: document.documentElement.dataset.overlayActive === 'true',
          rootInert: Boolean(root?.hasAttribute('inert')),
          dialogCount: dialogs.length,
          dialogs,
        };
        """,
    )
    if not isinstance(snapshot, dict):
        raise AssertionError(f"Unexpected overlay snapshot: {snapshot!r}")
    return snapshot


def wait_for_overlay_inactive(session_id: str, deadline: float, label: str) -> None:
    last: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        last = overlay_snapshot(session_id)
        if (
            last.get("overlayActive") is False
            and last.get("rootInert") is False
            and int(last.get("dialogCount", 0)) == 0
        ):
            return
        time.sleep(0.05)
    raise AssertionError(f"Overlay remained active before {label}: {last!r}")


def wait_for_focus(session_id: str, element_id: str, deadline: float) -> None:
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
    raise AssertionError(f"Dialog focus was not restored to {expected}: {last_label!r}")


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


def find_workspace_tab(session_id: str, label: str, deadline: float) -> str:
    return webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
        f"//button[@role='tab' and normalize-space(.)={json.dumps(label)}]",
        deadline,
    )


def assert_layout_mode(session_id: str, expected: str) -> None:
    actual = execute(session_id, "return document.documentElement.dataset.layout || '';")
    if actual != expected:
        raise AssertionError(f"Expected {expected} layout mode, got {actual!r}")


def verify_compact_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 390, 700)
    assert_layout_mode(session_id, "compact")
    more = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-more']",
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
        raise AssertionError("Compact More sheet exposed Flashcards sub-workspaces as destinations")

    settings = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
        deadline,
    )
    webdriver.click(session_id, settings)
    webdriver.find_element(session_id, "css selector", "main .settings-screen", deadline)
    wait_for_overlay_inactive(session_id, deadline, "compact Settings assertions")
    assert_no_horizontal_overflow(session_id, "compact Settings screen")

    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)
    for label in ("Decks", "Templates", "Search", "Import/Export"):
        find_workspace_tab(session_id, label, deadline)
    create_count = execute(
        session_id,
        "return [...document.querySelectorAll(\"[role='tablist'][aria-label='Flashcards workspaces'] [role='tab']\")].filter((button) => (button.textContent || '').includes('Create Cards')).length;",
    )
    if int(create_count) != 0:
        raise AssertionError("Removed Create Cards workspace is still present")
    assert_no_horizontal_overflow(session_id, "compact Flashcards workspace")


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
        "dialog input[data-autofocus='true']",
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
    wait_for_overlay_inactive(session_id, deadline, "create-deck focus restoration")
    wait_for_active_label(session_id, "New deck", deadline)


def verify_wide_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1280, 800)
    assert_layout_mode(session_id, "wide")
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    more_count = execute(
        session_id,
        "return [...document.querySelectorAll(\"nav[aria-label='Main navigation'] button\")].filter((button) => button.textContent.includes('More')).length;",
    )
    if int(more_count) != 0:
        raise AssertionError("Wide navigation displayed the compact More destination")
    assert_no_horizontal_overflow(session_id, "wide application shell")


def verify_medium_flashcards_layout(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 900, 760)
    assert_layout_mode(session_id, "medium")
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks'][aria-current='page']",
        deadline,
    )
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)

    label_metrics = execute(
        session_id,
        """
        const nav = document.querySelector("nav[aria-label='Main navigation']");
        if (!(nav instanceof HTMLElement)) return null;
        const labels = [...nav.querySelectorAll('button span:last-child')]
          .filter((span) => span instanceof HTMLElement)
          .map((span) => ({ text: (span.textContent || '').trim(), width: span.getBoundingClientRect().width }));
        return labels;
        """,
    )
    if not isinstance(label_metrics, list) or not label_metrics:
        raise AssertionError(f"Could not measure medium navigation labels: {label_metrics!r}")
    hidden = [item for item in label_metrics if item.get("text") and float(item.get("width", 0)) < 8]
    if hidden:
        raise AssertionError(f"Medium navigation hid text labels: {hidden!r}")

    tab_count = execute(
        session_id,
        "return document.querySelectorAll(\"[role='tablist'][aria-label='Flashcards workspaces'] [role='tab']\").length;",
    )
    if int(tab_count) != 4:
        raise AssertionError(f"Expected four Flashcards tabs at medium width, got {tab_count}")
    assert_no_horizontal_overflow(session_id, "medium Decks workspace")

    search = find_workspace_tab(session_id, "Search", deadline)
    webdriver.click(session_id, search)
    webdriver.find_element(
        session_id,
        "css selector",
        "main section[aria-label='Card filters']",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "medium Search workspace")

    decks = find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, decks)
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)


def verify_deck_scoped_bulk_add(session_id: str, deadline: float) -> None:
    deck_name = "Cardiology pharmacology respiratory physiology comprehensive review collection"
    new_deck = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='New deck']",
        deadline,
    )
    webdriver.click(session_id, new_deck)
    webdriver.set_input_text(
        session_id,
        "css selector",
        "dialog input[data-autofocus='true']",
        deck_name,
        deadline,
    )
    create = webdriver.find_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[normalize-space(.)='Create deck']",
        deadline,
    )
    webdriver.click(session_id, create)
    webdriver.wait_for_main_heading(session_id, deck_name, deadline)
    wait_for_overlay_inactive(session_id, deadline, "new deck detail assertions")

    bulk_add = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Bulk add']",
        deadline,
    )
    add_flashcard = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Add flashcard']",
        deadline,
    )
    if not bulk_add or not add_flashcard:
        raise AssertionError("Deck detail did not expose both card-authoring actions")
    webdriver.click(session_id, bulk_add)
    webdriver.find_element(
        session_id,
        "css selector",
        "[role='table'][aria-label='Bulk cards']",
        deadline,
    )

    destination = execute(
        session_id,
        """
        const controls = document.querySelector('.bulk-entry-controls');
        if (!(controls instanceof HTMLElement)) return null;
        const select = controls.querySelector('select');
        const strong = controls.querySelector('strong');
        return {
          hasSelect: Boolean(select),
          destination: (strong?.textContent || '').trim(),
          client: controls.clientWidth,
          scroll: controls.scrollWidth
        };
        """,
    )
    if not isinstance(destination, dict):
        raise AssertionError(f"Could not inspect deck-scoped Bulk Add controls: {destination!r}")
    if destination.get("hasSelect") is True:
        raise AssertionError("Deck-scoped Bulk Add still asks for a destination deck")
    if destination.get("destination") != deck_name:
        raise AssertionError(f"Bulk Add did not retain the open deck destination: {destination!r}")
    if int(destination.get("scroll", 0)) > int(destination.get("client", 0)) + 1:
        raise AssertionError(f"Bulk Add controls overflowed their container: {destination!r}")
    assert_no_horizontal_overflow(session_id, "deck-scoped Bulk Add dialog")

    send_key(session_id, ESCAPE_KEY)
    wait_for_absent(session_id, "css selector", "[role='dialog']", deadline)
    wait_for_overlay_inactive(session_id, deadline, "Bulk Add dismissal")


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
        webdriver.verify_shell(session_id, webdriver.deadline_after())
        verify_compact_navigation(session_id, webdriver.deadline_after(45))
        verify_dialog_focus(session_id, webdriver.deadline_after())
        verify_wide_navigation(session_id, webdriver.deadline_after())
        verify_medium_flashcards_layout(session_id, webdriver.deadline_after(45))
        verify_deck_scoped_bulk_add(session_id, webdriver.deadline_after(45))
        print(
            "Native UI platform smoke passed: compact/wide navigation, labeled medium navigation, "
            "four Flashcards roots, deck-scoped authoring, no horizontal overflow, Bulk Add fixed "
            "destination containment, dialog focus containment, Escape dismissal, and focus restoration."
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
