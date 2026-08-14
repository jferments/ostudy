#!/usr/bin/env python3
"""Native Tauri smoke workflow using only the W3C WebDriver protocol."""

from __future__ import annotations

import argparse
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
ANALYTICS_SCREEN_SELECTOR = "main .analytics-screen"
CHART_DATA_SELECTOR = "main .chart-data-table"


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
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected at least {minimum} elements for {using}={value}; "
        f"last count: {last_count}; last error: {last_error}"
    )


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


def click(session_id: str, element_id: str) -> None:
    """Click with a short retry for transient WebKit hit-testing/animation races."""
    deadline = time.monotonic() + 2.5
    last_error: WebDriverProtocolError | None = None
    while time.monotonic() < deadline:
        try:
            request("POST", f"/session/{session_id}/element/{element_id}/click", {})
            return
        except WebDriverProtocolError as error:
            if error.error not in TRANSIENT_CLICK_ERRORS:
                raise
            last_error = error
            time.sleep(0.1)
    if last_error is not None:
        raise last_error
    raise RuntimeError("WebDriver click retry ended without a result")


def clear(session_id: str, element_id: str) -> None:
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
    selector = ".rich-editor-content[contenteditable='true']"
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
    find_element(
        session_id,
        "css selector",
        "aside.sidebar [data-artwork-slot='brand.logo.sidebar'][aria-label='ostudy']",
        deadline,
    )


def create_and_review_card(session_id: str) -> tuple[str, str]:
    deck_name = "Native smoke deck"
    question = "What does the native smoke test persist?"
    answer = "A deck, a card, and one accepted review."

    progress("Creating a deck")
    deadline = deadline_after()
    click_text_button(session_id, "New deck", deadline)
    set_input_text(
        session_id,
        "css selector",
        "dialog input[data-autofocus='true']",
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
    wait_for_main_heading(session_id, deck_name, deadline)

    progress("Creating and verifying a flashcard")
    deadline = deadline_after()
    click_text_button(session_id, "Add flashcard", deadline)
    set_rich_editor_text(session_id, 0, question, deadline)
    set_rich_editor_text(session_id, 1, answer, deadline)
    save_button = wait_for_enabled_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Save card']",
        deadline,
    )
    click(session_id, save_button)
    wait_for_main_heading(session_id, deck_name, deadline)
    find_element(
        session_id,
        "xpath",
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    progress("Completing one review")
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
    click(session_id, show_answer)
    medium = find_element(
        session_id,
        "css selector",
        "button.confidence-score-button[aria-keyshortcuts='2'][aria-label='2 Medium']",
        deadline,
    )
    click(session_id, medium)
    find_element(session_id, "css selector", PRACTICE_COMPLETE_SELECTOR, deadline)

    progress("Verifying analytics and chart data")
    deadline = deadline_after(ANALYTICS_TIMEOUT_SECONDS)
    view_analytics = find_element(
        session_id,
        "css selector",
        f"{PRACTICE_COMPLETE_SELECTOR} .primary-button",
        deadline,
    )
    click(session_id, view_analytics)
    find_element(session_id, "css selector", ANALYTICS_SCREEN_SELECTOR, deadline)
    find_element(
        session_id,
        "xpath",
        "//section[contains(@class, 'recent-review-panel')]"
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
    progress("Analytics verification completed")
    return deck_name, question


def verify_persistence_after_restart(
    session_id: str,
    deck_name: str,
    question: str,
) -> None:
    progress("Verifying deck and card persistence after restart")
    deadline = deadline_after()
    verify_shell(session_id, deadline)
    decks_button = find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    click(session_id, decks_button)
    wait_for_element_text(session_id, "css selector", "main h1", "Flashcards", deadline)
    deck_button = find_element(
        session_id,
        "xpath",
        f"//main//button[.//strong[normalize-space(.)={json.dumps(deck_name)}]]",
        deadline,
    )
    click(session_id, deck_button)
    wait_for_main_heading(session_id, deck_name, deadline)
    find_element(
        session_id,
        "xpath",
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    progress("Verifying persisted review analytics after restart")
    deadline = deadline_after(ANALYTICS_TIMEOUT_SECONDS)
    analytics_button = find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-analytics']",
        deadline,
    )
    click(session_id, analytics_button)
    find_element(session_id, "css selector", ANALYTICS_SCREEN_SELECTOR, deadline)
    find_element(
        session_id,
        "xpath",
        "//section[contains(@class, 'recent-review-panel')]"
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    progress("Verifying post-restart navigation")
    deadline = deadline_after()
    settings_button = find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
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
            "Native smoke test passed: created a deck and card, accepted a review, "
            "restarted ostudy, and verified durable content and analytics."
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
