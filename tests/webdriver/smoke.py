#!/usr/bin/env python3
"""Native Tauri smoke workflow using only the W3C WebDriver protocol."""

from __future__ import annotations

import argparse
import http.client
import json
import os
import re
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

WEBDRIVER_URL = "http://127.0.0.1:4444"
ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf"
STEP_TIMEOUT_SECONDS = 30
ANALYTICS_TIMEOUT_SECONDS = 45
ROOT_DIR = Path(__file__).resolve().parents[2]
TUTORIAL_REGISTRY_PATH = ROOT_DIR / "src/ui/tutorials/tutorialRegistry.ts"
TUTORIAL_STORAGE_KEY = "ostudy.tutorials.v1"
TRANSIENT_CLICK_ERRORS = {"element click intercepted", "element not interactable"}
RETRYABLE_FIND_ERRORS = {"no such element", "stale element reference"}
RETRYABLE_PROFILE_ERRORS = {"no such window", "unknown error"}
PRACTICE_COMPLETE_SELECTOR = "main [data-testid='practice-complete']"
ANALYTICS_SCREEN_SELECTOR = "main section[aria-label='Learning summary']"
# Library hosts these sections as its views. Each keeps its own page heading.
LIBRARY_VIEW_HEADINGS = {
    "overview": "Overview",
    "lessons": "Lessons",
    "decks": "All cards",
    "documents": "Documents",
    "study-guides": "Study Guides",
}
CHART_DATA_SELECTOR = "main section[aria-labelledby='progress-heading'] table"


class WebDriverProtocolError(RuntimeError):
    """Decoded W3C WebDriver failure, including HTTP 4xx/5xx responses."""

    def __init__(self, status: int, error: str, message: str) -> None:
        self.status = status
        self.error = error
        self.message = message
        super().__init__(f"WebDriver {error} (HTTP {status}): {message}")


def deadline_after(seconds: int = STEP_TIMEOUT_SECONDS) -> float:
    return time.monotonic() + seconds


def progress(message: str) -> None:
    print(f"[native-smoke] {message}", flush=True)


def _protocol_error(decoded: Any, status: int) -> WebDriverProtocolError | None:
    if not isinstance(decoded, dict):
        return None
    value = decoded.get("value")
    if not isinstance(value, dict) or not value.get("error"):
        return None
    return WebDriverProtocolError(
        status,
        str(value.get("error")),
        str(value.get("message") or "No WebDriver error message was provided."),
    )


def request(method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        WEBDRIVER_URL + path,
        data=body,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            decoded = json.loads(response.read().decode("utf-8"))
            status = response.status
    except urllib.error.HTTPError as http_error:
        raw = http_error.read().decode("utf-8", errors="replace")
        try:
            decoded = json.loads(raw)
        except json.JSONDecodeError as decode_error:
            raise WebDriverProtocolError(
                http_error.code,
                "http_error",
                raw[:1_000] or http_error.reason,
            ) from decode_error
        protocol_error = _protocol_error(decoded, http_error.code)
        if protocol_error is not None:
            raise protocol_error from http_error
        raise WebDriverProtocolError(
            http_error.code,
            "http_error",
            raw[:1_000] or http_error.reason,
        ) from http_error

    protocol_error = _protocol_error(decoded, status)
    if protocol_error is not None:
        raise protocol_error
    return decoded.get("value") if isinstance(decoded, dict) else decoded


def wait_for_driver(deadline: float) -> None:
    while time.monotonic() < deadline:
        try:
            request("GET", "/status")
            return
        except (
            OSError,
            urllib.error.URLError,
            json.JSONDecodeError,
            WebDriverProtocolError,
        ):
            time.sleep(0.2)
    raise TimeoutError("tauri-driver did not become ready")


def tutorial_versions() -> dict[str, int]:
    """Read the current tutorial ids/versions so automation never races auto-start."""
    source = TUTORIAL_REGISTRY_PATH.read_text(encoding="utf-8")
    pattern = re.compile(
        r'^\s*[A-Za-z0-9_]+:\s*\{\s*\n\s*id:\s*"([^"]+)",\s*\n\s*version:\s*(\d+),',
        re.MULTILINE,
    )
    versions = {module_id: int(version) for module_id, version in pattern.findall(source)}
    if not versions:
        raise RuntimeError(
            f"Could not derive tutorial module versions from {TUTORIAL_REGISTRY_PATH}"
        )
    return versions


def prepare_automation_profile(session_id: str) -> None:
    """Disable contextual tutorials in the disposable native-smoke browser profile."""
    versions = tutorial_versions()
    state = {
        "schemaVersion": 1,
        "completed": versions,
        "dismissed": {},
    }
    deadline = time.monotonic() + 5
    last_error: WebDriverProtocolError | None = None
    while time.monotonic() < deadline:
        try:
            request(
                "POST",
                f"/session/{session_id}/execute/sync",
                {
                    "script": """
                        const key = arguments[0];
                        const state = arguments[1];
                        window.localStorage.setItem(key, JSON.stringify(state));
                        window.location.reload();
                        return true;
                    """,
                    "args": [TUTORIAL_STORAGE_KEY, state],
                },
            )
            return
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_PROFILE_ERRORS:
                raise
            last_error = error
            time.sleep(0.1)
    if last_error is not None:
        raise last_error
    raise RuntimeError("WebDriver automation profile preparation ended without a result")


def create_session(application: Path) -> str:
    result = request(
        "POST",
        "/session",
        {
            "capabilities": {
                "alwaysMatch": {
                    "browserName": "wry",
                    "tauri:options": {"application": str(application)},
                }
            }
        },
    )
    if not isinstance(result, dict):
        raise RuntimeError(f"Unexpected new-session response: {result!r}")
    session_id = str(result.get("sessionId") or "")
    if not session_id:
        raise RuntimeError(f"New-session response did not contain a session id: {result!r}")
    prepare_automation_profile(session_id)
    return session_id


def delete_session(session_id: str) -> None:
    request("DELETE", f"/session/{session_id}")


def find_element(session_id: str, using: str, value: str, deadline: float) -> str:
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            result = request(
                "POST",
                f"/session/{session_id}/element",
                {"using": using, "value": value},
            )
            if isinstance(result, dict) and ELEMENT_KEY in result:
                return str(result[ELEMENT_KEY])
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        except (OSError, json.JSONDecodeError) as error:
            # Element lookup is a read-only W3C operation. A transient HTTP
            # disconnect can be retried safely inside the existing deadline;
            # mutating commands such as clicks and session creation are never
            # retried here because their response could have been lost after
            # the operation was already applied.
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(f"Element was not found: {using}={value}; last error: {last_error}")


def find_elements(
    session_id: str,
    using: str,
    value: str,
    minimum: int,
    deadline: float,
) -> list[str]:
    last_count = 0
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            result = request(
                "POST",
                f"/session/{session_id}/elements",
                {"using": using, "value": value},
            )
            if isinstance(result, list):
                elements = [
                    str(item[ELEMENT_KEY])
                    for item in result
                    if isinstance(item, dict) and ELEMENT_KEY in item
                ]
                last_count = len(elements)
                if len(elements) >= minimum:
                    return elements
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        except (OSError, json.JSONDecodeError) as error:
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected at least {minimum} elements for {using}={value}; "
        f"last count: {last_count}; last error: {last_error}"
    )


def element_exists(session_id: str, using: str, value: str) -> bool:
    """Whether the element is present right now, without waiting for it."""
    result = request("POST", f"/session/{session_id}/elements", {"using": using, "value": value})
    return isinstance(result, list) and len(result) > 0


def element_text(session_id: str, element_id: str) -> str:
    value = request("GET", f"/session/{session_id}/element/{element_id}/text")
    return str(value)


def element_property(session_id: str, element_id: str, name: str) -> Any:
    return request(
        "GET",
        f"/session/{session_id}/element/{element_id}/property/{name}",
    )


def element_enabled(session_id: str, element_id: str) -> bool:
    return request("GET", f"/session/{session_id}/element/{element_id}/enabled") is True


def _element_hit_test(session_id: str, element_id: str) -> dict[str, Any]:
    """Scroll an element into a stable viewport position and identify its top hit target."""
    result = request(
        "POST",
        f"/session/{session_id}/execute/sync",
        {
            "script": """
                const element = arguments[0];
                if (!(element instanceof HTMLElement)) {
                  return { ready: false, reason: 'not_html_element' };
                }
                element.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'auto' });
                const rect = element.getBoundingClientRect();
                const style = getComputedStyle(element);
                const x = Math.min(window.innerWidth - 1, Math.max(0, rect.left + rect.width / 2));
                const y = Math.min(window.innerHeight - 1, Math.max(0, rect.top + rect.height / 2));
                const top = document.elementFromPoint(x, y);
                const disabled = element.matches(':disabled') || element.getAttribute('aria-disabled') === 'true';
                const inViewport = rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.right > 0 && rect.top < window.innerHeight && rect.left < window.innerWidth;
                const visible = style.display !== 'none' && style.visibility !== 'hidden' && style.pointerEvents !== 'none';
                const ownsHit = Boolean(top && (top === element || element.contains(top)));
                return {
                  ready: Boolean(inViewport && visible && !disabled && ownsHit),
                  disabled,
                  display: style.display,
                  visibility: style.visibility,
                  pointerEvents: style.pointerEvents,
                  rect: {
                    left: rect.left,
                    top: rect.top,
                    width: rect.width,
                    height: rect.height
                  },
                  topTag: top?.tagName ?? null,
                  topText: (top?.textContent || '').trim().slice(0, 120),
                  topTestId: top instanceof HTMLElement ? top.dataset.testid ?? null : null,
                  topClass: top instanceof HTMLElement && typeof top.className === 'string' ? top.className.slice(0, 180) : null
                };
            """,
            "args": [{ELEMENT_KEY: element_id}],
        },
    )
    if not isinstance(result, dict):
        raise RuntimeError(f"Unexpected WebDriver hit-test result: {result!r}")
    return result


def _hit_signature(hit: dict[str, Any]) -> tuple[float, float, float, float] | None:
    rect = hit.get("rect")
    if not isinstance(rect, dict):
        return None
    try:
        return tuple(
            round(float(rect[key]), 1)
            for key in ("left", "top", "width", "height")
        )
    except (KeyError, TypeError, ValueError):
        return None


def _arm_click_ack(session_id: str, element_id: str, token: str) -> None:
    """Install a one-shot capture listener that proves the DOM received the click."""
    path = f"/session/{session_id}/execute/sync"
    payload = {
        "script": """
            const element = arguments[0];
            const token = arguments[1];
            if (!(element instanceof Element)) return false;
            element.addEventListener(
              'click',
              () => { window.__OSTUDY_WEBDRIVER_CLICK_ACK__ = token; },
              { once: true, capture: true }
            );
            return true;
        """,
        "args": [{ELEMENT_KEY: element_id}, token],
    }
    try:
        armed = request("POST", path, payload)
    except (OSError, urllib.error.URLError) as error:
        if not _is_transient_disconnect(error):
            raise
        # The measured click has not been sent, and duplicate listeners using the
        # same token preserve the acknowledgement postcondition.
        armed = request("POST", path, payload)
    if armed is not True:
        raise RuntimeError("Could not arm DOM acknowledgement for WebDriver click")


def _wait_for_click_ack(session_id: str, token: str, deadline: float) -> bool:
    """Wait briefly for the target's capture listener to observe a real click event."""
    while time.monotonic() < deadline:
        acknowledged = request(
            "POST",
            f"/session/{session_id}/execute/sync",
            {
                "script": "return window.__OSTUDY_WEBDRIVER_CLICK_ACK__ === arguments[0];",
                "args": [token],
            },
        )
        if acknowledged is True:
            return True
        time.sleep(0.05)
    return False


def _is_transient_disconnect(error: BaseException) -> bool:
    if isinstance(error, (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError)):
        return True
    return isinstance(error, urllib.error.URLError) and isinstance(
        error.reason,
        (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError),
    )


def click(session_id: str, element_id: str) -> None:
    """Click a stable target and require proof that WebKit dispatched the DOM event."""
    deadline = time.monotonic() + 5
    last_error: WebDriverProtocolError | None = None
    last_hit: dict[str, Any] | None = None
    stable_signature: tuple[float, float, float, float] | None = None
    stable_samples = 0
    unacknowledged_clicks = 0

    while time.monotonic() < deadline:
        try:
            hit = _element_hit_test(session_id, element_id)
        except (OSError, urllib.error.URLError) as error:
            if not _is_transient_disconnect(error):
                raise
            stable_signature = None
            stable_samples = 0
            time.sleep(0.1)
            continue
        last_hit = hit
        if hit.get("ready") is not True:
            stable_signature = None
            stable_samples = 0
            time.sleep(0.1)
            continue

        signature = _hit_signature(hit)
        if signature is None:
            stable_signature = None
            stable_samples = 0
            time.sleep(0.1)
            continue
        if signature == stable_signature:
            stable_samples += 1
        else:
            stable_signature = signature
            stable_samples = 1
        if stable_samples < 2:
            time.sleep(0.1)
            continue

        token = f"ostudy-click-{time.monotonic_ns()}-{unacknowledged_clicks}"
        _arm_click_ack(session_id, element_id, token)
        try:
            request("POST", f"/session/{session_id}/element/{element_id}/click", {})
        except (OSError, urllib.error.URLError) as error:
            if not _is_transient_disconnect(error):
                raise
            ack_deadline = min(deadline, time.monotonic() + 0.5)
            if _wait_for_click_ack(session_id, token, ack_deadline):
                return
            raise
        except WebDriverProtocolError as error:
            if error.error not in TRANSIENT_CLICK_ERRORS:
                raise
            last_error = error
            stable_signature = None
            stable_samples = 0
            time.sleep(0.1)
            continue

        ack_deadline = min(deadline, time.monotonic() + 0.5)
        if _wait_for_click_ack(session_id, token, ack_deadline):
            return

        unacknowledged_clicks += 1
        if unacknowledged_clicks >= 2:
            raise RuntimeError(
                "WebDriver reported two successful clicks without DOM acknowledgement. "
                f"Last hit test: {json.dumps(last_hit, sort_keys=True)}"
            )
        stable_signature = None
        stable_samples = 0
        time.sleep(0.1)

    if last_hit is not None and last_hit.get("ready") is not True:
        raise RuntimeError(
            "Element never became hit-testable before click: "
            + json.dumps(last_hit, sort_keys=True)
        )
    if last_error is not None:
        raise WebDriverProtocolError(
            last_error.status,
            last_error.error,
            f"{last_error.message} Last hit test: {json.dumps(last_hit, sort_keys=True)}",
        )
    raise RuntimeError(
        "WebDriver click never reached a stable target: "
        + json.dumps(last_hit, sort_keys=True)
    )


def clear(session_id: str, element_id: str) -> None:
    """Reach the editable field through the same hit-tested focus path as a user.

    Finding a contenteditable in the DOM does not make it interactable inside a
    scrolled workspace. In particular, WebKit may reject Clear before scrolling
    a lower editor into view. Our existing click helper scrolls, checks a stable
    unobscured hit target and verifies the actual DOM click before clearing.
    Never bypass a hidden/covered control with a JavaScript value assignment.
    """
    click(session_id, element_id)
    request("POST", f"/session/{session_id}/element/{element_id}/clear", {})


def send_keys(session_id: str, element_id: str, text: str) -> None:
    click(session_id, element_id)
    request(
        "POST",
        f"/session/{session_id}/element/{element_id}/value",
        {"text": text, "value": list(text)},
    )


def set_input_text(
    session_id: str,
    using: str,
    value: str,
    text: str,
    deadline: float,
) -> str:
    """Enter controlled-input text and prove React/WebKit accepted the value."""
    last_value: str | None = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            attempt_deadline = min(deadline, time.monotonic() + 2)
            element = find_element(session_id, using, value, attempt_deadline)
            clear(session_id, element)
            send_keys(session_id, element, text)
            verify_deadline = min(deadline, time.monotonic() + 2)
            while time.monotonic() < verify_deadline:
                current = element_property(session_id, element, "value")
                last_value = "" if current is None else str(current)
                if last_value == text:
                    return element
                time.sleep(0.1)
        except TimeoutError as error:
            last_error = error
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Controlled input {using}={value} did not accept {text!r}; "
        f"last value: {last_value!r}; last error: {last_error}"
    )


def set_rich_editor_text(
    session_id: str,
    index: int,
    text: str,
    deadline: float,
) -> str:
    """Enter rich-editor text and verify the rendered editable text before saving."""
    selector = "main [contenteditable='true'][aria-label]"
    last_text: str | None = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            attempt_deadline = min(deadline, time.monotonic() + 2)
            editors = find_elements(
                session_id,
                "css selector",
                selector,
                index + 1,
                attempt_deadline,
            )
            element = editors[index]
            clear(session_id, element)
            send_keys(session_id, element, text)
            verify_deadline = min(deadline, time.monotonic() + 2)
            while time.monotonic() < verify_deadline:
                last_text = element_text(session_id, element).strip()
                if last_text == text:
                    return element
                time.sleep(0.1)
        except TimeoutError as error:
            last_error = error
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Rich editor {index} did not accept {text!r}; "
        f"last text: {last_text!r}; last error: {last_error}"
    )


def wait_for_enabled_element(
    session_id: str,
    using: str,
    value: str,
    deadline: float,
) -> str:
    last_enabled: bool | None = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            attempt_deadline = min(deadline, time.monotonic() + 2)
            element = find_element(session_id, using, value, attempt_deadline)
            last_enabled = element_enabled(session_id, element)
            if last_enabled:
                return element
        except TimeoutError as error:
            last_error = error
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.1)
    raise TimeoutError(
        f"Expected {using}={value} to become enabled; "
        f"last enabled state: {last_enabled!r}; last error: {last_error}"
    )


def wait_for_element_text(
    session_id: str,
    using: str,
    value: str,
    expected: str,
    deadline: float,
) -> None:
    last_text: str | None = None
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            result = request(
                "POST",
                f"/session/{session_id}/element",
                {"using": using, "value": value},
            )
            if isinstance(result, dict) and ELEMENT_KEY in result:
                last_text = element_text(session_id, str(result[ELEMENT_KEY]))
                if last_text.strip() == expected:
                    return
        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected {using}={value} to contain {expected!r}; "
        f"last text: {last_text!r}; last error: {last_error}"
    )


def wait_for_main_heading(session_id: str, expected: str, deadline: float) -> None:
    find_element(
        session_id,
        "xpath",
        f"//main//*[self::h1 or self::h2][normalize-space(.)={json.dumps(expected)}]",
        deadline,
    )


def click_text_button(session_id: str, label: str, deadline: float) -> None:
    element = find_element(
        session_id,
        "xpath",
        f"//button[normalize-space(.)={json.dumps(label)}]",
        deadline,
    )
    click(session_id, element)


def verify_shell(session_id: str, deadline: float) -> None:
    """The header carries the raccoon mark on every size, beside the main navigation."""
    find_element(
        session_id,
        "css selector",
        "header[data-slot='app-header'] [data-artwork-slot='brand.logo.compact']",
        deadline,
    )
    find_element(session_id, "css selector", "nav[aria-label='Main navigation']", deadline)


WORKBENCH_SELECTOR = "main [data-testid='library-workbench']"
WORKBENCH_TREE_XPATH = "//main//*[@data-testid='library-workbench']//nav[@aria-label='Library']"


def open_library_view(session_id: str, view: str, deadline: float) -> None:
    """Open one of Library's pages: the Workbench ("decks"), Documents or Study guides.

    Library reopens the page used last. Every page carries Library's tabs:
    Overview, Lessons, Study guides, Documents and Flashcards (the Workbench).
    """
    library = find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-library']",
        deadline,
    )
    click(session_id, library)
    find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-library'][aria-current='page']",
        deadline,
    )
    if view == "decks":
        # Whichever Library page opens, its Flashcards tab leads to the Workbench.
        find_element(
            session_id,
            "xpath",
            "//main//*[@data-testid='library-workbench']"
            " | //main//nav[@aria-label='Library pages']//button[@data-testid='library-view-decks']",
            deadline,
        )
        if not element_exists(session_id, "css selector", WORKBENCH_SELECTOR):
            switcher = find_element(
                session_id,
                "css selector",
                "main nav[aria-label='Library pages'] button[data-testid='library-view-decks']",
                deadline,
            )
            click(session_id, switcher)
        find_element(session_id, "css selector", WORKBENCH_SELECTOR, deadline)
        return
    switcher = find_element(
        session_id,
        "css selector",
        f"main nav[aria-label='Library pages'] button[data-testid='library-view-{view}']",
        deadline,
    )
    click(session_id, switcher)
    find_element(
        session_id,
        "css selector",
        f"button[data-testid='library-view-{view}'][aria-current='page']",
        deadline,
    )
    wait_for_main_heading(session_id, LIBRARY_VIEW_HEADINGS[view], deadline)


def workbench_tree_entry(session_id: str, label: str, deadline: float) -> str:
    """A subject, deck, view or tag in the Workbench's tree, by its name."""
    return find_element(
        session_id,
        "xpath",
        f"{WORKBENCH_TREE_XPATH}//button[.//span[normalize-space(.)={json.dumps(label)}]]",
        deadline,
    )


def open_workbench_deck(session_id: str, deck_name: str, deadline: float) -> None:
    """Choose a deck in the Workbench's tree and wait for its card list."""
    click(session_id, workbench_tree_entry(session_id, deck_name, deadline))
    wait_for_workbench_title(session_id, deck_name, deadline)


def wait_for_workbench_title(session_id: str, expected: str, deadline: float) -> None:
    find_element(
        session_id,
        "xpath",
        f"//main//h1[@data-testid='workbench-title'][normalize-space(.)={json.dumps(expected)}]",
        deadline,
    )


def find_card_row(session_id: str, question: str, deadline: float) -> str:
    """The Workbench row whose front starts with `question`."""
    return find_element(
        session_id,
        "xpath",
        f"//main//*[@data-card-id]//button[contains(normalize-space(.), {json.dumps(question)})]",
        deadline,
    )


def choose_menu_item(
    session_id: str,
    menu_label: str,
    item: str,
    deadline: float,
    trigger_scope: str = "//main",
) -> None:
    """Open an overflow menu by its accessible name and choose one item.

    A deck's row in the tree and the card list's header both carry a menu named
    for the deck; pass ``trigger_scope="//main//header"`` for the header's.
    """
    trigger = find_element(
        session_id,
        "xpath",
        f"{trigger_scope}//button[@aria-label={json.dumps(menu_label)}]",
        deadline,
    )
    click(session_id, trigger)
    # Keep the ellipsis literal: an ASCII escape would never match in XPath.
    item_literal = json.dumps(item, ensure_ascii=False)
    option = find_element(
        session_id,
        "xpath",
        # A menu on wider screens; a sheet of buttons on phones.
        f"//*[@role='menuitem'][normalize-space(.)={item_literal}]"
        f" | //*[@role='dialog']//*[@aria-label={json.dumps(menu_label)}]//button[normalize-space(.)={item_literal}]",
        deadline,
    )
    click(session_id, option)


def verify_fresh_home(session_id: str, deadline: float) -> None:
    """A fresh library opens on Home, which welcomes it with ways to start."""
    find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-home'][aria-current='page']",
        deadline,
    )
    wait_for_main_heading(session_id, "Your library is empty", deadline)
    for label in ("Create a subject", "Import a document", "Open Library"):
        find_element(
            session_id,
            "xpath",
            f"//main//button[normalize-space(.)={json.dumps(label)}]",
            deadline,
        )


def assert_internal_practice_shell(session_id: str, deck_name: str) -> None:
    state = request(
        "POST",
        f"/session/{session_id}/execute/sync",
        {
            "script": """
                const primary = [...document.querySelectorAll("nav[aria-label='Main navigation'] button")];
                return {
                  selectedPrimary: primary
                    .filter((button) => button.getAttribute('aria-current') === 'page')
                    .map((button) => button.dataset.testid || ''),
                  practiceDestinations: primary
                    .filter((button) => button.dataset.testid === 'nav-practice').length,
                  heading: (document.querySelector('main h1')?.textContent || '').trim(),
                  progressValueText: document.querySelector(
                    "main [role='progressbar'][aria-label='Practice session progress']",
                  )?.getAttribute('aria-valuetext') || '',
                  formerHeaderProgressCount: document.querySelectorAll(
                    "main header [aria-label*=' studied, '][aria-label*=' remaining']",
                  ).length,
                };
            """,
            "args": [],
        },
    )
    if not isinstance(state, dict):
        raise AssertionError(f"Could not inspect the internal Practice workflow: {state!r}")
    # A session runs inside Home, where Review lives, wherever it started.
    if state.get("selectedPrimary") != ["nav-home"]:
        raise AssertionError(
            f"Internal Practice did not run inside Home: {state!r}"
        )
    if int(state.get("practiceDestinations", -1)) != 0:
        raise AssertionError(f"Practice returned to standing navigation: {state!r}")
    if state.get("heading") != deck_name:
        raise AssertionError(
            f"Contextual Practice lost the launching deck identity: {state!r}"
        )
    if state.get("progressValueText") != (
        "Card 1 of 1, 0 studied, 1 remaining, 0% complete"
    ):
        raise AssertionError(
            f"Active Practice progress lost its accessible value text: {state!r}"
        )
    if int(state.get("formerHeaderProgressCount", -1)) != 0:
        raise AssertionError(
            f"Active Practice retained its former header progress chip: {state!r}"
        )


def verify_search_workspace_selected(session_id: str, deadline: float) -> None:
    """The header's Search opens Library on All cards with the search field focused."""
    wait_for_workbench_title(session_id, "All cards", deadline)
    while time.monotonic() < deadline:
        focused = request(
            "POST",
            f"/session/{session_id}/execute/sync",
            {
                "script": """
                    const active = document.activeElement;
                    return Boolean(active && active.matches("main [data-testid='library-workbench'] input[type='search']"));
                """,
                "args": [],
            },
        )
        if focused is True:
            return
        time.sleep(0.1)
    raise AssertionError("Search did not focus the Workbench's search field")


def verify_tag_filtered_search(
    session_id: str,
    question: str,
    tag_name: str,
    deadline: float,
) -> None:
    """A tag from Progress opens the Workbench on that tag's cards."""
    wait_for_workbench_title(session_id, tag_name, deadline)
    find_element(
        session_id,
        "xpath",
        f"{WORKBENCH_TREE_XPATH}//button[@aria-current='true'][.//span[normalize-space(.)={json.dumps(tag_name)}]]",
        deadline,
    )
    find_card_row(session_id, question, deadline)


def verify_analytics_tag_search(
    session_id: str,
    question: str,
    tag_name: str,
    deadline: float,
) -> None:
    tag_action = find_element(
        session_id,
        "xpath",
        f"//main//button[@aria-label={json.dumps(f'Search cards tagged {tag_name}')}]",
        deadline,
    )
    click(session_id, tag_action)
    verify_tag_filtered_search(session_id, question, tag_name, deadline)


def wait_for_saved_library_search_checkpoint(session_id: str, deadline: float) -> None:
    """The app saves its checkpoint shortly after navigating; restart only once it has."""
    checkpoint: Any = None
    while time.monotonic() < deadline:
        checkpoint = request(
            "POST",
            f"/session/{session_id}/execute/async",
            {
                "script": """
                    const done = arguments[arguments.length - 1];
                    window.__TAURI_INTERNALS__.invoke('get_ui_checkpoint')
                      .then((value) => done(value))
                      .catch((error) => done({ error: String(error) }));
                """,
                "args": [],
            },
        )
        if (
            isinstance(checkpoint, dict)
            and checkpoint.get("section") == "decks"
            and checkpoint.get("flashcardsWorkspace") == "search"
        ):
            return
        time.sleep(0.1)
    raise AssertionError(f"Library Search checkpoint was not saved before restart: {checkpoint!r}")


def verify_due_today(session_id: str, expected: str, deadline: float) -> None:
    """Home's Today strip and the Home badge agree on what is due today."""
    home = find_element(session_id, "css selector", "button[data-testid='nav-home']", deadline)
    click(session_id, home)
    find_element(
        session_id,
        "xpath",
        f"//main//*[@data-testid='home-today']//h2[normalize-space(.)={json.dumps(expected)}]",
        deadline,
    )
    count = expected.split(" ", 1)[0]
    find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[@data-testid='nav-home']"
        f"//*[@data-slot='nav-badge'][normalize-space(.)={json.dumps(count)}]",
        deadline,
    )
    # The smoke deck is filed under no subject.
    find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='subject-tile'][.//*[normalize-space(.)='No subject']]",
        deadline,
    )


def create_and_review_card(session_id: str) -> tuple[str, str]:
    deck_name = "Native smoke deck"
    question = "What does the native smoke test persist?"
    answer = "A deck, a card, and one accepted review."
    tag_name = "Native smoke tag"

    progress("Opening Library from Home on a fresh library")
    deadline = deadline_after()
    verify_fresh_home(session_id, deadline)
    open_library_view(session_id, "decks", deadline)
    wait_for_workbench_title(session_id, "All cards", deadline)

    progress("Creating a deck from the empty Workbench")
    deadline = deadline_after()
    find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='library-workbench']//*[normalize-space(.)='No cards yet']",
        deadline,
    )
    click_text_button(session_id, "New deck", deadline)
    set_input_text(
        session_id,
        "css selector",
        "[role='dialog'] input[data-autofocus='true']",
        deck_name,
        deadline,
    )
    create_button = wait_for_enabled_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[normalize-space(.)='Create deck']",
        deadline,
    )
    click(session_id, create_button)
    wait_for_workbench_title(session_id, deck_name, deadline)
    workbench_tree_entry(session_id, deck_name, deadline)

    progress("Creating and verifying a flashcard")
    deadline = deadline_after()
    click_text_button(session_id, "New card", deadline)
    set_rich_editor_text(session_id, 0, question, deadline)
    set_rich_editor_text(session_id, 1, answer, deadline)
    tag_input = find_element(
        session_id,
        "css selector",
        "main input[aria-label='Tags']",
        deadline,
    )
    send_keys(session_id, tag_input, f"{tag_name},")
    find_element(
        session_id,
        "css selector",
        f"main button[aria-label={json.dumps(f'Remove {tag_name} tag')}]",
        deadline,
    )
    save_button = wait_for_enabled_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Save card']",
        deadline,
    )
    click(session_id, save_button)
    wait_for_workbench_title(session_id, deck_name, deadline)
    find_card_row(session_id, question, deadline)

    progress("Verifying today's count on Home and its badge")
    deadline = deadline_after()
    verify_due_today(session_id, "1 card due today", deadline)

    progress("Starting today's review from Home and ending it early")
    deadline = deadline_after()
    start_review = find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='home-today']//button[normalize-space(.)='Start review']",
        deadline,
    )
    click(session_id, start_review)
    find_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Show answer']",
        deadline,
    )
    assert_internal_practice_shell(session_id, deck_name)
    click_text_button(session_id, "End session", deadline)
    wait_for_main_heading(session_id, "Home", deadline)
    find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-home'][aria-current='page']",
        deadline,
    )

    progress("Opening and dismissing selected-deck Practice setup")
    deadline = deadline_after()
    open_library_view(session_id, "decks", deadline)
    choose_menu_item(session_id, f"{deck_name} actions", "Practice with options…", deadline)
    find_element(
        session_id,
        "xpath",
        "//*[@role='dialog'][.//h2[normalize-space(.)='Practice selected decks']]",
        deadline,
    )
    original_rect = request("GET", f"/session/{session_id}/window/rect")
    if not isinstance(original_rect, dict):
        raise AssertionError(f"Practice setup viewport could not be recorded: {original_rect!r}")
    try:
        for expected_mode, width, height in (
            ("wide", 1280, 800),
            ("medium", 900, 760),
            ("compact", 390, 700),
        ):
            request(
                "POST",
                f"/session/{session_id}/window/rect",
                {"width": width, "height": height},
            )
            layout = None
            layout_deadline = deadline_after()
            while time.monotonic() < layout_deadline:
                layout = request(
                    "POST",
                    f"/session/{session_id}/execute/sync",
                    {
                        "script": """
                            const content = document.querySelector(
                              "[role='dialog'] [data-layout-mode]",
                            );
                            return content ? {
                              mode: content.dataset.layoutMode,
                              contained: content.scrollWidth <= content.clientWidth,
                            } : null;
                        """,
                        "args": [],
                    },
                )
                if layout == {"mode": expected_mode, "contained": True}:
                    break
                time.sleep(0.1)
            else:
                raise AssertionError(
                    f"Practice setup {expected_mode} layout was not contained: {layout!r}"
                )
    finally:
        request(
            "POST",
            f"/session/{session_id}/window/rect",
            {
                "x": original_rect.get("x"),
                "y": original_rect.get("y"),
                "width": original_rect.get("width"),
                "height": original_rect.get("height"),
            },
        )
    close_setup = find_element(
        session_id,
        "css selector",
        "[role='dialog'] button[aria-label='Close dialog']",
        deadline,
    )
    click(session_id, close_setup)
    find_element(session_id, "css selector", WORKBENCH_SELECTOR, deadline)

    progress("Verifying one-step Practice from the tree returns to the Workbench")
    deadline = deadline_after()
    click(session_id, workbench_tree_entry(session_id, "All cards", deadline))
    wait_for_workbench_title(session_id, "All cards", deadline)
    choose_menu_item(session_id, f"{deck_name} actions", "Practice", deadline)
    show_answer = find_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Show answer']",
        deadline,
    )
    assert_internal_practice_shell(session_id, deck_name)
    click_text_button(session_id, "End session", deadline)
    find_element(session_id, "css selector", WORKBENCH_SELECTOR, deadline)
    find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-library'][aria-current='page']",
        deadline,
    )
    wait_for_workbench_title(session_id, "All cards", deadline)

    progress("Verifying deck Practice returns to that exact deck")
    deadline = deadline_after()
    open_workbench_deck(session_id, deck_name, deadline)
    practice_button = find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Practice']",
        deadline,
    )
    click(session_id, practice_button)
    show_answer = find_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Show answer']",
        deadline,
    )
    assert_internal_practice_shell(session_id, deck_name)
    click_text_button(session_id, "End session", deadline)
    wait_for_workbench_title(session_id, deck_name, deadline)
    find_card_row(session_id, question, deadline)

    progress("Completing one review after the exact deck return")
    deadline = deadline_after()
    practice_button = find_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Practice']",
        deadline,
    )
    click(session_id, practice_button)
    show_answer = find_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Show answer']",
        deadline,
    )
    assert_internal_practice_shell(session_id, deck_name)
    click(session_id, show_answer)
    medium = find_element(
        session_id,
        "css selector",
        "button[aria-keyshortcuts='2'][aria-label='2 Medium']",
        deadline,
    )
    click(session_id, medium)
    find_element(session_id, "css selector", PRACTICE_COMPLETE_SELECTOR, deadline)

    progress("Verifying Progress and chart data")
    deadline = deadline_after(ANALYTICS_TIMEOUT_SECONDS)
    view_progress = find_element(
        session_id,
        "xpath",
        "//main//*[@data-testid='practice-complete']//button[normalize-space(.)='See your progress']",
        deadline,
    )
    click(session_id, view_progress)
    wait_for_main_heading(session_id, "Progress", deadline)
    find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-progress'][aria-current='page']",
        deadline,
    )
    find_element(session_id, "css selector", ANALYTICS_SCREEN_SELECTOR, deadline)
    find_element(
        session_id,
        "xpath",
        "//main//section[.//h2[normalize-space(.)='Every answer']]"
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )
    chart_data_button = find_element(
        session_id,
        "css selector",
        "main section[aria-labelledby='progress-heading'] button[aria-expanded]",
        deadline,
    )
    click(session_id, chart_data_button)
    find_element(session_id, "css selector", CHART_DATA_SELECTOR, deadline)
    progress("Opening Search from the current recent-review tag")
    verify_analytics_tag_search(session_id, question, tag_name, deadline)
    wait_for_saved_library_search_checkpoint(session_id, deadline)
    progress("Progress verification completed")
    return deck_name, question


def verify_persistence_after_restart(
    session_id: str,
    deck_name: str,
    question: str,
) -> None:
    progress("Verifying deck and card persistence after restart")
    deadline = deadline_after()
    verify_shell(session_id, deadline)
    open_library_view(session_id, "decks", deadline)
    open_workbench_deck(session_id, deck_name, deadline)
    find_card_row(session_id, question, deadline)

    progress("Verifying persisted review history in Progress after restart")
    deadline = deadline_after(ANALYTICS_TIMEOUT_SECONDS)
    progress_button = find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-progress']",
        deadline,
    )
    click(session_id, progress_button)
    wait_for_main_heading(session_id, "Progress", deadline)
    find_element(session_id, "css selector", ANALYTICS_SCREEN_SELECTOR, deadline)
    find_element(
        session_id,
        "xpath",
        "//main//section[.//h2[normalize-space(.)='Every answer']]"
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    progress("Verifying post-restart navigation to Settings")
    deadline = deadline_after()
    settings_button = find_element(
        session_id,
        "css selector",
        "button[data-testid='app-settings']",
        deadline,
    )
    click(session_id, settings_button)
    wait_for_element_text(session_id, "css selector", "main h1", "Settings", deadline)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True, type=Path)
    args = parser.parse_args()
    application = args.application.resolve()
    if not application.is_file():
        raise FileNotFoundError(application)

    session_id: str | None = None
    progress("Starting tauri-driver")
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
        wait_for_driver(deadline_after(20))
        progress("Opening the first native application session")
        session_id = create_session(application)
        verify_shell(session_id, deadline_after())
        deck_name, question = create_and_review_card(session_id)

        progress("Restarting the native application session")
        delete_session(session_id)
        session_id = None
        session_id = create_session(application)
        verify_persistence_after_restart(session_id, deck_name, question)

        print(
            "Native smoke test passed: opened a fresh library on Home, created a deck and "
            "card, matched today's count on Home and its badge, ran Practice "
            "inside Home with accessible progress, accepted a review, restarted ostudy, "
            "and verified durable content and Progress."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver log ---")
        try:
            print(log_path.read_text(encoding="utf-8"))
        except OSError:
            pass
        if session_id:
            try:
                source = request("GET", f"/session/{session_id}/source")
                print("\n--- current WebView source (truncated) ---")
                print(str(source)[:12_000])
            except Exception:
                pass
        raise
    finally:
        if session_id:
            try:
                delete_session(session_id)
            except Exception:
                pass
        if driver.poll() is None:
            os.killpg(driver.pid, signal.SIGTERM)
            try:
                driver.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(driver.pid, signal.SIGKILL)
        log_path.unlink(missing_ok=True)


if __name__ == "__main__":
    raise SystemExit(main())
