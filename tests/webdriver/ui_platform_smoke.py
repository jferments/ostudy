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
    """Require top-layer teardown before interacting with background controls."""
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


def find_workspace_tab(session_id: str, label: str, deadline: float) -> str:
    return webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
        f"//button[@role='tab' and normalize-space(.)={json.dumps(label)}]",
        deadline,
    )


def verify_compact_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 390, 700)
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
        raise AssertionError("Compact More sheet still exposed Flashcards workspaces as top-level destinations")

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
    find_workspace_tab(session_id, "Decks", deadline)
    find_workspace_tab(session_id, "Templates", deadline)
    find_workspace_tab(session_id, "Search", deadline)
    find_workspace_tab(session_id, "Import/Export", deadline)
    workspace_labels = execute(
        session_id,
        """
        return [...document.querySelectorAll(
          'main [role=tablist][aria-label="Flashcards workspaces"] button[role=tab]'
        )].map((button) => button.getAttribute('aria-label'));
        """,
    )
    if workspace_labels != ["Decks", "Templates", "Search", "Import/Export"]:
        raise AssertionError(f"Unexpected compact Flashcards workspaces: {workspace_labels!r}")
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
    set_window_rect(session_id, 1100, 720)
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
        raise AssertionError("Wide navigation still displayed the compact More destination")
    removed_count = execute(
        session_id,
        """
        return [...document.querySelectorAll("nav[aria-label='Main navigation'] button")]
          .filter((button) => {
            const label = button.textContent || '';
            return label.includes('Templates') || label.includes('Search/Filter') || label.includes('Create Cards');
          }).length;
        """,
    )
    if int(removed_count) != 0:
        raise AssertionError("Wide navigation exposed Flashcards workspaces as top-level destinations")
    assert_no_horizontal_overflow(session_id, "wide application shell")


def verify_medium_flashcards_layout(session_id: str, deadline: float) -> None:
    """Regress medium-desktop overflow with the full sidebar consuming workspace width."""
    set_window_rect(session_id, 1174, 760)
    # The preceding compact-navigation scenario already returns to the
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

    deck_tab = find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, deck_tab)
    webdriver.find_element(
        session_id,
        "css selector",
        "main [aria-label='Deck library controls']",
        deadline,
    )
    workspace = execute(
        session_id,
        """
        const root = document.querySelector('main .flashcards-workspace');
        const tabList = document.querySelector(
          'main [role="tablist"][aria-label="Flashcards workspaces"]'
        );
        const toolbar = document.querySelector("main [aria-label='Deck library controls']");
        if (!(root instanceof HTMLElement) || !(tabList instanceof HTMLElement)
            || !(toolbar instanceof HTMLElement)) return null;
        const tabs = [...tabList.querySelectorAll('[role="tab"]')];
        const rootRect = root.getBoundingClientRect();
        const toolbarRect = toolbar.getBoundingClientRect();
        return {
          labels: tabs.map((tab) => tab.getAttribute('aria-label')),
          rootClient: root.clientWidth,
          rootScroll: root.scrollWidth,
          toolbarLeft: toolbarRect.left,
          toolbarRight: toolbarRect.right,
          rootLeft: rootRect.left,
          rootRight: rootRect.right,
        };
        """,
    )
    if not isinstance(workspace, dict):
        raise AssertionError("Could not measure the Decks workspace at medium desktop width")
    if workspace.get("labels") != ["Decks", "Templates", "Search", "Import/Export"]:
        raise AssertionError(f"Unexpected medium Flashcards workspaces: {workspace!r}")
    if (
        int(workspace["rootScroll"]) > int(workspace["rootClient"]) + 1
        or float(workspace["toolbarLeft"]) < float(workspace["rootLeft"]) - 1
        or float(workspace["toolbarRight"]) > float(workspace["rootRight"]) + 1
    ):
        raise AssertionError(f"Deck library controls escaped the medium workspace: {workspace!r}")
    assert_no_horizontal_overflow(session_id, "medium Decks workspace")

    search = find_workspace_tab(session_id, "Search", deadline)
    webdriver.click(session_id, search)
    webdriver.find_element(
        session_id,
        "css selector",
        "main section[aria-label='Card filters']",
        deadline,
    )
    filter_bar = execute(
        session_id,
        """
        const section = document.querySelector("section[aria-label='Card filters']");
        const bar = section?.querySelector('[data-surface-level]');
        if (!(bar instanceof HTMLElement)) return null;
        const rect = bar.getBoundingClientRect();
        const children = [...bar.children].filter((child) => child instanceof HTMLElement);
        const childRects = children.map((child) => child.getBoundingClientRect());
        return {
          left: rect.left,
          right: rect.right,
          client: bar.clientWidth,
          scroll: bar.scrollWidth,
          minChildLeft: childRects.length ? Math.min(...childRects.map((child) => child.left)) : rect.left,
          maxChildRight: childRects.length ? Math.max(...childRects.map((child) => child.right)) : rect.right
        };
        """,
    )
    if not isinstance(filter_bar, dict):
        raise AssertionError("Could not measure the Search filter bar at medium desktop width")
    if (
        float(filter_bar["minChildLeft"]) < float(filter_bar["left"]) - 1
        or float(filter_bar["maxChildRight"]) > float(filter_bar["right"]) + 1
        or int(filter_bar["scroll"]) > int(filter_bar["client"]) + 1
    ):
        raise AssertionError(f"Search controls escaped the filter bar: {filter_bar!r}")
    assert_no_horizontal_overflow(session_id, "medium Search workspace")

    decks = find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, decks)


def verify_bulk_add_containment(session_id: str, deadline: float) -> None:
    """Regress the native-select intrinsic-width overlap shown in Bulk Add."""
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

    # Bulk Add is a deck-local authoring action after Phase 1. Stay on the
    # newly created deck detail rather than navigating to the removed Create Cards workspace.

    bulk_add = webdriver.find_element(
        session_id,
        "css selector",
        "main button[data-testid='bulk-add-cards']",
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
        const controls = document.querySelector('.bulk-entry-controls');
        const destination = controls?.querySelector('.bulk-entry-fixed-destination');
        const help = controls?.querySelector('p');
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
          controlsClient: controls.clientWidth,
          controlsScroll: controls.scrollWidth
        };
        """,
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure deck-local Bulk Add controls: {measurements!r}")

    controls_left = float(measurements["controlsLeft"])
    controls_right = float(measurements["controlsRight"])
    destination_left = float(measurements["destinationLeft"])
    destination_right = float(measurements["destinationRight"])
    help_left = float(measurements["helpLeft"])
    help_right = float(measurements["helpRight"])
    if (
        destination_left < controls_left - 1
        or destination_right > controls_right + 1
        or help_left < controls_left - 1
        or help_right > controls_right + 1
        or int(measurements["controlsScroll"]) > int(measurements["controlsClient"]) + 1
    ):
        raise AssertionError(
            f"Deck-local Bulk Add controls escaped their container: {measurements!r}"
        )
    assert_no_horizontal_overflow(session_id, "Bulk Add dialog")

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
        verify_bulk_add_containment(session_id, webdriver.deadline_after(45))
        print(
            "Native UI platform smoke passed: compact/wide navigation, medium-width Flashcards "
            "containment, no horizontal overflow, Bulk Add control containment, dialog focus "
            "containment, Escape dismissal, and focus restoration."
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
