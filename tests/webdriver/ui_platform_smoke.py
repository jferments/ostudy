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
    raise AssertionError(
        f"Dialog focus was not restored to {expected}: {last_label!r}"
    )


def wait_for_layout_mode(session_id: str, expected: str, deadline: float) -> None:
    last = ""
    while time.monotonic() < deadline:
        last = str(
            execute(
                session_id,
                "return document.documentElement.dataset.layoutMode || '';",
            )
        )
        if last == expected:
            return
        time.sleep(0.05)
    raise AssertionError(f"Expected layout mode {expected!r}, received {last!r}")


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


def assert_flashcards_tabs(session_id: str, deadline: float) -> None:
    for label in ("Decks", "Templates", "Search", "Import/Export"):
        find_workspace_tab(session_id, label, deadline)
    removed = execute(
        session_id,
        """
        return [...document.querySelectorAll("[role='tablist'][aria-label='Flashcards workspaces'] [role='tab']")]
          .filter((tab) => (tab.textContent || '').includes('Create Cards')).length;
        """,
    )
    if int(removed) != 0:
        raise AssertionError("The removed Create Cards workspace is still visible")


def assert_navigation_labels(
    session_id: str,
    expected: list[str],
    label: str,
) -> None:
    labels = execute(
        session_id,
        """
        return [...document.querySelectorAll("nav[aria-label='Main navigation'] button")]
          .map((button) => (button.textContent || '').trim());
        """,
    )
    if labels != expected:
        raise AssertionError(f"Unexpected {label} navigation order: {labels!r}")


def open_study_guides_and_return(
    session_id: str,
    deadline: float,
    label: str,
) -> None:
    study_guides = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-study-guides']",
        deadline,
    )
    webdriver.click(session_id, study_guides)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Study Guides",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, label)

    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(
        session_id,
        "css selector",
        "main [role='tablist'][aria-label='Flashcards workspaces']",
        deadline,
    )


def verify_compact_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 390, 700)
    wait_for_layout_mode(session_id, "compact", deadline)
    assert_navigation_labels(
        session_id,
        ["Flashcards", "Study Guides", "Analytics", "More"],
        "compact",
    )
    open_study_guides_and_return(
        session_id,
        deadline,
        "compact Study Guides destination",
    )
    assert_no_horizontal_overflow(session_id, "compact navigation")

    more = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-more']",
        deadline,
    )
    webdriver.click(session_id, more)
    webdriver.find_element(
        session_id,
        "xpath",
        "//*[@role='dialog' and @aria-modal='true']//*[normalize-space(.)='More']",
        deadline,
    )
    more_destinations = execute(
        session_id,
        """
        return [...document.querySelectorAll("[role=dialog] button[data-testid^='nav-']")]
          .map((button) => (button.textContent || '').trim());
        """,
    )
    if more_destinations != ["Documents", "Settings"]:
        raise AssertionError(
            f"Compact More destinations were incomplete, duplicated, or out of order: {more_destinations!r}"
        )

    settings = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
        deadline,
    )
    webdriver.click(session_id, settings)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Settings", deadline)
    wait_for_overlay_inactive(session_id, deadline, "compact Settings assertions")
    assert_no_horizontal_overflow(session_id, "compact Settings screen")

    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(session_id, "css selector", "main [role='tablist'][aria-label='Flashcards workspaces']", deadline)
    assert_flashcards_tabs(session_id, deadline)
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
    set_window_rect(session_id, 1440, 900)
    wait_for_layout_mode(session_id, "wide", deadline)
    assert_navigation_labels(
        session_id,
        ["Flashcards", "Study Guides", "Documents", "Analytics", "Settings"],
        "wide",
    )
    open_study_guides_and_return(
        session_id,
        deadline,
        "wide Study Guides destination",
    )
    more_count = execute(
        session_id,
        "return document.querySelectorAll(\"button[data-testid='nav-more']\").length;",
    )
    if int(more_count) != 0:
        raise AssertionError("Wide navigation displayed the compact More destination")
    assert_no_horizontal_overflow(session_id, "wide application shell")


def verify_medium_flashcards_layout(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1000, 800)
    wait_for_layout_mode(session_id, "medium", deadline)
    assert_navigation_labels(
        session_id,
        ["Flashcards", "Study Guides", "Documents", "Analytics", "Settings"],
        "medium",
    )
    open_study_guides_and_return(
        session_id,
        deadline,
        "medium Study Guides destination",
    )
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks'][aria-current='page']",
        deadline,
    )
    webdriver.find_element(session_id, "css selector", "main [role='tablist'][aria-label='Flashcards workspaces']", deadline)
    assert_flashcards_tabs(session_id, deadline)

    navigation = execute(
        session_id,
        """
        const sidebar = document.querySelector("nav[aria-label='Main navigation']")?.closest("aside");
        const labels = [...document.querySelectorAll("nav[aria-label='Main navigation'] button span:last-child")];
        return {
          sidebarWidth: sidebar instanceof HTMLElement ? sidebar.getBoundingClientRect().width : 0,
          visibleLabels: labels.filter((label) => {
            if (!(label instanceof HTMLElement)) return false;
            const style = getComputedStyle(label);
            return style.display !== 'none' && style.visibility !== 'hidden' && label.getBoundingClientRect().width > 20;
          }).map((label) => (label.textContent || '').trim())
        };
        """,
    )
    if not isinstance(navigation, dict):
        raise AssertionError(f"Could not measure medium navigation: {navigation!r}")
    if float(navigation.get("sidebarWidth", 0)) < 150:
        raise AssertionError(f"Medium navigation collapsed into an icon rail: {navigation!r}")
    if "Flashcards" not in navigation.get("visibleLabels", []):
        raise AssertionError(f"Medium navigation labels are not visible: {navigation!r}")
    assert_no_horizontal_overflow(session_id, "medium Flashcards workspace")

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


def verify_deck_scoped_authoring(session_id: str, deadline: float) -> None:
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

    webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Add flashcard']",
        deadline,
    )
    bulk_add = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Bulk add']",
        deadline,
    )
    webdriver.click(session_id, bulk_add)
    webdriver.find_element(
        session_id,
        "css selector",
        "[role='table'][aria-label='Bulk cards']",
        deadline,
    )

    measurements = execute(
        session_id,
        """
        const table = document.querySelector("[role='table'][aria-label='Bulk cards']");
        const layout = table?.closest("[aria-busy]");
        const destination = layout
          ? [...layout.querySelectorAll("div")].find((candidate) => {
              const span = candidate.querySelector(":scope > span");
              const strong = candidate.querySelector(":scope > strong");
              return span?.textContent?.trim() === "Destination deck" && strong;
            })
          : null;
        const controls = destination?.parentElement;
        const help = controls?.querySelector(":scope > p");
        const select = controls?.querySelector(":scope > label select");
        if (!(controls instanceof HTMLElement) || !(destination instanceof HTMLElement) || !(help instanceof HTMLElement)) {
          return null;
        }
        const c = controls.getBoundingClientRect();
        const d = destination.getBoundingClientRect();
        const h = help.getBoundingClientRect();
        return {
          controlsLeft: c.left,
          controlsRight: c.right,
          destinationLeft: d.left,
          destinationRight: d.right,
          helpLeft: h.left,
          helpRight: h.right,
          selectCount: select ? 1 : 0,
          destinationText: (destination.textContent || '').trim()
        };
        """,
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure deck-scoped Bulk Add controls: {measurements!r}")
    if int(measurements["selectCount"]) != 0:
        raise AssertionError("Deck-scoped Bulk Add exposed a destination selector")
    if deck_name not in str(measurements["destinationText"]):
        raise AssertionError(f"Bulk Add did not preserve its deck destination: {measurements!r}")
    controls_right = float(measurements["controlsRight"])
    if (
        float(measurements["destinationRight"]) > controls_right + 1
        or float(measurements["helpRight"]) > controls_right + 1
    ):
        raise AssertionError(f"Bulk Add controls escaped their container: {measurements!r}")
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
        verify_deck_scoped_authoring(session_id, webdriver.deadline_after(45))
        print(
            "Native UI platform smoke passed: compact, labeled medium, and wide navigation; "
            "four-workspace Flashcards containment; deck-scoped authoring; no horizontal overflow; "
            "dialog focus containment; Escape dismissal; and focus restoration."
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
