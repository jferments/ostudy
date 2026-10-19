#!/usr/bin/env python3
"""Focused native regression for the Library Workbench's card list at medium width."""

from __future__ import annotations

import argparse
import os
import re
import signal
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

import smoke as webdriver
import ui_platform_smoke as ui
import tutor_smoke


MEDIUM_WIDTH = 928
MEDIUM_HEIGHT = 900
EXPECTED_NAVIGATION = ["Home", "Library", "Review", "Tutor", "Progress"]


def assert_medium_navigation(session_id: str, deadline: float) -> None:
    ui.set_window_rect(session_id, MEDIUM_WIDTH, MEDIUM_HEIGHT)
    ui.wait_for_layout_mode(session_id, "medium", deadline)
    measurement = ui.execute(
        session_id,
        """
        const header = document.querySelector("header[data-slot='app-header']");
        const navigation = document.querySelector("nav[aria-label='Main navigation']");
        const labels = [...document.querySelectorAll("nav[aria-label='Main navigation'] [data-slot='nav-label']")]
          .filter((label) => label instanceof HTMLElement);
        const headerActions = [...(header?.querySelectorAll("button[data-testid^='app-']") ?? [])];
        return {
          inHeader: Boolean(header && navigation && header.contains(navigation)),
          headerActions: headerActions.map((button) => button.dataset.testid),
          headerFits: header instanceof HTMLElement && header.scrollWidth <= header.clientWidth + 1,
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
    if measurement.get("inHeader") is not True or measurement.get("headerFits") is not True:
        raise AssertionError(f"Medium navigation left the labeled header bar: {measurement!r}")
    if measurement.get("headerActions") != ["app-search", "app-settings"]:
        raise AssertionError(f"Medium header lost Search or the Settings gear: {measurement!r}")
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
        "[role='dialog'] input[data-autofocus='true']",
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
    webdriver.wait_for_workbench_title(session_id, deck_name, deadline)
    ui.wait_for_overlay_inactive(session_id, deadline, "card listing deck creation")

    webdriver.click_text_button(session_id, "New card", deadline)
    webdriver.set_rich_editor_text(session_id, 0, question, deadline)
    webdriver.set_rich_editor_text(session_id, 1, answer, deadline)
    save = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Save card']",
        deadline,
    )
    webdriver.click(session_id, save)
    # Medium screens show the editor in place of the list; closing it returns to the list.
    close_open_editor(session_id, deadline)
    webdriver.wait_for_workbench_title(session_id, deck_name, deadline)
    webdriver.find_card_row(session_id, question, deadline)
    return deck_name, question, answer


def close_open_editor(session_id: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        if not webdriver.element_exists(session_id, "css selector", "main [data-testid='workbench-editor']"):
            return
        buttons = webdriver.request(
            "POST",
            f"/session/{session_id}/elements",
            {"using": "xpath", "value": "//main//*[@data-testid='workbench-editor']//button[normalize-space(.)='Cancel' or @aria-label='Close editor' or normalize-space(.)='Close']"},
        )
        if isinstance(buttons, list) and buttons:
            try:
                webdriver.click(session_id, str(buttons[0][webdriver.ELEMENT_KEY]))
            except webdriver.WebDriverProtocolError as error:
                # Saving a new card closes the editor itself, which can remove
                # the button between finding and clicking it.
                if error.error not in ("stale element reference", "no such element"):
                    raise
        time.sleep(0.2)
    raise AssertionError("The card editor did not close")


def workbench_snapshot(session_id: str, deadline: float) -> dict[str, Any]:
    measurement: Any = None
    while time.monotonic() < deadline:
        measurement = ui.execute(
            session_id,
            """
            const workbench = document.querySelector("main [data-testid='library-workbench']");
            if (!(workbench instanceof HTMLElement)) return null;
            const tree = workbench.querySelector("nav[aria-label='Library']");
            const list = workbench.querySelector("section[aria-label='Cards']");
            const rows = [...workbench.querySelectorAll("[data-card-id]")];
            if (!(tree instanceof HTMLElement) || !(list instanceof HTMLElement) || rows.length === 0) return null;
            const separators = [...workbench.querySelectorAll("[role='separator'][aria-orientation='vertical']")];
            const treeRect = tree.getBoundingClientRect();
            const listRect = list.getBoundingClientRect();
            const row = rows[0];
            const open = row.querySelector("button");
            return {
              layoutMode: document.querySelector("[data-layout-mode]")?.getAttribute("data-layout-mode") || "",
              separators: separators.length,
              sideBySide: treeRect.right <= listRect.left + 1,
              rowText: (open?.textContent || "").trim(),
              rowFits: row.scrollWidth <= row.clientWidth + 1,
              filters: [...workbench.querySelectorAll("[role='group'][aria-label='Filter cards'] button")]
                .map((button) => (button.textContent || "").replace(/\s+/g, " ").trim()),
            };
            """,
        )
        if isinstance(measurement, dict):
            return measurement
        time.sleep(0.1)
    raise AssertionError(f"The Workbench was not measurable before deadline: {measurement!r}")


def assert_deck_list(session_id: str, question: str, deadline: float) -> None:
    """A deck shows the tree and its card list side by side, with quick filters."""
    measurement = workbench_snapshot(session_id, deadline)
    if measurement.get("separators") != 1 or measurement.get("sideBySide") is not True:
        raise AssertionError(f"Medium Workbench did not show tree and list side by side: {measurement!r}")
    if question not in str(measurement.get("rowText", "")):
        raise AssertionError(f"Card row lost its question: {measurement!r}")
    if measurement.get("rowFits") is not True:
        raise AssertionError(f"Card row overflowed its pane: {measurement!r}")
    filters = measurement.get("filters")
    if not isinstance(filters, list) or [re.sub(r"[\d,]+$", "", label).strip() for label in filters] != ["All", "Due", "New", "Suspended"]:
        raise AssertionError(f"Deck quick filters drifted: {measurement!r}")
    ui.assert_no_horizontal_overflow(session_id, "medium Workbench deck list")


def assert_editor_replaces_list(session_id: str, question: str, deadline: float) -> None:
    """Medium screens open a card in place of the list, then return to it."""
    webdriver.click(session_id, webdriver.find_card_row(session_id, question, deadline))
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='workbench-editor']//h1[normalize-space(.)='Edit card']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "css selector",
        "main [data-testid='workbench-editor'] section[aria-label='Schedule']",
        deadline,
    )
    if webdriver.element_exists(session_id, "css selector", "main section[aria-label='Cards']"):
        raise AssertionError("Medium Workbench kept the card list beside the editor")
    ui.assert_no_horizontal_overflow(session_id, "medium Workbench card editor")
    close_open_editor(session_id, deadline)
    webdriver.find_card_row(session_id, question, deadline)


def assert_search_surface_measurement(measurement: Any) -> None:
    """Require one painted control, independent of whether shadcn or its wrapper paints it."""
    if not isinstance(measurement, dict):
        raise AssertionError(f"Search input surface was not measurable: {measurement!r}")
    if measurement.get("iconInsideSurface") is not True or measurement.get("iconVisible") is not True:
        raise AssertionError(f"Search icon is missing or hidden: {measurement!r}")
    colors = [measurement.get("inputBackground"), measurement.get("surfaceBackground")]
    if not all(isinstance(color, str) and color.strip() for color in colors):
        raise AssertionError(f"Search background ownership was not measurable: {measurement!r}")
    transparent = {"rgba(0,0,0,0)", "transparent"}
    painted = [color.replace(" ", "").lower() not in transparent for color in colors]
    if sum(painted) != 1:
        raise AssertionError(f"Search control must paint exactly one background: {measurement!r}")

    def rect(name: str) -> dict[str, float]:
        value = measurement.get(name)
        if not isinstance(value, dict):
            raise AssertionError(f"Search {name} geometry is missing: {measurement!r}")
        try:
            result = {edge: float(value[edge]) for edge in ("left", "top", "right", "bottom")}
        except (KeyError, TypeError, ValueError) as error:
            raise AssertionError(f"Search {name} geometry is invalid: {measurement!r}") from error
        if not (result["right"] > result["left"] and result["bottom"] > result["top"]):
            raise AssertionError(f"Search {name} has no visible area: {measurement!r}")
        return result

    def contained(inner: dict[str, float], outer: dict[str, float]) -> bool:
        return (inner["left"] >= outer["left"] - 1 and inner["right"] <= outer["right"] + 1
                and inner["top"] >= outer["top"] - 1 and inner["bottom"] <= outer["bottom"] + 1)

    input_rect, surface_rect, icon_rect = rect("inputRect"), rect("surfaceRect"), rect("iconRect")
    painted_rect = input_rect if painted[0] else surface_rect
    if not contained(input_rect, surface_rect):
        raise AssertionError(f"Search input escapes its wrapper: {measurement!r}")
    if not contained(input_rect, painted_rect) or not contained(icon_rect, painted_rect):
        raise AssertionError(f"Search input or icon escapes its painted surface: {measurement!r}")


def assert_search_surface(session_id: str) -> None:
    measurement = ui.execute(
        session_id,
        """
        const input = document.querySelector("main [data-testid='library-workbench'] input[type='search']");
        const surface = input?.parentElement;
        const icon = surface?.querySelector('svg');
        if (!(input instanceof HTMLInputElement) || !(surface instanceof HTMLElement)
            || !(icon instanceof SVGElement)) return null;
        const rect = (element) => {
          const { left, top, right, bottom } = element.getBoundingClientRect();
          return { left, top, right, bottom };
        };
        const iconStyle = getComputedStyle(icon);
        return {
          iconInsideSurface: surface.contains(icon),
          iconVisible: iconStyle.display !== 'none' && iconStyle.visibility !== 'hidden',
          inputBackground: getComputedStyle(input).backgroundColor,
          surfaceBackground: getComputedStyle(surface).backgroundColor,
          inputRect: rect(input),
          surfaceRect: rect(surface),
          iconRect: rect(icon),
        };
        """,
    )
    assert_search_surface_measurement(measurement)


def assert_search_list(session_id: str, question: str, deadline: float) -> None:
    """Searching All cards finds the card by its answer's words, ranked by relevance."""
    webdriver.click(session_id, webdriver.workbench_tree_entry(session_id, "All cards", deadline))
    webdriver.wait_for_workbench_title(session_id, "All cards", deadline)
    webdriver.set_input_text(
        session_id,
        "css selector",
        "main [data-testid='library-workbench'] input[type='search']",
        "aquaporin",
        deadline,
    )
    webdriver.find_card_row(session_id, question, deadline)
    sort = ui.execute(
        session_id,
        "return document.querySelector(\"main [data-testid='library-workbench'] select\")?.value || '';",
    )
    if sort != "relevance":
        raise AssertionError(f"Searching did not sort by relevance: {sort!r}")
    assert_search_surface(session_id)
    ui.assert_no_horizontal_overflow(session_id, "medium Workbench search")


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
        webdriver.open_library_view(session_id, "decks", deadline)
        deck_name, question, _answer = create_card(session_id, deadline)
        assert_deck_list(session_id, question, deadline)
        assert_editor_replaces_list(session_id, question, deadline)
        assert_search_list(session_id, question, deadline)
        tutor_smoke.verify_tutor_and_contextual_return(session_id, deck_name)
        print(
            "Native Workbench card-list smoke passed: 928px side rail navigation labels fit; the tree "
            "and card list sit side by side; a card opens in place of the list; search ranks by "
            "relevance with one icon/input surface."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver Workbench card-list log ---")
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
