#!/usr/bin/env python3
"""Native regression for the deck-library action row reported in manual testing."""

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


def verify_toolbar(session_id: str, width: int, height: int, deadline: float) -> None:
    ui.set_window_rect(session_id, width, height)
    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    webdriver.find_element(session_id, "css selector", "main .flashcards-workspace", deadline)
    decks = ui.find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, decks)
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
        const folder = arguments[0];
        const deck = arguments[1];
        const actions = folder.parentElement;
        if (!(folder instanceof HTMLElement) || !(deck instanceof HTMLElement) || !(actions instanceof HTMLElement)) {
          return null;
        }
        const f = folder.getBoundingClientRect();
        const d = deck.getBoundingClientRect();
        const a = actions.getBoundingClientRect();
        const style = getComputedStyle(actions);
        return {
          folderTop: f.top,
          folderBottom: f.bottom,
          deckTop: d.top,
          deckBottom: d.bottom,
          actionsLeft: a.left,
          actionsRight: a.right,
          actionsClient: actions.clientWidth,
          actionsScroll: actions.scrollWidth,
          flexWrap: style.flexWrap,
        };
        """,
        [
            {webdriver.ELEMENT_KEY: new_folder},
            {webdriver.ELEMENT_KEY: new_deck},
        ],
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure deck toolbar at {width}px: {measurements!r}")
    if abs(float(measurements["folderTop"]) - float(measurements["deckTop"])) > 2:
        raise AssertionError(
            f"New deck wrapped away from New folder at {width}px: {measurements!r}"
        )
    if int(measurements["actionsScroll"]) > int(measurements["actionsClient"]) + 1:
        raise AssertionError(
            f"Deck toolbar action row overflowed at {width}px: {measurements!r}"
        )
    ui.assert_no_horizontal_overflow(session_id, f"deck toolbar at {width}px")


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
        # 1174px reproduces the manually reported screenshot dimensions closely;
        # 1280px covers the new default window width.
        verify_toolbar(session_id, 1174, 760, webdriver.deadline_after(45))
        verify_toolbar(session_id, 1280, 760, webdriver.deadline_after(45))
        print(
            "Native deck-toolbar smoke passed: New folder and New deck stay on one action row "
            "at the reported medium width and at the default desktop width."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver deck-toolbar log ---")
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
