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
SPACE_KEY = "\ue00d"
EXPECTED_PLEX_FAMILY_SERIALIZATIONS = frozenset(
    {
        "'IBM Plex Sans', var(--ostudy-font-family-system)",
        '"IBM Plex Sans", var(--ostudy-font-family-system)',
    }
)


def is_expected_plex_family(value: Any) -> bool:
    return isinstance(value, str) and value in EXPECTED_PLEX_FAMILY_SERIALIZATIONS


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


def execute_async(session_id: str, script: str, args: list[Any] | None = None) -> Any:
    return webdriver.request(
        "POST",
        f"/session/{session_id}/execute/async",
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


def send_element_key(session_id: str, element_id: str, value: str) -> None:
    """Send a key to one resolved element without introducing a pointer click."""
    webdriver.request(
        "POST",
        f"/session/{session_id}/element/{element_id}/value",
        {"text": value, "value": [value]},
    )


def arm_window_control_keyboard_probe(
    session_id: str,
    element_id: str,
    token: str,
) -> None:
    armed = execute(
        session_id,
        """
        const button = arguments[0];
        const token = arguments[1];
        if (!(button instanceof HTMLButtonElement)) return null;
        window.__ostudyWindowControlProbeCleanup?.();
        const state = {
          token,
          initialLabel: button.getAttribute("aria-label"),
          keydown: null,
          keyup: null,
          click: null
        };
        const recordKey = (field, event) => {
          if (event.target !== button || event.key !== " ") return;
          state[field] = {
            trusted: event.isTrusted,
            key: event.key,
            label: button.getAttribute("aria-label")
          };
        };
        const recordClick = (event) => {
          if (event.target !== button) return;
          state.click = {
            trusted: event.isTrusted,
            detail: event.detail,
            label: button.getAttribute("aria-label")
          };
        };
        const onKeyDown = (event) => recordKey("keydown", event);
        const onKeyUp = (event) => recordKey("keyup", event);
        document.addEventListener("keydown", onKeyDown, true);
        document.addEventListener("keyup", onKeyUp, true);
        document.addEventListener("click", recordClick, true);
        window.__ostudyWindowControlProbe = state;
        window.__ostudyWindowControlProbeCleanup = () => {
          document.removeEventListener("keydown", onKeyDown, true);
          document.removeEventListener("keyup", onKeyUp, true);
          document.removeEventListener("click", recordClick, true);
        };
        return token;
        """,
        [{webdriver.ELEMENT_KEY: element_id}, token],
    )
    if armed != token:
        raise AssertionError(f"Could not arm native keyboard probe {token!r}: {armed!r}")


def read_window_control_keyboard_probe(session_id: str, token: str) -> Any:
    return execute(
        session_id,
        """
        const state = window.__ostudyWindowControlProbe;
        if (!state || state.token !== arguments[0]) return null;
        return JSON.parse(JSON.stringify(state));
        """,
        [token],
    )


def wait_for_window_control_keyboard_probe(
    session_id: str,
    token: str,
    deadline: float,
) -> dict[str, Any]:
    probe_deadline = min(deadline, time.monotonic() + 5)
    last: Any = None
    while time.monotonic() < probe_deadline:
        last = read_window_control_keyboard_probe(session_id, token)
        if isinstance(last, dict):
            keydown = last.get("keydown")
            keyup = last.get("keyup")
            click = last.get("click")
            if (
                isinstance(keydown, dict)
                and keydown.get("trusted") is True
                and keydown.get("key") == " "
                and isinstance(keyup, dict)
                and keyup.get("trusted") is True
                and keyup.get("key") == " "
                and isinstance(click, dict)
                and click.get("trusted") is True
                and type(click.get("detail")) is int
                and click.get("detail") == 0
            ):
                execute(
                    session_id,
                    "window.__ostudyWindowControlProbeCleanup?.(); return true;",
                )
                return last
        time.sleep(0.05)
    raise AssertionError(
        "Native window-control keyboard activation did not produce trusted "
        f"Space keydown/keyup and keyboard click events: {last!r}"
    )


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


def wait_for_layout_stable(session_id: str) -> None:
    deadline = webdriver.deadline_after()
    previous: tuple[float, ...] | None = None
    stable_samples = 0
    while time.monotonic() < deadline:
        snapshot = execute_async(
            session_id,
            """
            const done = arguments[arguments.length - 1];
            requestAnimationFrame(() => requestAnimationFrame(() => {
              const main = document.querySelector("main");
              const shell = main?.parentElement;
              const route = main?.firstElementChild;
              const measure = (element) => {
                if (!(element instanceof HTMLElement)) return [0, 0];
                const bounds = element.getBoundingClientRect();
                return [bounds.width, bounds.height];
              };
              done([
                document.documentElement.clientWidth,
                document.documentElement.clientHeight,
                ...measure(shell),
                ...measure(main),
                ...measure(route),
              ]);
            }));
            """,
        )
        if not isinstance(snapshot, list) or len(snapshot) != 8:
            raise AssertionError(f"Unexpected layout-stability snapshot: {snapshot!r}")
        current = tuple(round(float(value), 2) for value in snapshot)
        if current == previous:
            stable_samples += 1
            if stable_samples >= 2:
                return
        else:
            stable_samples = 0
            previous = current
        time.sleep(0.05)
    raise AssertionError(f"Viewport geometry did not stabilize: {previous!r}")


def assert_viewport_consumption(
    session_id: str,
    label: str,
    route_selector: str | None = None,
) -> None:
    wait_for_layout_stable(session_id)
    measurements = execute(
        session_id,
        """
        const routeSelector = arguments[0];
        const main = document.querySelector("main");
        const shell = main?.parentElement;
        const routeLayer = main?.firstElementChild;
        const route = routeSelector ? routeLayer?.querySelector(routeSelector) : null;
        const shellStyle = shell instanceof HTMLElement ? getComputedStyle(shell) : null;
        const rect = (element) => {
          if (!(element instanceof HTMLElement)) return null;
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
        return {
          viewportWidth: document.documentElement.clientWidth,
          viewportHeight: document.documentElement.clientHeight,
          shellPaddingTop: Number.parseFloat(shellStyle?.paddingTop || "0") || 0,
          shellPaddingBottom: Number.parseFloat(shellStyle?.paddingBottom || "0") || 0,
          shell: rect(shell),
          main: rect(main),
          routeLayer: rect(routeLayer),
          route: rect(route),
        };
        """,
        [route_selector],
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Unexpected viewport measurement for {label}: {measurements!r}")
    shell = measurements.get("shell")
    main = measurements.get("main")
    route_layer = measurements.get("routeLayer")
    if not all(isinstance(value, dict) for value in (shell, main, route_layer)):
        raise AssertionError(f"Missing shell geometry for {label}: {measurements!r}")
    viewport_width = float(measurements.get("viewportWidth", 0))
    viewport_height = float(measurements.get("viewportHeight", 0))
    shell_padding_top = float(measurements.get("shellPaddingTop", 0))
    shell_padding_bottom = float(measurements.get("shellPaddingBottom", 0))
    tolerance = 2
    if (
        abs(float(shell["left"])) > tolerance
        or abs(float(shell["top"])) > tolerance
        or abs(float(shell["right"]) - viewport_width) > tolerance
        or abs(float(shell["bottom"]) - viewport_height) > tolerance
    ):
        raise AssertionError(f"Application shell did not consume {label}: {measurements!r}")
    if (
        float(main["width"]) <= 0
        or abs(float(main["right"]) - float(shell["right"])) > tolerance
        or abs(float(main["top"]) - (float(shell["top"]) + shell_padding_top)) > tolerance
        or abs(float(main["bottom"]) - (float(shell["bottom"]) - shell_padding_bottom)) > tolerance
        or abs(float(route_layer["left"]) - float(main["left"])) > tolerance
        or abs(float(route_layer["top"]) - float(main["top"])) > tolerance
        or abs(float(route_layer["width"]) - float(main["width"])) > tolerance
        or float(route_layer["height"]) + tolerance < float(main["height"])
    ):
        raise AssertionError(f"Main route layer did not consume {label}: {measurements!r}")
    route = measurements.get("route")
    if route_selector and (
        not isinstance(route, dict)
        or abs(float(route["width"]) - float(route_layer["width"])) > tolerance
        or float(route["height"]) + tolerance < float(main["height"])
    ):
        raise AssertionError(f"Maintained route did not consume {label}: {measurements!r}")
    assert_no_horizontal_overflow(session_id, label)


def assert_dialog_viewport_bounds(session_id: str, label: str) -> None:
    wait_for_layout_stable(session_id)
    measurements = execute(
        session_id,
        """
        const dialog = document.querySelector("dialog[open]");
        const card = dialog?.querySelector(":scope > section");
        const body = card?.querySelector(":scope > div:last-child");
        if (!(dialog instanceof HTMLDialogElement) || !(card instanceof HTMLElement) || !(body instanceof HTMLElement)) {
          return null;
        }
        const d = dialog.getBoundingClientRect();
        const c = card.getBoundingClientRect();
        return {
          viewportWidth: document.documentElement.clientWidth,
          viewportHeight: document.documentElement.clientHeight,
          dialog: { left: d.left, top: d.top, right: d.right, bottom: d.bottom },
          card: { left: c.left, top: c.top, right: c.right, bottom: c.bottom },
          bodyOverflowY: getComputedStyle(body).overflowY,
        };
        """,
    )
    if not isinstance(measurements, dict):
        raise AssertionError(f"Could not measure {label}: {measurements!r}")
    dialog = measurements["dialog"]
    card = measurements["card"]
    tolerance = 2
    if (
        abs(float(dialog["left"])) > tolerance
        or abs(float(dialog["top"])) > tolerance
        or abs(float(dialog["right"]) - float(measurements["viewportWidth"])) > tolerance
        or abs(float(dialog["bottom"]) - float(measurements["viewportHeight"])) > tolerance
        or float(card["left"]) < -tolerance
        or float(card["top"]) < -tolerance
        or float(card["right"]) > float(measurements["viewportWidth"]) + tolerance
        or float(card["bottom"]) > float(measurements["viewportHeight"]) + tolerance
        or measurements["bodyOverflowY"] not in ("auto", "scroll")
    ):
        raise AssertionError(f"Dialog escaped the viewport or lost its scroll owner in {label}: {measurements!r}")
    assert_no_horizontal_overflow(session_id, label)


def find_workspace_tab(session_id: str, label: str, deadline: float) -> str:
    return webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
        f"//button[@role='tab' and normalize-space(.)={json.dumps(label)}]",
        deadline,
    )


def assert_flashcards_tabs(session_id: str, deadline: float) -> None:
    expected = ["Decks", "Search/Filter"]
    for label in expected:
        find_workspace_tab(session_id, label, deadline)
    labels = execute(
        session_id,
        """
        return [...document.querySelectorAll(
          "[role='tablist'][aria-label='Flashcards workspaces'] [role='tab']"
        )].map((tab) => tab.getAttribute('aria-label') || (tab.textContent || '').trim());
        """,
    )
    if labels != expected:
        raise AssertionError(
            f"Flashcards must expose exactly Decks and Search/Filter: {labels!r}"
        )


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


def assert_brand_and_fresh_font_defaults(session_id: str, label: str) -> None:
    measurement = execute(
        session_id,
        """
        const visible = (element) => {
          if (!(element instanceof HTMLElement)) return false;
          const bounds = element.getBoundingClientRect();
          const style = getComputedStyle(element);
          return bounds.width > 0 && bounds.height > 0 &&
            style.display !== 'none' && style.visibility !== 'hidden';
        };
        const lockups = [...document.querySelectorAll(
          "[data-artwork-slot='brand.logo.sidebar'][aria-label='ostudy']"
        )].filter(visible);
        const mascots = lockups
          .map((lockup) => lockup.querySelector("[data-artwork-slot='brand.logo.compact'] img"))
          .filter(visible);
        const root = document.documentElement;
        return {
          visibleLockups: lockups.length,
          lockupText: lockups.map((lockup) => (lockup.textContent || '').trim()),
          visibleMascots: mascots.length,
          mascotLoaded: mascots.every((image) => image.complete && image.naturalWidth > 0),
          mascotDimensions: mascots.map((image) => [image.naturalWidth, image.naturalHeight]),
          mottoPresent: (document.body.innerText || '').includes('Learn with intent'),
          uiFont: root.style.getPropertyValue('--ui-font-family').trim(),
          cardFont: root.style.getPropertyValue('--card-font-family').trim(),
        };
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Could not inspect brand and typography in {label}: {measurement!r}")
    if (
        int(measurement.get("visibleLockups", 0)) != 1
        or measurement.get("lockupText") != ["ostudy"]
        or int(measurement.get("visibleMascots", 0)) != 1
        or measurement.get("mascotLoaded") is not True
        or measurement.get("mascotDimensions") != [[1254, 1254]]
        or measurement.get("mottoPresent") is not False
    ):
        raise AssertionError(f"Brand lockup drifted in {label}: {measurement!r}")
    if (
        not is_expected_plex_family(measurement.get("uiFont"))
        or not is_expected_plex_family(measurement.get("cardFont"))
    ):
        raise AssertionError(f"Fresh-profile IBM Plex defaults drifted in {label}: {measurement!r}")


def wait_for_window_control(
    session_id: str,
    label: str,
    pressed: bool,
    deadline: float,
) -> str:
    last: Any = None
    while time.monotonic() < deadline:
        last = execute(
            session_id,
            """
            const expected = arguments[0];
            const controls = [...document.querySelectorAll(
              "[role='group'][aria-label='Window controls'] button[aria-label]"
            )].map((button) => {
              const style = getComputedStyle(button);
              return {
                label: button.getAttribute("aria-label"),
                disabled: button.disabled,
                pressed: button.getAttribute("aria-pressed"),
                visible: style.display !== "none" && style.visibility !== "hidden" &&
                  button.getBoundingClientRect().width > 0
              };
            });
            const button = controls.find((candidate) => candidate.label === expected);
            return {
              disabled: button?.disabled ?? null,
              pressed: button?.pressed ?? null,
              visible: button?.visible ?? false,
              controls,
              activeLabel: document.activeElement?.getAttribute("aria-label") ?? null,
              layoutMode: document.documentElement.dataset.layoutMode ?? null
            };
            """,
            [label],
        )
        if (
            isinstance(last, dict)
            and last.get("disabled") is False
            and last.get("visible") is True
            and last.get("pressed") == ("true" if pressed else "false")
        ):
            return webdriver.find_element(
                session_id,
                "css selector",
                f"button[aria-label={json.dumps(label)}]",
                deadline,
            )
        time.sleep(0.05)
    raise AssertionError(f"Window control {label!r} did not reach state {pressed}: {last!r}")


def activate_window_control_with_keyboard(
    session_id: str,
    label: str,
    pressed: bool,
    deadline: float,
) -> dict[str, Any]:
    element = wait_for_window_control(session_id, label, pressed, deadline)
    initial_window_rect = webdriver.request("GET", f"/session/{session_id}/window/rect")
    focused = execute(
        session_id,
        "arguments[0].focus(); return document.activeElement === arguments[0];",
        [{webdriver.ELEMENT_KEY: element}],
    )
    if focused is not True:
        raise AssertionError(f"Window control {label!r} could not receive keyboard focus")
    # WebKitDriver intermittently drops global W3C key-action sequences even
    # when the DOM focus assertion above succeeds. Address the already-resolved
    # button directly through Element Send Keys so this remains a real native
    # keyboard activation without relying on WebKitDriver's global focus bridge.
    token = f"window-control-{time.monotonic_ns()}"
    arm_window_control_keyboard_probe(session_id, element, token)
    # Space activates an HTML button on keyup, so the complete trusted key
    # sequence is observed before the click can trigger a native-state rerender.
    send_element_key(session_id, element, SPACE_KEY)
    probe = wait_for_window_control_keyboard_probe(session_id, token, deadline)
    probe["initialWindowRect"] = initial_window_rect
    return probe


def wait_for_window_control_transition(
    session_id: str,
    label: str,
    pressed: bool,
    keyboard_probe: dict[str, Any],
    deadline: float,
) -> str:
    try:
        return wait_for_window_control(session_id, label, pressed, deadline)
    except Exception as error:
        try:
            final_window_rect: Any = webdriver.request(
                "GET", f"/session/{session_id}/window/rect"
            )
        except Exception as rect_error:
            final_window_rect = {"error": repr(rect_error)}
        raise AssertionError(
            f"{error}; trustedKeyboardProbe={keyboard_probe!r}; "
            f"finalWindowRect={final_window_rect!r}"
        ) from error


def verify_native_window_controls(session_id: str, deadline: float) -> None:
    open_study_guides(session_id, deadline, "resized desktop viewport")
    route_selector = "[aria-label='Study Guides']"
    keyboard_probe = activate_window_control_with_keyboard(
        session_id, "Maximize window", False, deadline
    )
    wait_for_window_control_transition(
        session_id, "Restore window", True, keyboard_probe, deadline
    )
    assert_viewport_consumption(
        session_id, "maximized desktop viewport", route_selector
    )
    keyboard_probe = activate_window_control_with_keyboard(
        session_id, "Restore window", True, deadline
    )
    wait_for_window_control_transition(
        session_id, "Maximize window", False, keyboard_probe, deadline
    )
    assert_viewport_consumption(
        session_id, "restored desktop viewport after maximize", route_selector
    )

    keyboard_probe = activate_window_control_with_keyboard(
        session_id, "Enter full screen", False, deadline
    )
    wait_for_window_control_transition(
        session_id, "Exit full screen", True, keyboard_probe, deadline
    )
    assert_viewport_consumption(
        session_id, "full-screen desktop viewport", route_selector
    )
    keyboard_probe = activate_window_control_with_keyboard(
        session_id, "Exit full screen", True, deadline
    )
    wait_for_window_control_transition(
        session_id, "Enter full screen", False, keyboard_probe, deadline
    )
    assert_viewport_consumption(
        session_id, "desktop viewport after exiting full screen", route_selector
    )

    set_window_rect(session_id, 1440, 900)
    wait_for_layout_mode(session_id, "wide", deadline)
    assert_viewport_consumption(
        session_id, "resized desktop viewport after native states", route_selector
    )


def open_study_guides(
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
    assert_viewport_consumption(session_id, label, "[aria-label='Study Guides']")


def open_study_guides_and_return(
    session_id: str,
    deadline: float,
    label: str,
) -> None:
    open_study_guides(session_id, deadline, label)

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
    assert_brand_and_fresh_font_defaults(session_id, "compact layout")
    assert_navigation_labels(
        session_id,
        ["Flashcards", "Study Guides", "Documents", "Analytics"],
        "compact",
    )
    open_study_guides_and_return(
        session_id,
        deadline,
        "compact Study Guides destination",
    )
    assert_no_horizontal_overflow(session_id, "compact navigation")

    more_count = execute(
        session_id,
        "return document.querySelectorAll(\"button[data-testid='nav-more']\").length;",
    )
    if int(more_count) != 0:
        raise AssertionError("Compact navigation still exposes the removed More destination")

    documents = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-documents']",
        deadline,
    )
    webdriver.click(session_id, documents)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Documents", deadline)
    wait_for_overlay_inactive(session_id, deadline, "compact Documents assertions")
    assert_no_horizontal_overflow(session_id, "compact Documents screen")

    settings = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='app-settings']",
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
    webdriver.find_element(
        session_id,
        "css selector",
        "main [role='tablist'][aria-label='Flashcards workspaces']",
        deadline,
    )
    assert_flashcards_tabs(session_id, deadline)
    assert_no_horizontal_overflow(session_id, "compact Flashcards workspace")


def verify_dialog_focus(session_id: str, deadline: float) -> None:
    new_deck = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Create a new deck' or normalize-space(.)='New deck']",
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
    assert_dialog_viewport_bounds(session_id, "responsive create-deck dialog")

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
    wait_for_active_label(session_id, "Create a new deck", deadline)


def verify_wide_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1440, 900)
    wait_for_layout_mode(session_id, "wide", deadline)
    assert_brand_and_fresh_font_defaults(session_id, "wide layout")
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


def verify_study_guides_checkpoint_after_restart(
    application: Path,
    session_id: str,
) -> str:
    study_guides = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-study-guides']",
        webdriver.deadline_after(),
    )
    webdriver.click(session_id, study_guides)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Study Guides",
        webdriver.deadline_after(),
    )
    checkpoint: Any = None
    deadline = webdriver.deadline_after()
    while time.monotonic() < deadline:
        checkpoint = execute_async(
            session_id,
            """
            const done = arguments[arguments.length - 1];
            window.__TAURI_INTERNALS__.invoke('get_ui_checkpoint')
              .then((value) => done(value))
              .catch((error) => done({ error: String(error) }));
            """,
        )
        if isinstance(checkpoint, dict) and checkpoint.get("section") == "study-guides":
            break
        time.sleep(0.1)
    else:
        raise AssertionError(
            f"Study Guides checkpoint was not durably saved before restart: {checkpoint!r}"
        )
    webdriver.delete_session(session_id)

    restarted = webdriver.create_session(application)
    webdriver.verify_shell(restarted, webdriver.deadline_after())
    webdriver.wait_for_element_text(
        restarted,
        "css selector",
        "main h1",
        "Study Guides",
        webdriver.deadline_after(),
    )
    webdriver.find_element(
        restarted,
        "css selector",
        "button[data-testid='nav-study-guides'][aria-current='page']",
        webdriver.deadline_after(),
    )
    return restarted


def verify_medium_flashcards_layout(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1000, 800)
    wait_for_layout_mode(session_id, "medium", deadline)
    assert_brand_and_fresh_font_defaults(session_id, "medium layout")
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

    search = find_workspace_tab(session_id, "Search/Filter", deadline)
    webdriver.click(session_id, search)
    webdriver.find_element(
        session_id,
        "css selector",
        "main section[aria-label='Card filters']",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "medium Search/Filter workspace")

    decks = find_workspace_tab(session_id, "Decks", deadline)
    webdriver.click(session_id, decks)


def verify_deck_detail_header(
    session_id: str,
    deck_name: str,
    deck_description: str,
) -> None:
    placements = (
        (
            "wide",
            1440,
            900,
            ["Practice", "Export", f"Archive {deck_name}", "More actions"],
            [f"Delete {deck_name}"],
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
        set_window_rect(session_id, width, height)
        snapshot = None
        while time.monotonic() < mode_deadline:
            snapshot = execute(
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
                const titleLine = heading?.parentElement;
                const titleRow = titleLine?.parentElement;
                if (!(heading instanceof HTMLElement)
                    || !(header instanceof HTMLElement)
                    || !(back instanceof HTMLElement)
                    || !(description instanceof HTMLElement)
                    || !(actions instanceof HTMLElement)
                    || !(titleRow instanceof HTMLElement)) return null;
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
                  titleRow: rect(titleRow),
                  heading: rect(heading),
                  description: rect(description),
                  actions: rect(actions),
                };
                const headerRect = geometry.header;
                const contained = [back, heading, description, actions].every((element) => {
                  const bounds = element.getBoundingClientRect();
                  return bounds.left >= headerRect.left - 1
                    && bounds.right <= headerRect.right + 1;
                });
                const openTooltips = [...header.querySelectorAll('[role="tooltip"]')];
                const tooltipDisplays = openTooltips.map((tooltip) => tooltip.style.display);
                let persistentHeaderScrollWidth;
                let persistentHeaderClientWidth;
                try {
                  openTooltips.forEach((tooltip) => {
                    tooltip.style.display = 'none';
                  });
                  persistentHeaderScrollWidth = header.scrollWidth;
                  persistentHeaderClientWidth = header.clientWidth;
                } finally {
                  openTooltips.forEach((tooltip, index) => {
                    tooltip.style.display = tooltipDisplays[index];
                  });
                }
                const descendantOverflow = [...header.querySelectorAll('*')]
                  .map((element) => {
                    const bounds = element.getBoundingClientRect();
                    const style = getComputedStyle(element);
                    return {
                      tag: element.tagName.toLowerCase(),
                      className: element.className?.toString() || '',
                      role: element.getAttribute('role'),
                      ariaLabel: element.getAttribute('aria-label'),
                      position: style.position,
                      overflowX: style.overflowX,
                      left: bounds.left,
                      right: bounds.right,
                      clientWidth: element.clientWidth,
                      scrollWidth: element.scrollWidth,
                      text: (element.textContent || '').trim().slice(0, 120),
                    };
                  })
                  .filter((entry) =>
                    entry.scrollWidth > entry.clientWidth + 1
                    || entry.left < headerRect.left - 1
                    || entry.right > headerRect.right + 1
                  );
                const above = (upper, lower) => upper.bottom <= lower.top + 1;
                return {
                  mode: actions.dataset.layoutMode,
                  labels,
                  ordered:
                    Boolean(back.compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING)
                    && Boolean(heading.compareDocumentPosition(description) & Node.DOCUMENT_POSITION_FOLLOWING)
                    && Boolean(description.compareDocumentPosition(actions) & Node.DOCUMENT_POSITION_FOLLOWING),
                  visuallyOrdered:
                    above(geometry.back, geometry.titleRow)
                    && above(geometry.heading, geometry.description)
                    && above(geometry.description, geometry.actions),
                  titleRowFullWidth:
                    Math.abs(geometry.titleRow.left - headerRect.left) <= 1
                    && Math.abs(geometry.titleRow.right - headerRect.right) <= 1
                    && geometry.titleRow.width >= headerRect.width - 2,
                  geometry,
                  contained,
                  descendantOverflow,
                  excludedOpenTooltipCount: openTooltips.length,
                  persistentHeaderScrollWidth,
                  persistentHeaderClientWidth,
                  noHeaderOverflow:
                    persistentHeaderScrollWidth <= persistentHeaderClientWidth,
                  noDocumentOverflow:
                    document.documentElement.scrollWidth <= document.documentElement.clientWidth,
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
                and snapshot.get("titleRowFullWidth") is True
                and snapshot.get("contained") is True
                and snapshot.get("noHeaderOverflow") is True
                and snapshot.get("noDocumentOverflow") is True
            ):
                break
            time.sleep(0.1)
        else:
            raise AssertionError(
                f"Deck detail header failed {mode} visual order, width, placement, or containment: {snapshot!r}"
            )

        more_actions = webdriver.find_element(
            session_id,
            "css selector",
            "main header button[aria-label='More actions']",
            mode_deadline,
        )
        webdriver.click(session_id, more_actions)
        overflow_items = execute(
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
        send_key(session_id, ESCAPE_KEY)
        wait_for_absent(
            session_id,
            "css selector",
            '[role="menu"][aria-label="More actions"]',
            mode_deadline,
        )


def verify_deck_scoped_authoring(session_id: str, deadline: float) -> None:
    deck_name = "Cardiology pharmacology respiratory physiology comprehensive review collection"
    deck_description = (
        "A comprehensive deck description that verifies the detail header remains in one flow."
    )
    new_deck = webdriver.find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Create a new deck' or normalize-space(.)='New deck']",
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
    webdriver.set_input_text(
        session_id,
        "css selector",
        "dialog textarea",
        deck_description,
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
    verify_deck_detail_header(session_id, deck_name, deck_description)
    set_window_rect(session_id, 1000, 800)

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
        verify_native_window_controls(session_id, webdriver.deadline_after(45))
        session_id = verify_study_guides_checkpoint_after_restart(application, session_id)
        verify_medium_flashcards_layout(session_id, webdriver.deadline_after(45))
        verify_deck_scoped_authoring(session_id, webdriver.deadline_after(45))
        print(
            "Native UI platform smoke passed: compact, labeled medium, and wide navigation and branding; "
            "fresh-profile IBM Plex interface/card defaults; "
            "two-workspace Flashcards containment; vertical responsive deck-detail actions; "
            "deck-scoped authoring; no horizontal overflow; "
            "maintained-route viewport consumption through resize, maximize/restore, and full-screen entry/exit; "
            "dialog focus containment; "
            "Study Guides checkpoint restart; Escape dismissal; and focus restoration."
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
