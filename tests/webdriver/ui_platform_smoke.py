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
# The same five destinations on every screen size; Search and Settings sit in the header.
MAIN_NAVIGATION_LABELS = ["Home", "Library", "Review", "Tutor", "Progress"]
# Library's outer page, which hosts Decks, Documents and Study guides below its view switcher.
LIBRARY_ROUTE_SELECTOR = "[data-testid='library']"
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
        const dialogs = [...document.querySelectorAll("[role='dialog'][aria-modal='true'], [role='alertdialog'][aria-modal='true']")]
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
        // Wider screens stack the header above main; a phone's header is fixed over the padding.
        const header = shell?.querySelector(":scope > header[data-slot='app-header']");
        const headerInFlow = header instanceof HTMLElement && getComputedStyle(header).position !== "fixed";
        const contentBox = main instanceof HTMLElement ? {
          left: main.getBoundingClientRect().left + main.clientLeft,
          top: main.getBoundingClientRect().top + main.clientTop,
          width: main.clientWidth,
          height: main.clientHeight,
        } : null;
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
          shellPaddingRight: Number.parseFloat(shellStyle?.paddingRight || "0") || 0,
          shellPaddingLeft: Number.parseFloat(shellStyle?.paddingLeft || "0") || 0,
          headerBottom: headerInFlow ? header.getBoundingClientRect().bottom : null,
          contentBox,
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
    content_box = measurements.get("contentBox")
    if not all(isinstance(value, dict) for value in (shell, main, route_layer, content_box)):
        raise AssertionError(f"Missing shell geometry for {label}: {measurements!r}")
    viewport_width = float(measurements.get("viewportWidth", 0))
    viewport_height = float(measurements.get("viewportHeight", 0))
    shell_padding_top = float(measurements.get("shellPaddingTop", 0))
    shell_padding_bottom = float(measurements.get("shellPaddingBottom", 0))
    shell_padding_right = float(measurements.get("shellPaddingRight", 0))
    shell_padding_left = float(measurements.get("shellPaddingLeft", 0))
    header_bottom = measurements.get("headerBottom")
    tolerance = 2
    if (
        abs(float(shell["left"])) > tolerance
        or abs(float(shell["top"])) > tolerance
        or abs(float(shell["right"]) - viewport_width) > tolerance
        or abs(float(shell["bottom"]) - viewport_height) > tolerance
    ):
        raise AssertionError(f"Application shell did not consume {label}: {measurements!r}")
    # Main starts below an in-flow header, or at the shell's padding when the header is fixed.
    expected_main_top = (
        float(header_bottom)
        if header_bottom is not None
        else float(shell["top"]) + shell_padding_top
    )
    if (
        float(main["width"]) <= 0
        or abs(float(main["left"]) - (float(shell["left"]) + shell_padding_left)) > tolerance
        or abs(float(main["right"]) - (float(shell["right"]) - shell_padding_right)) > tolerance
        or abs(float(main["top"]) - expected_main_top) > tolerance
        or abs(float(main["bottom"]) - (float(shell["bottom"]) - shell_padding_bottom)) > tolerance
        or abs(float(route_layer["left"]) - float(content_box["left"])) > tolerance
        or abs(float(route_layer["top"]) - float(content_box["top"])) > tolerance
        or abs(float(route_layer["width"]) - float(content_box["width"])) > tolerance
        or float(route_layer["height"]) + tolerance < float(content_box["height"])
    ):
        raise AssertionError(f"Main route layer did not consume {label}: {measurements!r}")
    route = measurements.get("route")
    if route_selector and (
        not isinstance(route, dict)
        or abs(float(route["width"]) - float(route_layer["width"])) > tolerance
        or float(route["height"]) + tolerance < float(content_box["height"])
    ):
        raise AssertionError(f"Maintained route did not consume {label}: {measurements!r}")
    assert_no_horizontal_overflow(session_id, label)


def assert_dialog_viewport_bounds(session_id: str, label: str) -> None:
    wait_for_layout_stable(session_id)
    measurements = execute(
        session_id,
        """
        const dialog = [...document.querySelectorAll("[data-slot='dialog-overlay']")].at(-1);
        const card = [...document.querySelectorAll("[data-overlay-id][aria-modal='true']")].at(-1);
        const body = card?.querySelector("[data-slot='dialog-body']");
        if (!(dialog instanceof HTMLElement) || !(card instanceof HTMLElement) || !(body instanceof HTMLElement)) {
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


def assert_navigation_labels(
    session_id: str,
    expected: list[str],
    label: str,
) -> None:
    labels = execute(
        session_id,
        """
        return [...document.querySelectorAll("nav[aria-label='Main navigation'] button")]
          .map((button) => (button.querySelector("[data-slot='nav-label']")?.textContent || '').trim());
        """,
    )
    if labels != expected:
        raise AssertionError(f"Unexpected {label} navigation order: {labels!r}")


def assert_shell_placement(session_id: str, layout: str) -> None:
    """A phone has a bottom bar; wider screens carry the navigation in the header."""
    placement = execute(
        session_id,
        """
        const header = document.querySelector("header[data-slot='app-header']");
        const navigation = document.querySelector("nav[aria-label='Main navigation']");
        const visible = (element) => element instanceof HTMLElement
          && element.getBoundingClientRect().width > 0
          && getComputedStyle(element).visibility !== 'hidden';
        return {
          navigationCount: document.querySelectorAll("nav[aria-label='Main navigation']").length,
          inHeader: Boolean(header && navigation && header.contains(navigation)),
          bottom: navigation?.dataset.slot === 'bottom-navigation',
          search: visible(header?.querySelector("button[data-testid='app-search']")),
          settings: visible(header?.querySelector("button[data-testid='app-settings']")),
          settingsInNavigation: Boolean(navigation?.querySelector("[data-testid*='settings']")),
        };
        """,
    )
    expected = {
        "navigationCount": 1,
        "inHeader": layout != "compact",
        "bottom": layout == "compact",
        "search": True,
        "settings": True,
        "settingsInNavigation": False,
    }
    if placement != expected:
        raise AssertionError(f"The {layout} shell placed its navigation or header actions wrongly: {placement!r}")


def assert_brand_and_fresh_font_defaults(
    session_id: str, label: str, *, mark_only: bool = False
) -> None:
    # Medium widths show the raccoon mark alone in the header, without the wordmark.
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
        const markOnly = arguments[0];
        const header = document.querySelector("[data-testid='app-header']");
        const mascots = (markOnly
          ? [...(header?.querySelectorAll("[data-artwork-slot='brand.logo.compact'] img") ?? [])]
          : lockups.map((lockup) => lockup.querySelector("[data-artwork-slot='brand.logo.compact'] img"))
        ).filter(visible);
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
        [mark_only],
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Could not inspect brand and typography in {label}: {measurement!r}")
    expected_lockups = 0 if mark_only else 1
    expected_lockup_text = [] if mark_only else ["ostudy"]
    if (
        int(measurement.get("visibleLockups", 0)) != expected_lockups
        or measurement.get("lockupText") != expected_lockup_text
        or int(measurement.get("visibleMascots", 0)) != 1
        or measurement.get("mascotLoaded") is not True
        # The 64 px raccoon head; a 2x screen loads the 128 px file, reported as 64 once density-corrected.
        or measurement.get("mascotDimensions") not in ([[64, 64]], [[128, 128]])
        or measurement.get("mottoPresent") is not False
    ):
        raise AssertionError(f"Brand lockup drifted in {label}: {measurement!r}")
    if (
        not is_expected_plex_family(measurement.get("uiFont"))
        or not is_expected_plex_family(measurement.get("cardFont"))
    ):
        raise AssertionError(f"Fresh-profile IBM Plex defaults drifted in {label}: {measurement!r}")


def assert_utilities_restyle_document_defaults(session_id: str) -> None:
    # Utilities must beat the reset and the heading and icon defaults in the bundled
    # stylesheet: the layer order statement has to load before any CSS Module.
    measurement = execute(
        session_id,
        """
        const host = document.createElement('div');
        host.setAttribute('aria-hidden', 'true');
        host.style.cssText = 'position: fixed; left: -10000px; top: 0;';
        host.innerHTML = `
          <div data-probe="parent"><button data-probe="button" class="tw:text-small tw:font-semibold">Probe</button></div>
          <h2 data-probe="plain">Plain</h2>
          <h2 data-probe="utility" class="tw:font-medium">Utility</h2>
          <svg data-probe="icon" data-icon="probe" width="20" height="20"></svg>
          <span class="tw:[&_svg:not([class*='size-'])]:size-3.5">
            <svg data-probe="sized" data-icon="probe" width="20" height="20"></svg>
          </span>`;
        // The app's CSP drops style attributes that the HTML parser creates, so set
        // inline styles through CSSOM, as React does for Icon's --icon-size.
        host.querySelector("[data-probe='parent']").style.fontSize = '20px';
        for (const name of ['icon', 'sized']) {
          host.querySelector(`[data-probe='${name}']`).style.setProperty('--icon-size', '2rem');
        }
        document.body.append(host);
        const style = (name) => getComputedStyle(host.querySelector(`[data-probe='${name}']`));
        const result = {
          rootFontSize: parseFloat(getComputedStyle(document.documentElement).fontSize),
          buttonSize: parseFloat(style('button').fontSize),
          buttonWeight: style('button').fontWeight,
          plainWeight: style('plain').fontWeight,
          utilityWeight: style('utility').fontWeight,
          iconWidth: parseFloat(style('icon').width),
          sizedWidth: parseFloat(style('sized').width),
        };
        host.remove();
        return result;
        """,
    )
    if not isinstance(measurement, dict):
        raise AssertionError(f"Could not inspect document defaults: {measurement!r}")
    root = float(measurement.get("rootFontSize") or 0)
    if (
        root <= 0
        or abs(float(measurement.get("buttonSize") or 0) - 0.875 * root) > 0.5
        or str(measurement.get("buttonWeight")) != "600"
        or str(measurement.get("plainWeight")) != "600"
        or str(measurement.get("utilityWeight")) != "500"
        or abs(float(measurement.get("iconWidth") or 0) - 2 * root) > 0.5
        or abs(float(measurement.get("sizedWidth") or 0) - 0.875 * root) > 0.5
    ):
        raise AssertionError(f"Utilities no longer win the cascade over the reset and document defaults: {measurement!r}")


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
    route_selector = LIBRARY_ROUTE_SELECTOR
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
    webdriver.open_library_view(session_id, "study-guides", deadline)
    webdriver.wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Study Guides",
        deadline,
    )
    assert_viewport_consumption(session_id, label, LIBRARY_ROUTE_SELECTOR)


def open_study_guides_and_return(
    session_id: str,
    deadline: float,
    label: str,
) -> None:
    open_study_guides(session_id, deadline, label)

    # Library's switcher returns to the Workbench without leaving Library.
    workbench = webdriver.find_element(
        session_id,
        "css selector",
        "main nav[aria-label='Library'] button[data-testid='library-view-decks']",
        deadline,
    )
    webdriver.click(session_id, workbench)
    webdriver.find_element(session_id, "css selector", webdriver.WORKBENCH_SELECTOR, deadline)


def verify_compact_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 390, 700)
    wait_for_layout_mode(session_id, "compact", deadline)
    assert_brand_and_fresh_font_defaults(session_id, "compact layout")
    assert_navigation_labels(session_id, MAIN_NAVIGATION_LABELS, "compact")
    assert_shell_placement(session_id, "compact")
    webdriver.verify_fresh_home(session_id, deadline)
    assert_no_horizontal_overflow(session_id, "compact first-run Home")
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
        "main nav[aria-label='Library'] button[data-testid='library-view-documents']",
        deadline,
    )
    webdriver.click(session_id, documents)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Documents", deadline)
    wait_for_overlay_inactive(session_id, deadline, "compact Documents assertions")
    assert_no_horizontal_overflow(session_id, "compact Documents screen")

    review = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-review']",
        deadline,
    )
    webdriver.click(session_id, review)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Review", deadline)
    webdriver.find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='review-today']//h2[normalize-space(.)='Nothing due today']",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "compact Review screen")

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

    # Search in the header opens Library's Workbench with its search focused.
    search = webdriver.find_element(
        session_id,
        "css selector",
        "header button[data-testid='app-search']",
        deadline,
    )
    webdriver.click(session_id, search)
    webdriver.verify_search_workspace_selected(session_id, deadline)
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-library'][aria-current='page']",
        deadline,
    )
    assert_no_horizontal_overflow(session_id, "compact Workbench search")

    # Phones show one Workbench page at a time; the back button leads to its tree.
    back = webdriver.find_element(
        session_id,
        "css selector",
        "main [data-testid='library-workbench'] button[aria-label='Library']",
        deadline,
    )
    webdriver.click(session_id, back)
    webdriver.find_element(
        session_id,
        "xpath",
        f"{webdriver.WORKBENCH_TREE_XPATH}//button[.//span[normalize-space(.)='All cards']]",
        deadline,
    )
    if execute(session_id, "return document.querySelectorAll(\"main section[aria-label='Cards']\").length;") != 0:
        raise AssertionError("Compact Workbench showed its card list beside the tree")
    assert_no_horizontal_overflow(session_id, "compact Workbench tree")


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
        "[role='dialog'] input[data-autofocus='true']",
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
    wait_for_active_label(session_id, "New deck", deadline)


def verify_wide_navigation(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1440, 900)
    wait_for_layout_mode(session_id, "wide", deadline)
    assert_brand_and_fresh_font_defaults(session_id, "wide layout")
    assert_utilities_restyle_document_defaults(session_id)
    assert_navigation_labels(session_id, MAIN_NAVIGATION_LABELS, "wide")
    assert_shell_placement(session_id, "wide")
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
    webdriver.open_library_view(session_id, "study-guides", webdriver.deadline_after())
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
        "button[data-testid='nav-library'][aria-current='page']",
        webdriver.deadline_after(),
    )
    webdriver.find_element(
        restarted,
        "css selector",
        "button[data-testid='library-view-study-guides'][aria-current='page']",
        webdriver.deadline_after(),
    )
    return restarted


def verify_medium_workbench_layout(session_id: str, deadline: float) -> None:
    set_window_rect(session_id, 1000, 800)
    wait_for_layout_mode(session_id, "medium", deadline)
    assert_brand_and_fresh_font_defaults(session_id, "medium layout", mark_only=True)
    assert_navigation_labels(session_id, MAIN_NAVIGATION_LABELS, "medium")
    assert_shell_placement(session_id, "medium")
    open_study_guides_and_return(session_id, deadline, "medium Study Guides destination")
    webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-library'][aria-current='page']",
        deadline,
    )
    webdriver.find_element(session_id, "css selector", webdriver.WORKBENCH_SELECTOR, deadline)
    panes = execute(
        session_id,
        """
        const workbench = document.querySelector("main [data-testid='library-workbench']");
        const tree = workbench?.querySelector("nav[aria-label='Library']");
        const list = workbench?.querySelector("section[aria-label='Cards']");
        if (!tree || !list) return null;
        return {
          separators: workbench.querySelectorAll("[role='separator'][aria-orientation='vertical']").length,
          sideBySide: tree.getBoundingClientRect().right <= list.getBoundingClientRect().left + 1,
        };
        """,
    )
    if panes != {"separators": 1, "sideBySide": True}:
        raise AssertionError(f"Medium Workbench did not show its tree beside the card list: {panes!r}")

    navigation = execute(
        session_id,
        """
        const labels = [...document.querySelectorAll("nav[aria-label='Main navigation'] [data-slot='nav-label']")];
        return {
          visibleLabels: labels.filter((label) => {
            if (!(label instanceof HTMLElement)) return false;
            const style = getComputedStyle(label);
            return style.display !== 'none' && style.visibility !== 'hidden' && label.getBoundingClientRect().width > 20;
          }).map((label) => (label.textContent || '').trim()),
          clipped: labels
            .filter((label) => label instanceof HTMLElement && label.scrollWidth > label.clientWidth + 1)
            .map((label) => (label.textContent || '').trim()),
        };
        """,
    )
    if not isinstance(navigation, dict):
        raise AssertionError(f"Could not measure medium navigation: {navigation!r}")
    # Medium widths keep the labeled top bar rather than collapsing to icons.
    if navigation.get("visibleLabels") != MAIN_NAVIGATION_LABELS or navigation.get("clipped"):
        raise AssertionError(f"Medium navigation labels are not all visible: {navigation!r}")
    assert_no_horizontal_overflow(session_id, "medium Workbench")


DECK_OVERFLOW_ITEMS = [
    "Bulk add…",
    "Practice with options…",
    "Rename…",
    "Move to subject…",
    "Merge into…",
    "Export…",
    "Archive",
    "Delete deck",
]


def read_deck_overflow(session_id: str, actions_label: str) -> dict | None:
    """Return the open deck-actions overflow surface and its item labels."""
    return execute(
        session_id,
        """
        const label = arguments[0];
        const text = (element) => (element.textContent || '').trim();
        const menu = [...document.querySelectorAll('[role="menu"]')]
          .find((candidate) => candidate.getAttribute('aria-label') === label);
        if (menu) {
          return {
            surface: 'menu',
            items: [...menu.querySelectorAll('[role="menuitem"]')].map(text),
          };
        }
        const sheet = [...document.querySelectorAll('[role="dialog"] [aria-label]')]
          .find((candidate) => candidate.getAttribute('aria-label') === label);
        if (sheet) {
          return {
            surface: 'sheet',
            items: [...sheet.querySelectorAll('button')].map(text),
          };
        }
        return null;
        """,
        [actions_label],
    )


def verify_deck_detail_header(session_id: str, deck_name: str) -> None:
    """A deck's Workbench header keeps Practice, New card and its actions menu at every width.

    The long deck name wraps above the actions rather than pushing them out of
    the header. The rest of the deck's actions live in its overflow: a menu on
    wide and medium screens and a sheet on phones.
    """
    actions_label = f"{deck_name} actions"
    expected_direct = ["Practice", "New card", actions_label]
    for mode, width, height, surface in (
        ("wide", 1440, 900, "menu"),
        ("medium", 1000, 800, "menu"),
        ("compact", 390, 700, "sheet"),
    ):
        mode_deadline = webdriver.deadline_after()
        set_window_rect(session_id, width, height)
        wait_for_layout_mode(session_id, mode, mode_deadline)
        snapshot = None
        while time.monotonic() < mode_deadline:
            snapshot = execute(
                session_id,
                """
                const deckName = arguments[0];
                const heading = document.querySelector("main h1[data-testid='workbench-title']");
                const header = heading?.closest('header');
                if (!(heading instanceof HTMLElement) || !(header instanceof HTMLElement)) return null;
                const titleRow = header.firstElementChild;
                const actions = titleRow?.lastElementChild;
                if (!(actions instanceof HTMLElement) || actions.contains(heading)) return null;
                const labels = [...actions.querySelectorAll('button')]
                  .map((button) => (button.getAttribute('aria-label') || button.textContent || '').trim());
                const headerRect = header.getBoundingClientRect();
                const contained = [heading, actions].every((element) => {
                  const bounds = element.getBoundingClientRect();
                  return bounds.left >= headerRect.left - 1 && bounds.right <= headerRect.right + 1;
                });
                return {
                  title: (heading.textContent || '').trim() === deckName,
                  labels,
                  contained,
                  noHeaderOverflow: header.scrollWidth <= header.clientWidth + 1,
                  noDocumentOverflow:
                    document.documentElement.scrollWidth <= document.documentElement.clientWidth,
                };
                """,
                [deck_name],
            )
            if (
                isinstance(snapshot, dict)
                and snapshot.get("title") is True
                and snapshot.get("labels") == expected_direct
                and snapshot.get("contained") is True
                and snapshot.get("noHeaderOverflow") is True
                and snapshot.get("noDocumentOverflow") is True
            ):
                break
            time.sleep(0.1)
        else:
            raise AssertionError(
                f"Deck header failed {mode} placement or containment: {snapshot!r}"
            )

        more_actions = webdriver.find_element(
            session_id,
            "css selector",
            f"main header button[aria-label={json.dumps(actions_label)}]",
            mode_deadline,
        )
        webdriver.click(session_id, more_actions)
        overflow = None
        while time.monotonic() < mode_deadline:
            overflow = read_deck_overflow(session_id, actions_label)
            if isinstance(overflow, dict) and overflow.get("items"):
                break
            time.sleep(0.1)
        expected_items = DECK_OVERFLOW_ITEMS
        if not isinstance(overflow, dict) or overflow.get("surface") != surface or overflow.get("items") != expected_items:
            raise AssertionError(f"Deck {mode} overflow actions were incorrect: {overflow!r}")
        send_key(session_id, ESCAPE_KEY)
        while time.monotonic() < mode_deadline:
            if read_deck_overflow(session_id, actions_label) is None:
                break
            time.sleep(0.1)
        else:
            raise AssertionError(f"Deck {mode} overflow did not close on Escape")
        wait_for_overlay_inactive(session_id, mode_deadline, f"{mode} deck overflow dismissal")


def verify_deck_scoped_authoring(session_id: str, deadline: float) -> None:
    deck_name = "Cardiology pharmacology respiratory physiology comprehensive review collection"
    webdriver.open_library_view(session_id, "decks", deadline)
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
        "[role='dialog'] input[data-autofocus='true']",
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
    webdriver.wait_for_workbench_title(session_id, deck_name, deadline)
    wait_for_overlay_inactive(session_id, deadline, "new deck assertions")
    verify_deck_detail_header(session_id, deck_name)
    set_window_rect(session_id, 1000, 800)
    wait_for_layout_mode(session_id, "medium", deadline)

    webdriver.find_element(
        session_id,
        "xpath",
        "//main//header//button[normalize-space(.)='New card']",
        deadline,
    )
    webdriver.choose_menu_item(
        session_id, f"{deck_name} actions", "Bulk add…", deadline, trigger_scope="//main//header"
    )
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
        verify_medium_workbench_layout(session_id, webdriver.deadline_after(45))
        verify_deck_scoped_authoring(session_id, webdriver.deadline_after(45))
        print(
            "Native UI platform smoke passed: fresh-library Home; the same five destinations in the "
            "phone bottom bar and the labeled medium and wide header, with Search and Settings in the header; "
            "Library's Workbench, Documents and Study guides pages; Review; "
            "compact, labeled medium, and wide branding; "
            "fresh-profile IBM Plex interface/card defaults; "
            "the Workbench's tree and card panes; responsive deck header actions; "
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
