#!/usr/bin/env python3
"""Run the UI-platform smoke against the current desktop UI contracts.

The redesign deliberately removed ostudy-rendered maximize/full-screen buttons,
and the responsive action system deliberately promotes a sole overflow action
instead of rendering a one-item More menu. This adapter replaces only stale
assertions for those two policies while reusing the complete UI-platform smoke.
"""

from __future__ import annotations

import time

import ui_platform_smoke as ui
import smoke as webdriver


def assert_no_app_window_controls(session_id: str, label: str) -> None:
    result = ui.execute(
        session_id,
        """
        const groups = [...document.querySelectorAll(
          "[role='group'][aria-label='Window controls']"
        )];
        const labeled = [...document.querySelectorAll(
          "button[aria-label='Maximize window'], button[aria-label='Restore window'], " +
          "button[aria-label='Enter full screen'], button[aria-label='Exit full screen']"
        )];
        return {
          groupCount: groups.length,
          labeledControlCount: labeled.length,
          layoutMode: document.documentElement.dataset.layoutMode ?? null,
        };
        """,
    )
    if not isinstance(result, dict):
        raise AssertionError(f"Could not inspect app window controls in {label}: {result!r}")
    if int(result.get("groupCount", -1)) != 0 or int(result.get("labeledControlCount", -1)) != 0:
        raise AssertionError(
            f"ostudy reintroduced duplicate in-app window controls in {label}: {result!r}"
        )


def verify_os_owned_native_window_states(session_id: str, deadline: float) -> None:
    ui.open_study_guides(session_id, deadline, "resized desktop viewport")
    route_selector = "[aria-label='Study Guides']"

    # Start from a deterministic ordinary window. W3C Set Window Rect also
    # leaves any prior maximized/full-screen state before applying the size.
    ui.set_window_rect(session_id, 1280, 800)
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    assert_no_app_window_controls(session_id, "resized desktop viewport")
    ui.assert_viewport_consumption(session_id, "resized desktop viewport", route_selector)

    webdriver.request("POST", f"/session/{session_id}/window/maximize", {})
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    assert_no_app_window_controls(session_id, "maximized desktop viewport")
    ui.assert_viewport_consumption(session_id, "maximized desktop viewport", route_selector)

    ui.set_window_rect(session_id, 1280, 800)
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    assert_no_app_window_controls(session_id, "restored desktop viewport after maximize")
    ui.assert_viewport_consumption(
        session_id,
        "restored desktop viewport after maximize",
        route_selector,
    )

    webdriver.request("POST", f"/session/{session_id}/window/fullscreen", {})
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    assert_no_app_window_controls(session_id, "full-screen desktop viewport")
    ui.assert_viewport_consumption(session_id, "full-screen desktop viewport", route_selector)

    ui.set_window_rect(session_id, 1280, 800)
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    assert_no_app_window_controls(session_id, "desktop viewport after exiting full screen")
    ui.assert_viewport_consumption(
        session_id,
        "desktop viewport after exiting full screen",
        route_selector,
    )

    ui.set_window_rect(session_id, 1440, 900)
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    ui.assert_viewport_consumption(
        session_id,
        "resized desktop viewport after native states",
        route_selector,
    )


def verify_current_deck_detail_header(
    session_id: str,
    deck_name: str,
    deck_description: str,
) -> None:
    """Verify the current vertical deck header and responsive action policy."""
    placements = (
        (
            "wide",
            1440,
            900,
            [
                "Practice",
                "Export",
                f"Archive {deck_name}",
                f"Delete {deck_name}",
            ],
            [],
        ),
        (
            "medium",
            1000,
            800,
            ["Practice", "Export", "More actions"],
            [f"Archive {deck_name}", f"Delete {deck_name}"],
        ),
        (
            "compact",
            390,
            700,
            ["Practice", "More actions"],
            ["Export", f"Archive {deck_name}", f"Delete {deck_name}"],
        ),
    )

    for mode, width, height, expected_direct, expected_overflow in placements:
        mode_deadline = webdriver.deadline_after()
        ui.set_window_rect(session_id, width, height)
        snapshot = None
        while time.monotonic() < mode_deadline:
            snapshot = ui.execute(
                session_id,
                """
                const deckName = arguments[0];
                const deckDescription = arguments[1];
                const heading = [...document.querySelectorAll('main h1')]
                  .find((candidate) => candidate.textContent?.trim() === deckName);
                const header = heading?.closest('header');
                const back = header
                  ? [...header.querySelectorAll('button')]
                    .find((button) => button.textContent?.trim() === '← All decks')
                  : null;
                const description = header
                  ? [...header.querySelectorAll('p')]
                    .find((candidate) => candidate.textContent?.trim() === deckDescription)
                  : null;
                const actions = header?.querySelector('[role="group"]');
                const titleBlock = heading?.parentElement;
                if (!(heading instanceof HTMLElement)
                    || !(header instanceof HTMLElement)
                    || !(back instanceof HTMLElement)
                    || !(description instanceof HTMLElement)
                    || !(actions instanceof HTMLElement)
                    || !(titleBlock instanceof HTMLElement)) return null;

                const directButtons = [...actions.children].flatMap((child) => {
                  if (child instanceof HTMLButtonElement) return [child];
                  const trigger = child.querySelector('button');
                  return trigger instanceof HTMLButtonElement ? [trigger] : [];
                });
                const labels = directButtons.map((button) =>
                  (button.getAttribute('aria-label') || button.textContent || '').trim()
                );
                const rect = (element) => {
                  const bounds = element.getBoundingClientRect();
                  return {
                    left: bounds.left,
                    top: bounds.top,
                    right: bounds.right,
                    bottom: bounds.bottom,
                    width: bounds.width,
                    height: bounds.height,
                  };
                };
                const geometry = {
                  header: rect(header),
                  back: rect(back),
                  titleBlock: rect(titleBlock),
                  heading: rect(heading),
                  description: rect(description),
                  actions: rect(actions),
                };
                const headerRect = geometry.header;
                const contained = [back, titleBlock, actions].every((element) => {
                  const bounds = element.getBoundingClientRect();
                  return bounds.left >= headerRect.left - 1
                    && bounds.right <= headerRect.right + 1;
                });
                const visibleOverflow = [...header.querySelectorAll('*')]
                  .filter((element) => {
                    if (!(element instanceof HTMLElement)) return false;
                    if (element.classList.contains('sr-only')) return false;
                    const style = getComputedStyle(element);
                    if (style.display === 'none' || style.visibility === 'hidden') return false;
                    const bounds = element.getBoundingClientRect();
                    if (bounds.width <= 1 && bounds.height <= 1) return false;
                    return element.scrollWidth > element.clientWidth + 1
                      || bounds.left < headerRect.left - 1
                      || bounds.right > headerRect.right + 1;
                  })
                  .map((element) => ({
                    tag: element.tagName.toLowerCase(),
                    className: element.className?.toString() || '',
                    ariaLabel: element.getAttribute('aria-label'),
                    text: (element.textContent || '').trim().slice(0, 120),
                  }));
                const above = (upper, lower) => upper.bottom <= lower.top + 1;
                return {
                  mode: actions.dataset.layoutMode,
                  labels,
                  geometry,
                  contained,
                  ordered:
                    Boolean(back.compareDocumentPosition(titleBlock) & Node.DOCUMENT_POSITION_FOLLOWING)
                    && Boolean(titleBlock.compareDocumentPosition(actions) & Node.DOCUMENT_POSITION_FOLLOWING),
                  visuallyOrdered:
                    above(geometry.back, geometry.titleBlock)
                    && above(geometry.heading, geometry.description)
                    && above(geometry.titleBlock, geometry.actions),
                  titleBlockContained:
                    geometry.titleBlock.left >= headerRect.left - 1
                    && geometry.titleBlock.right <= headerRect.right + 1,
                  noHeaderOverflow: header.scrollWidth <= header.clientWidth,
                  noDocumentOverflow:
                    document.documentElement.scrollWidth <= document.documentElement.clientWidth,
                  visibleOverflow,
                };
                """,
                [deck_name, deck_description],
            )
            if (
                isinstance(snapshot, dict)
                and snapshot.get("mode") == mode
                and snapshot.get("labels") == expected_direct
                and snapshot.get("ordered") is True
                and snapshot.get("visuallyOrdered") is True
                and snapshot.get("titleBlockContained") is True
                and snapshot.get("contained") is True
                and snapshot.get("noHeaderOverflow") is True
                and snapshot.get("noDocumentOverflow") is True
                and snapshot.get("visibleOverflow") == []
            ):
                break
            time.sleep(0.1)
        else:
            raise AssertionError(
                f"Deck detail header failed {mode} current-layout contract: {snapshot!r}"
            )

        if not expected_overflow:
            more_count = ui.execute(
                session_id,
                """
                return document.querySelectorAll(
                  "main header button[aria-label='More actions']"
                ).length;
                """,
            )
            if more_count != 0:
                raise AssertionError(
                    "Wide deck detail exposed a one-item More menu instead of promoting Delete."
                )
            continue

        more_actions = webdriver.find_element(
            session_id,
            "css selector",
            "main header button[aria-label='More actions']",
            mode_deadline,
        )
        webdriver.click(session_id, more_actions)
        overflow_items = ui.execute(
            session_id,
            """
            return [...document.querySelectorAll(
              '[role="menu"][aria-label="More actions"] [role="menuitem"]'
            )].map((item) => (item.textContent || '').trim());
            """,
        )
        if overflow_items != expected_overflow:
            raise AssertionError(
                f"Deck detail {mode} overflow actions were incorrect: {overflow_items!r}"
            )
        ui.send_key(session_id, ui.ESCAPE_KEY)
        ui.wait_for_absent(
            session_id,
            "css selector",
            '[role="menu"][aria-label="More actions"]',
            mode_deadline,
        )


if __name__ == "__main__":
    ui.verify_native_window_controls = verify_os_owned_native_window_states
    ui.verify_deck_detail_header = verify_current_deck_detail_header
    raise SystemExit(ui.main())
