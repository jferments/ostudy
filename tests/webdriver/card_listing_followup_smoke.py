#!/usr/bin/env python3
"""Focused native regression for shared card listings and narrow medium layout."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import tempfile
from pathlib import Path

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
    return deck_name, question, answer


def assert_deck_preview(session_id: str, question: str, answer: str) -> None:
    measurement = ui.execute(
        session_id,
        """
        const preview = document.querySelector('.card-row-preview');
        const lines = preview ? [...preview.querySelectorAll('.card-preview-line')] : [];
        if (!(preview instanceof HTMLElement) || lines.length < 2) return null;
        const q = lines[0].getBoundingClientRect();
        const a = lines[1].getBoundingClientRect();
        const qLabel = lines[0].querySelector('.card-preview-label');
        const aLabel = lines[1].querySelector('.card-preview-label');
        const qText = lines[0].querySelector('strong');
        const aText = lines[1].querySelector('strong');
        return {
          qLabel: (qLabel?.textContent || '').trim(),
          aLabel: (aLabel?.textContent || '').trim(),
          question: (qText?.textContent || '').replace(/\\s+/g, ' ').trim(),
          answer: (aText?.textContent || '').replace(/\\s+/g, ' ').trim(),
          qTop: q.top,
          aTop: a.top,
          columns: getComputedStyle(preview).gridTemplateColumns,
        };
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Deck card preview was not measurable: {measurement!r}")
    if measurement.get("qLabel") != "Q:" or measurement.get("aLabel") != "A:":
        raise AssertionError(f"Deck preview does not use compact Q:/A: labels: {measurement!r}")
    if measurement.get("question") != question or measurement.get("answer") != answer:
        raise AssertionError(f"Deck preview lost card content: {measurement!r}")
    if float(measurement["aTop"]) <= float(measurement["qTop"]) + 1:
        raise AssertionError(f"Deck Q/A previews share one horizontal line: {measurement!r}")


def assert_search_surface_and_preview(session_id: str, question: str, answer: str, deadline: float) -> None:
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
        "main .organized-card-sides",
        deadline,
    )

    measurement = ui.execute(
        session_id,
        """
        const input = document.querySelector("main section[aria-label='Card filters'] input[type='search']");
        const surface = input?.parentElement;
        const sides = document.querySelector('main .organized-card-sides');
        if (!(input instanceof HTMLInputElement) || !(surface instanceof HTMLElement) || !(sides instanceof HTMLElement)) {
          return null;
        }
        const content = [...sides.children]
          .filter((element) => element.getAttribute('aria-hidden') !== 'true')
          .filter((element) => element instanceof HTMLElement);
        if (content.length < 2) return null;
        const qLabel = content[0].querySelector('small');
        const aLabel = content[1].querySelector('small');
        const qText = content[0].querySelector('strong');
        const aText = content[1].querySelector('strong');
        const arrow = [...sides.children].find((element) => element.getAttribute('aria-hidden') === 'true');
        const q = content[0].getBoundingClientRect();
        const a = content[1].getBoundingClientRect();
        const inputRect = input.getBoundingClientRect();
        const surfaceRect = surface.getBoundingClientRect();
        return {
          question: (qText?.textContent || '').replace(/\\s+/g, ' ').trim(),
          answer: (aText?.textContent || '').replace(/\\s+/g, ' ').trim(),
          qLabel: qLabel ? getComputedStyle(qLabel, '::after').content : '',
          aLabel: aLabel ? getComputedStyle(aLabel, '::after').content : '',
          qTop: q.top,
          aTop: a.top,
          arrowDisplay: arrow instanceof HTMLElement ? getComputedStyle(arrow).display : null,
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
        raise AssertionError(f"Search card presentation was not measurable: {measurement!r}")
    if "Q:" not in str(measurement.get("qLabel", "")) or "A:" not in str(measurement.get("aLabel", "")):
        raise AssertionError(f"Search results do not present compact Q:/A: labels: {measurement!r}")
    if measurement.get("question") != question or measurement.get("answer") != answer:
        raise AssertionError(f"Search result preview lost card content: {measurement!r}")
    if float(measurement["aTop"]) <= float(measurement["qTop"]) + 1:
        raise AssertionError(f"Search Q/A previews share one horizontal line: {measurement!r}")
    if measurement.get("arrowDisplay") != "none":
        raise AssertionError(f"Search result still displays the old Q→A arrow: {measurement!r}")
    if measurement.get("iconInsideSurface") is not True:
        raise AssertionError(f"Search icon is outside the input surface: {measurement!r}")
    if measurement.get("inputBackground") not in ("rgba(0, 0, 0, 0)", "transparent"):
        raise AssertionError(f"Search input paints a second background inside its surface: {measurement!r}")
    if measurement.get("surfaceBackground") in ("rgba(0, 0, 0, 0)", "transparent"):
        raise AssertionError(f"Search control has no unified painted surface: {measurement!r}")
    if float(measurement["inputLeft"]) < float(measurement["surfaceLeft"]) or float(measurement["inputRight"]) > float(measurement["surfaceRight"]) + 1:
        raise AssertionError(f"Search input escapes its unified surface: {measurement!r}")
    ui.assert_no_horizontal_overflow(session_id, "medium Search listing follow-up")


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
        assert_medium_navigation(session_id, webdriver.deadline_after(45))
        _, question, answer = create_card(session_id, webdriver.deadline_after(60))
        assert_deck_preview(session_id, question, answer)
        assert_search_surface_and_preview(
            session_id,
            question,
            answer,
            webdriver.deadline_after(60),
        )
        print(
            "Native card-listing follow-up smoke passed: 928px medium navigation has complete labels; "
            "Deck and Search use separate Q:/A: lines; Search uses one icon/input surface."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver card-listing follow-up log ---")
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
