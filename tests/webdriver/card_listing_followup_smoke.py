#!/usr/bin/env python3
"""Focused native regression for the shared Deck/Search card browser."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import tempfile
from pathlib import Path
from typing import Any

import smoke as webdriver
import ui_platform_smoke as ui


MEDIUM_WIDTH = 928
MEDIUM_HEIGHT = 900
EXPECTED_NAVIGATION = ["Flashcards", "Documents", "Practice", "Analytics", "Settings"]


def assert_medium_navigation(session_id: str, deadline: float) -> None:
    ui.set_window_rect(session_id, MEDIUM_WIDTH, MEDIUM_HEIGHT)
    ui.wait_for_layout_mode(session_id, "medium", deadline)
    measurement = ui.execute(
        session_id,
        """
        const sidebar = document.querySelector('aside.sidebar');
        const labels = [...document.querySelectorAll("nav[aria-label='Main navigation'] button span:last-child")]
          .filter((label) => label instanceof HTMLElement);
        return {
          sidebarWidth: sidebar instanceof HTMLElement ? sidebar.getBoundingClientRect().width : 0,
          labels: labels.map((label) => (label.textContent || '').trim()),
          clipped: labels
            .filter((label) => label.scrollWidth > label.clientWidth + 1)
            .map((label) => ({
              label: (label.textContent || '').trim(),
              clientWidth: label.clientWidth,
              scrollWidth: label.scrollWidth,
            })),
        };
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Could not measure medium navigation: {measurement!r}")
    if float(measurement.get("sidebarWidth", 0)) < 170:
        raise AssertionError(f"Medium navigation collapsed below its labeled width: {measurement!r}")
    if measurement.get("labels") != EXPECTED_NAVIGATION:
        raise AssertionError(f"Medium navigation did not expose complete labels: {measurement!r}")
    if measurement.get("clipped"):
        raise AssertionError(f"Medium navigation clipped label text: {measurement!r}")
    ui.assert_no_horizontal_overflow(session_id, "928px medium navigation")


def create_card(session_id: str, deadline: float) -> tuple[str, str, str]:
    deck_name = "Card listing regression deck"
    question = "Why does ADH increase collecting-duct water permeability?"
    answer = "It promotes aquaporin-2 insertion in principal-cell apical membranes."

    webdriver.click_text_button(session_id, "New deck", deadline)
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
    ui.wait_for_overlay_inactive(session_id, deadline, "card listing deck creation")

    webdriver.click_text_button(session_id, "Add flashcard", deadline)
    webdriver.set_rich_editor_text(session_id, 0, question, deadline)
    webdriver.set_rich_editor_text(session_id, 1, answer, deadline)
    save = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Save card']",
        deadline,
    )
    webdriver.click(session_id, save)
    webdriver.wait_for_main_heading(session_id, deck_name, deadline)
    webdriver.find_element(
        session_id,
        "css selector",
        "main .organized-card-list[aria-label='Matching cards']",
        deadline,
    )
    return deck_name, question, answer


def listing_snapshot(session_id: str) -> dict[str, Any]:
    measurement = ui.execute(
        session_id,
        """
        const sides = document.querySelector('main .organized-card-row .card-listing-sides');
        if (!(sides instanceof HTMLElement)) return null;
        const lines = [...sides.querySelectorAll(':scope > .card-listing-line')]
          .filter((line) => line instanceof HTMLElement);
        if (lines.length !== 2) return null;
        const q = lines[0].getBoundingClientRect();
        const a = lines[1].getBoundingClientRect();
        return {
          sidesClass: sides.className,
          lineCount: lines.length,
          qLabel: (lines[0].querySelector('.card-listing-label')?.textContent || '').trim(),
          aLabel: (lines[1].querySelector('.card-listing-label')?.textContent || '').trim(),
          question: (lines[0].querySelector('strong')?.textContent || '').trim(),
          answer: (lines[1].querySelector('strong')?.textContent || '').trim(),
          qTop: q.top,
          aTop: a.top,
          arrowCount: [...sides.querySelectorAll('[aria-hidden=true]')]
            .filter((element) => (element.textContent || '').includes('→')).length,
        };
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Shared card listing was not measurable: {measurement!r}")
    return measurement


def assert_shared_listing(
    measurement: dict[str, Any],
    question: str,
    answer: str,
    label: str,
) -> None:
    if measurement.get("lineCount") != 2:
        raise AssertionError(f"{label} does not contain exactly two shared Q/A lines: {measurement!r}")
    if measurement.get("qLabel") != "Q:" or measurement.get("aLabel") != "A:":
        raise AssertionError(f"{label} does not use literal Q:/A: labels: {measurement!r}")
    if question not in str(measurement.get("question", "")):
        raise AssertionError(f"{label} lost the question text: {measurement!r}")
    if answer not in str(measurement.get("answer", "")):
        raise AssertionError(f"{label} lost the answer text: {measurement!r}")
    if float(measurement["aTop"]) <= float(measurement["qTop"]) + 1:
        raise AssertionError(f"{label} Q/A previews share one horizontal line: {measurement!r}")
    if int(measurement.get("arrowCount", -1)) != 0:
        raise AssertionError(f"{label} still renders the old Q→A arrow: {measurement!r}")


def assert_deck_browser(session_id: str, question: str, answer: str) -> dict[str, Any]:
    filter_count = ui.execute(
        session_id,
        "return document.querySelectorAll(\"main section[aria-label='Card filters']\").length;",
    )
    if int(filter_count) != 0:
        raise AssertionError("Deck detail exposed the advanced Search/Filter form")
    measurement = listing_snapshot(session_id)
    assert_shared_listing(measurement, question, answer, "Deck detail")
    return measurement


def assert_search_surface(session_id: str) -> None:
    measurement = ui.execute(
        session_id,
        """
        const input = document.querySelector("main section[aria-label='Card filters'] input[type='search']");
        const surface = input?.parentElement;
        if (!(input instanceof HTMLInputElement) || !(surface instanceof HTMLElement)) return null;
        const inputRect = input.getBoundingClientRect();
        const surfaceRect = surface.getBoundingClientRect();
        return {
          iconInsideSurface: Boolean(surface.querySelector('svg')),
          inputBackground: getComputedStyle(input).backgroundColor,
          surfaceBackground: getComputedStyle(surface).backgroundColor,
          inputLeft: inputRect.left,
          inputRight: inputRect.right,
          surfaceLeft: surfaceRect.left,
          surfaceRight: surfaceRect.right,
        };
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Search input surface was not measurable: {measurement!r}")
    if measurement.get("iconInsideSurface") is not True:
        raise AssertionError(f"Search icon is outside the input surface: {measurement!r}")
    if measurement.get("inputBackground") not in ("rgba(0, 0, 0, 0)", "transparent"):
        raise AssertionError(f"Search input paints a second background: {measurement!r}")
    if measurement.get("surfaceBackground") in ("rgba(0, 0, 0, 0)", "transparent"):
        raise AssertionError(f"Search control has no unified painted surface: {measurement!r}")
    if (
        float(measurement["inputLeft"]) < float(measurement["surfaceLeft"])
        or float(measurement["inputRight"]) > float(measurement["surfaceRight"]) + 1
    ):
        raise AssertionError(f"Search input escapes its unified surface: {measurement!r}")


def assert_search_browser(
    session_id: str,
    question: str,
    answer: str,
    deck_measurement: dict[str, Any],
    deadline: float,
) -> None:
    search_tab = ui.find_workspace_tab(session_id, "Search", deadline)
    webdriver.click(session_id, search_tab)
    webdriver.find_element(
        session_id,
        "css selector",
        "main section[aria-label='Card filters']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "css selector",
        "main .organized-card-list[aria-label='Matching cards']",
        deadline,
    )
    measurement = listing_snapshot(session_id)
    assert_shared_listing(measurement, question, answer, "Search/Filter")
    if measurement.get("sidesClass") != deck_measurement.get("sidesClass"):
        raise AssertionError(
            "Deck and Search did not render the same shared card-listing DOM contract: "
            f"deck={deck_measurement!r}, search={measurement!r}"
        )
    assert_search_surface(session_id)
    ui.assert_no_horizontal_overflow(session_id, "medium shared Search card browser")


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
        webdriver.wait_for_driver(webdriver.deadline_after(20))
        session_id = webdriver.create_session(application)
        webdriver.verify_shell(session_id, webdriver.deadline_after())
        deadline = webdriver.deadline_after(60)
        assert_medium_navigation(session_id, deadline)
        _, question, answer = create_card(session_id, deadline)
        deck_measurement = assert_deck_browser(session_id, question, answer)
        assert_search_browser(session_id, question, answer, deck_measurement, deadline)
        print(
            "Native shared card-browser smoke passed: 928px navigation labels fit; Deck hides "
            "advanced filters; Deck and Search share literal two-line Q:/A: rows; Search uses one "
            "icon/input surface."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver shared card-browser log ---")
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
