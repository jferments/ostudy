#!/usr/bin/env python3
"""Native regressions for Flashcards tabs and deck-library action containment."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path

import smoke as webdriver
import ui_platform_smoke as ui


def verify_compact_tabs(session_id: str, width: int) -> None:
    if width > 760:
        return
    measurements = ui.execute(
        session_id,
        """
        const list = document.querySelector(
          '[role="tablist"][aria-label="Flashcards workspaces"]'
        );
        if (!(list instanceof HTMLElement)) return null;
        const tabs = Array.from(list.querySelectorAll('[role="tab"]'));
        if (tabs.length !== 5 || tabs.some((tab) => !(tab instanceof HTMLElement))) {
          return null;
        }
        const rects = tabs.map((tab) => tab.getBoundingClientRect());
        const listRect = list.getBoundingClientRect();
        return {
          count: rects.length,
          firstRowTop: rects[0].top,
          firstRowTop2: rects[1].top,
          firstRowTop3: rects[2].top,
          secondRowTop: rects[3].top,
          secondRowTop2: rects[4].top,
          secondRowCenter: (rects[3].left + rects[4].right) / 2,
          listCenter: (listRect.left + listRect.right) / 2,
          clientWidth: list.clientWidth,
          scrollWidth: list.scrollWidth,
        };
        """,
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure compact Flashcards tabs at {width}px")
    first_top = float(measurements["firstRowTop"])
    if any(
        abs(float(measurements[key]) - first_top) > 2
        for key in ("firstRowTop2", "firstRowTop3")
    ):
        raise AssertionError(
            f"The first three Flashcards tabs did not share one row at {width}px: {measurements!r}"
        )
    second_top = float(measurements["secondRowTop"])
    if abs(float(measurements["secondRowTop2"]) - second_top) > 2 or second_top <= first_top + 2:
        raise AssertionError(
            f"The final two Flashcards tabs did not form a second row at {width}px: {measurements!r}"
        )
    if abs(float(measurements["secondRowCenter"]) - float(measurements["listCenter"])) > 3:
        raise AssertionError(
            f"The second Flashcards tab row was not centered at {width}px: {measurements!r}"
        )
    if int(measurements["scrollWidth"]) > int(measurements["clientWidth"]) + 1:
        raise AssertionError(
            f"The Flashcards tabs still required horizontal scrolling at {width}px: {measurements!r}"
        )


def verify_toolbar(session_id: str, width: int, height: int, deadline: float) -> None:
    ui.set_window_rect(session_id, width, height)
    # Each viewport check begins and ends on Flashcards > Decks. Resolve that
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
        session_id,
        "xpath",
        "//main//h2[normalize-space(.)='Folders']",
        deadline,
    )
    new_folder = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='New folder']",
        deadline,
    )
    new_deck = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='New deck']",
        deadline,
    )
    measurements = ui.execute(
        session_id,
        """
        const heading = arguments[0];
        const folder = arguments[1];
        const deck = arguments[2];
        const folderSection = heading.closest('section');
        const folderActions = folder.parentElement;
        const deckActions = deck.parentElement;
        if (
          !(heading instanceof HTMLElement) ||
          !(folder instanceof HTMLElement) ||
          !(deck instanceof HTMLElement) ||
          !(folderSection instanceof HTMLElement) ||
          !(folderActions instanceof HTMLElement) ||
          !(deckActions instanceof HTMLElement)
        ) {
          return null;
        }

        // HelpTrigger renders an absolutely positioned, opacity-zero tooltip inside
        // these groups. That tooltip is intentionally wider than its icon and is
        // included in an ancestor's scrollWidth even when no visible control
        // overflows. Measure the interactive controls themselves instead.
        const controlMetrics = (group) => {
          const groupRect = group.getBoundingClientRect();
          const controls = [...group.querySelectorAll('button')]
            .filter((control) => control instanceof HTMLElement);
          const rects = controls.map((control) => control.getBoundingClientRect());
          return {
            left: groupRect.left,
            right: groupRect.right,
            controlCount: controls.length,
            minControlLeft: rects.length
              ? Math.min(...rects.map((rect) => rect.left))
              : groupRect.left,
            maxControlRight: rects.length
              ? Math.max(...rects.map((rect) => rect.right))
              : groupRect.right,
            clippedLabelCount: controls.filter(
              (control) => control.scrollWidth > control.clientWidth + 1
            ).length,
          };
        };

        const f = folder.getBoundingClientRect();
        const d = deck.getBoundingClientRect();
        const viewport = document.documentElement.clientWidth;
        return {
          folderInFoldersSection: folderSection.contains(folder),
          deckInFoldersSection: folderSection.contains(deck),
          folderLeft: f.left,
          folderRight: f.right,
          deckLeft: d.left,
          deckRight: d.right,
          viewport,
          folderActions: controlMetrics(folderActions),
          deckActions: controlMetrics(deckActions),
        };
        """,
        [
            {webdriver.ELEMENT_KEY: folders_heading},
            {webdriver.ELEMENT_KEY: new_folder},
            {webdriver.ELEMENT_KEY: new_deck},
        ],
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure deck-library controls at {width}px: {measurements!r}")
    if measurements["folderInFoldersSection"] is not True:
        raise AssertionError(
            f"New folder was not placed inside the Folders section at {width}px: {measurements!r}"
        )
    if measurements["deckInFoldersSection"] is True:
        raise AssertionError(
            f"New deck was incorrectly moved into the Folders section at {width}px: {measurements!r}"
        )
    if (
        float(measurements["folderLeft"]) < -1
        or float(measurements["folderRight"]) > float(measurements["viewport"]) + 1
        or float(measurements["deckLeft"]) < -1
        or float(measurements["deckRight"]) > float(measurements["viewport"]) + 1
    ):
        raise AssertionError(
            f"Deck-library controls escaped the viewport at {width}px: {measurements!r}"
        )

    for key, label in (
        ("folderActions", "Folders action group"),
        ("deckActions", "Deck toolbar action group"),
    ):
        group = measurements[key]
        if not isinstance(group, dict) or int(group.get("controlCount", 0)) == 0:
            raise AssertionError(
                f"{label} had no measurable controls at {width}px: {measurements!r}"
            )
        if (
            float(group["minControlLeft"]) < float(group["left"]) - 1
            or float(group["maxControlRight"]) > float(group["right"]) + 1
        ):
            raise AssertionError(
                f"{label} controls escaped their container at {width}px: {measurements!r}"
            )
        if int(group["clippedLabelCount"]) != 0:
            raise AssertionError(
                f"{label} clipped one or more button labels at {width}px: {measurements!r}"
            )

    ui.assert_no_horizontal_overflow(session_id, f"deck-library controls at {width}px")


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
        verify_toolbar(session_id, 390, 760, webdriver.deadline_after(45))
        verify_toolbar(session_id, 700, 760, webdriver.deadline_after(45))
        # 1174px reproduces the manually reported screenshot dimensions closely;
        # 1280px covers the default desktop window width.
        verify_toolbar(session_id, 1174, 760, webdriver.deadline_after(45))
        verify_toolbar(session_id, 1280, 760, webdriver.deadline_after(45))
        print(
            "Native Flashcards layout smoke passed: compact tabs wrap into a centered 3+2 "
            "layout without horizontal scrolling, New folder stays inside the Folders section, "
            "New deck remains in the top toolbar, and visible action controls stay contained "
            "without clipped labels."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver Flashcards layout log ---")
        try:
            print(log_path.read_text(encoding="utf-8"))
        except OSError:
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
