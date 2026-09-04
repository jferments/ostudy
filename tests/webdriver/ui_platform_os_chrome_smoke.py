#!/usr/bin/env python3
"""Run the UI-platform smoke with the current OS-owned window-chrome policy.

The UI redesign deliberately removed ostudy-rendered maximize/full-screen
buttons. Desktop window state is owned by the operating system/window manager,
so the native smoke drives those W3C window states directly and verifies that
our responsive shell consumes each resulting viewport without reintroducing
duplicate application controls.
"""

from __future__ import annotations

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


if __name__ == "__main__":
    ui.verify_native_window_controls = verify_os_owned_native_window_states
    raise SystemExit(ui.main())
