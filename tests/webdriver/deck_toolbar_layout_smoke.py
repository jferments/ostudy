#!/usr/bin/env python3
"""Native regressions for the current Flashcards tabs and deck-library controls."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

import smoke as webdriver
import ui_platform_smoke as ui


VIEWPORTS = (
    (390, 760),
    (700, 760),
    (1174, 760),
    (1280, 760),
)
EXPECTED_TABS = ["Decks", "Search/Filter"]


def library_snapshot(session_id: str) -> dict[str, Any] | None:
    measurement = ui.execute(
        session_id,
        """
        const main = document.querySelector('main');
        const tabList = main?.querySelector(
          '[role="tablist"][aria-label="Flashcards workspaces"]'
        );
        const pointerSurface = main?.querySelector(
          '[data-deck-library-pointer-surface="true"]'
        );
        if (!(main instanceof HTMLElement)
            || !(tabList instanceof HTMLElement)
            || !(pointerSurface instanceof HTMLElement)) return null;

        const tabs = [...tabList.querySelectorAll('[role="tab"]')];
        const buttons = [...main.querySelectorAll('button')]
          .filter((button) => button instanceof HTMLButtonElement);
        const buttonLabel = (button) =>
          (button.getAttribute('aria-label') || button.textContent || '').trim();
        const labels = buttons.map(buttonLabel);
        const folderHeading = [...main.querySelectorAll('h2')]
          .find((heading) => heading.textContent?.trim() === 'Folders');
        const unsortedHeading = [...main.querySelectorAll('h2')]
          .find((heading) => heading.textContent?.trim() === 'Unsorted decks');
        const emptyHeading = [...main.querySelectorAll('h2')]
          .find((heading) => heading.textContent?.trim() === 'No decks have been created yet.');
        const folderSection = folderHeading?.closest('section');
        const folderButton = buttons.find((button) => button.getAttribute('aria-label') === 'Create a folder');
        const importButton = buttons.find((button) => button.getAttribute('aria-label') === 'Import deck from file');
        const newDeckButton = buttons.find((button) => button.textContent?.trim() === 'New deck');
        const createDeckButton = buttons.find((button) => button.textContent?.trim() === 'Create a new deck');
        const viewportWidth = document.documentElement.clientWidth;
        const visibleButtons = buttons.filter((button) => {
          const style = getComputedStyle(button);
          const rect = button.getBoundingClientRect();
          return style.display !== 'none'
            && style.visibility !== 'hidden'
            && rect.width > 0
            && rect.height > 0;
        });
        return {
          tabs: tabs.map((tab) => tab.getAttribute('aria-label') || (tab.textContent || '').trim()),
          tabsContained: tabList.scrollWidth <= tabList.clientWidth + 1,
          emptyHeading: Boolean(emptyHeading),
          foldersHeading: Boolean(folderHeading),
          unsortedHeading: Boolean(unsortedHeading),
          importAction: Boolean(importButton),
          createDeckAction: Boolean(createDeckButton),
          newDeckAction: Boolean(newDeckButton),
          createFolderAction: Boolean(folderButton),
          folderButtonInFoldersSection: Boolean(folderSection && folderButton && folderSection.contains(folderButton)),
          newDeckInFoldersSection: Boolean(folderSection && newDeckButton && folderSection.contains(newDeckButton)),
          hasSelectAll: labels.some((label) => label.startsWith('Select all')),
          hasCollapseToggle: labels.includes('Collapse all') || labels.includes('Expand all'),
          staleEmptyCopy:
            main.textContent?.includes('No folders yet.') === true
            || main.textContent?.includes('All active decks are in folders.') === true,
          pointerSurfacePresent: pointerSurface.dataset.deckLibraryPointerSurface === 'true',
          obsoleteDraggableHandleCount: pointerSurface.querySelectorAll(
            '[draggable="true"][aria-label^="Reorder "]'
          ).length,
          visibleButtonsContained: visibleButtons.every((button) => {
            const rect = button.getBoundingClientRect();
            return rect.left >= -1 && rect.right <= viewportWidth + 1;
          }),
          noDocumentOverflow:
            document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1,
        };
        """,
    )
    return measurement if isinstance(measurement, dict) else None


def wait_for_snapshot(
    session_id: str,
    deadline: float,
    predicate: Callable[[dict[str, Any]], bool],
) -> dict[str, Any]:
    last: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        last = library_snapshot(session_id)
        if last is not None and predicate(last):
            return last
        time.sleep(0.1)
    raise AssertionError(f"Deck library did not reach the expected state: {last!r}")


def prepare_decks_workspace(session_id: str, width: int, height: int, deadline: float) -> None:
    ui.set_window_rect(session_id, width, height)
    expected_layout = "compact" if width <= 760 else "medium" if width <= 1180 else "wide"
    ui.wait_for_layout_mode(session_id, expected_layout, deadline)
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks'][aria-current='page']",
        deadline,
    )
    decks_tab = webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tab' and @aria-label='Decks']",
        deadline,
    )
    webdriver.click(session_id, decks_tab)
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tab' and @aria-label='Decks' and @aria-selected='true']",
        deadline,
    )


def assert_common_contract(snapshot: dict[str, Any], width: int) -> None:
    if snapshot.get("tabs") != EXPECTED_TABS:
        raise AssertionError(
            f"Flashcards did not expose exactly Decks and Search/Filter at {width}px: {snapshot!r}"
        )
    for key, message in (
        ("tabsContained", "Flashcards tabs overflowed their tablist"),
        ("importAction", "Import deck from file was not directly available"),
        ("pointerSurfacePresent", "Deck pointer-session surface was unavailable"),
        ("visibleButtonsContained", "A visible deck-library button escaped the viewport"),
        ("noDocumentOverflow", "The deck library introduced horizontal document overflow"),
    ):
        if snapshot.get(key) is not True:
            raise AssertionError(f"{message} at {width}px: {snapshot!r}")
    if int(snapshot.get("obsoleteDraggableHandleCount", -1)) != 0:
        raise AssertionError(f"An obsolete draggable reorder handle remained at {width}px: {snapshot!r}")
    if snapshot.get("staleEmptyCopy") is True:
        raise AssertionError(f"Retired empty-library copy returned at {width}px: {snapshot!r}")


def verify_empty_library(session_id: str, width: int, height: int) -> None:
    deadline = webdriver.deadline_after(45)
    prepare_decks_workspace(session_id, width, height, deadline)
    snapshot = wait_for_snapshot(
        session_id,
        deadline,
        lambda state: state.get("emptyHeading") is True,
    )
    assert_common_contract(snapshot, width)
    expected = {
        "emptyHeading": True,
        "foldersHeading": False,
        "unsortedHeading": False,
        "createDeckAction": True,
        "newDeckAction": False,
        "createFolderAction": False,
        "hasSelectAll": False,
        "hasCollapseToggle": False,
    }
    mismatches = {key: snapshot.get(key) for key, value in expected.items() if snapshot.get(key) != value}
    if mismatches:
        raise AssertionError(
            f"Fresh deck library exposed irrelevant chrome at {width}px: {snapshot!r}"
        )
    ui.assert_no_horizontal_overflow(session_id, f"empty deck library at {width}px")


def create_first_deck(session_id: str) -> None:
    deck_name = "Deck toolbar regression deck"
    deadline = webdriver.deadline_after(45)
    webdriver.click_text_button(session_id, "Create a new deck", deadline)
    webdriver.set_input_text(
        session_id,
        "css selector",
        "dialog input[data-autofocus='true']",
        deck_name,
        deadline,
    )
    create = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[normalize-space(.)='Create deck']",
        deadline,
    )
    webdriver.click(session_id, create)
    webdriver.wait_for_main_heading(session_id, deck_name, deadline)
    ui.wait_for_overlay_inactive(session_id, deadline, "deck toolbar deck creation")

    # Deck creation can legitimately consume most of the bounded mutation phase.
    # Navigation back to the library is a separate read/navigation phase, so give
    # it a fresh deadline instead of reusing an almost-expired absolute timestamp.
    navigation_deadline = webdriver.deadline_after()
    webdriver.click_text_button(session_id, "← All decks", navigation_deadline)
    webdriver.wait_for_main_heading(session_id, "Flashcards", navigation_deadline)


def verify_populated_library(session_id: str, width: int, height: int) -> None:
    deadline = webdriver.deadline_after(45)
    prepare_decks_workspace(session_id, width, height, deadline)
    snapshot = wait_for_snapshot(
        session_id,
        deadline,
        lambda state: state.get("foldersHeading") is True and state.get("unsortedHeading") is True,
    )
    assert_common_contract(snapshot, width)
    expected = {
        "emptyHeading": False,
        "foldersHeading": True,
        "unsortedHeading": True,
        "createDeckAction": False,
        "newDeckAction": True,
        "createFolderAction": True,
        "folderButtonInFoldersSection": True,
        "newDeckInFoldersSection": False,
        "hasSelectAll": True,
        "hasCollapseToggle": False,
    }
    mismatches = {key: snapshot.get(key) for key, value in expected.items() if snapshot.get(key) != value}
    if mismatches:
        raise AssertionError(
            f"Populated deck library controls did not match the cleanup contract at {width}px: {snapshot!r}"
        )
    ui.assert_no_horizontal_overflow(session_id, f"populated deck library at {width}px")


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

        for width, height in VIEWPORTS:
            verify_empty_library(session_id, width, height)

        create_first_deck(session_id)

        for width, height in VIEWPORTS:
            verify_populated_library(session_id, width, height)

        print(
            "Native Flashcards layout smoke passed: Flashcards exposes exactly two contained tabs; "
            "the fresh library hides folder/unsorted/selection/collapse chrome while keeping import "
            "and the create-deck CTA; after deck creation the folder plus, Unsorted section, "
            "Select all, and top New deck action appear without horizontal overflow."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver Flashcards layout log ---")
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
