#!/usr/bin/env python3
"""Native Tauri smoke workflow using only the W3C WebDriver protocol."""

from __future__ import annotations

import argparse
import json
import os
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


def request(method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        WEBDRIVER_URL + path,
        data=body,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=15) as response:
        decoded = json.loads(response.read().decode("utf-8"))
    if isinstance(decoded, dict) and isinstance(decoded.get("value"), dict):
        error = decoded["value"].get("error")
        if error:
            raise RuntimeError(f"WebDriver {error}: {decoded['value'].get('message', '')}")
    return decoded.get("value") if isinstance(decoded, dict) else decoded


def wait_for_driver(deadline: float) -> None:
    while time.monotonic() < deadline:
        try:
            request("GET", "/status")
            return
        except (OSError, urllib.error.URLError, json.JSONDecodeError):
            time.sleep(0.2)
    raise TimeoutError("tauri-driver did not become ready")


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
        except Exception as error:  # WebDriver reports no-such-element while React loads.
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
        except Exception as error:
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected at least {minimum} elements for {using}={value}; "
        f"last count: {last_count}; last error: {last_error}"
    )


def element_text(session_id: str, element_id: str) -> str:
    value = request("GET", f"/session/{session_id}/element/{element_id}/text")
    return str(value)


def click(session_id: str, element_id: str) -> None:
    request("POST", f"/session/{session_id}/element/{element_id}/click", {})


def send_keys(session_id: str, element_id: str, text: str) -> None:
    click(session_id, element_id)
    request(
        "POST",
        f"/session/{session_id}/element/{element_id}/value",
        {"text": text, "value": list(text)},
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
        except Exception as error:  # The old element may be replaced during navigation.
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected {using}={value} to contain {expected!r}; "
        f"last text: {last_text!r}; last error: {last_error}"
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
    brand = find_element(session_id, "css selector", ".brand-block strong", deadline)
    brand_text = element_text(session_id, brand)
    if brand_text.strip() != "ostudy":
        raise AssertionError(f"Expected ostudy branding, found {brand_text!r}")


def create_and_review_card(session_id: str, deadline: float) -> tuple[str, str]:
    deck_name = "Native smoke deck"
    question = "What does the native smoke test persist?"
    answer = "A deck, a card, and one accepted review."

    click_text_button(session_id, "New deck", deadline)
    name_input = find_element(
        session_id,
        "css selector",
        "input[placeholder='NCLEX Pharmacology']",
        deadline,
    )
    send_keys(session_id, name_input, deck_name)
    create_button = find_element(
        session_id,
        "xpath",
        "//section[@role='dialog']//button[normalize-space(.)='Create deck']",
        deadline,
    )
    click(session_id, create_button)
    wait_for_element_text(session_id, "css selector", "main h1", deck_name, deadline)

    click_text_button(session_id, "Add flashcard", deadline)
    editors = find_elements(
        session_id,
        "css selector",
        ".rich-editor-content[contenteditable='true']",
        2,
        deadline,
    )
    send_keys(session_id, editors[0], question)
    send_keys(session_id, editors[1], answer)
    click_text_button(session_id, "Save card", deadline)
    wait_for_element_text(session_id, "css selector", "main h1", deck_name, deadline)
    find_element(
        session_id,
        "xpath",
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

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
        "xpath",
        "//button[contains(normalize-space(.), '2 Medium')]",
        deadline,
    )
    click(session_id, medium)
    wait_for_element_text(
        session_id,
        "css selector",
        "main h1",
        "Session complete",
        deadline,
    )

    click_text_button(session_id, "View analytics", deadline)
    wait_for_element_text(session_id, "css selector", "main h1", "Analytics", deadline)
    find_element(
        session_id,
        "xpath",
        f"//*[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )
    click_text_button(session_id, "View chart data", deadline)
    wait_for_element_text(
        session_id,
        "css selector",
        ".chart-data-table caption",
        "Data represented in the learning progress chart",
        deadline,
    )
    return deck_name, question


def verify_persistence_after_restart(
    session_id: str,
    deck_name: str,
    question: str,
    deadline: float,
) -> None:
    verify_shell(session_id, deadline)
    deck_button = find_element(
        session_id,
        "xpath",
        f"//button[contains(normalize-space(.), {json.dumps(deck_name)})]",
        deadline,
    )
    click(session_id, deck_button)
    wait_for_element_text(session_id, "css selector", "main h1", deck_name, deadline)
    find_element(
        session_id,
        "xpath",
        f"//strong[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    analytics_button = find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[contains(normalize-space(.), 'Analytics')]",
        deadline,
    )
    click(session_id, analytics_button)
    wait_for_element_text(session_id, "css selector", "main h1", "Analytics", deadline)
    find_element(
        session_id,
        "xpath",
        f"//*[normalize-space(.)={json.dumps(question)}]",
        deadline,
    )

    settings_button = find_element(
        session_id,
        "xpath",
        "//nav[@aria-label='Main navigation']//button[contains(normalize-space(.), 'Settings')]",
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
        wait_for_driver(time.monotonic() + 20)
        session_id = create_session(application)
        deadline = time.monotonic() + 45
        verify_shell(session_id, deadline)
        deck_name, question = create_and_review_card(session_id, deadline)

        delete_session(session_id)
        session_id = None
        session_id = create_session(application)
        verify_persistence_after_restart(
            session_id,
            deck_name,
            question,
            time.monotonic() + 45,
        )

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
