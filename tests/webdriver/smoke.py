#!/usr/bin/env python3
"""Minimal native Tauri smoke test using the W3C WebDriver protocol."""

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
    with urllib.request.urlopen(req, timeout=10) as response:
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
        except Exception as error:  # WebDriver returns no-such-element while React loads.
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(f"Element was not found: {using}={value}; last error: {last_error}")


def element_text(session_id: str, element_id: str) -> str:
    value = request("GET", f"/session/{session_id}/element/{element_id}/text")
    return str(value)


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

        deadline = time.monotonic() + 30
        brand = find_element(session_id, "css selector", ".brand-block strong", deadline)
        brand_text = element_text(session_id, brand)
        if brand_text.strip() != "ostudy":
            raise AssertionError(f"Expected ostudy branding, found {brand_text!r}")

        practice_button = find_element(
            session_id,
            "xpath",
            "//button[contains(normalize-space(.), 'Practice')]",
            deadline,
        )
        request("POST", f"/session/{session_id}/element/{practice_button}/click", {})
        heading = find_element(session_id, "css selector", "main h1", time.monotonic() + 15)
        heading_text = element_text(session_id, heading)
        if heading_text.strip() != "Practice":
            raise AssertionError(f"Expected Practice screen, found {heading_text!r}")
        find_element(
            session_id,
            "css selector",
            ".practice-setup-controls label.practice-shuffle-checkbox input[type='checkbox']",
            time.monotonic() + 15,
        )
        recent_heading = find_element(
            session_id,
            "css selector",
            "#recent-practice-heading",
            time.monotonic() + 15,
        )
        if element_text(session_id, recent_heading).strip() != "Recently practiced":
            raise AssertionError("The recent-practice summary is missing from Practice setup.")

        settings_button = find_element(
            session_id,
            "xpath",
            "//button[contains(normalize-space(.), 'Settings')]",
            deadline,
        )
        request("POST", f"/session/{session_id}/element/{settings_button}/click", {})
        heading = find_element(session_id, "css selector", "main h1", time.monotonic() + 15)
        heading_text = element_text(session_id, heading)
        if heading_text.strip() != "Settings":
            raise AssertionError(f"Expected Settings screen, found {heading_text!r}")

        print(
            "Native smoke test passed: launched ostudy, verified compact Practice setup, "
            "and opened Settings."
        )
        return 0
    except Exception:
        print("\n--- tauri-driver log ---")
        try:
            print(log_path.read_text(encoding="utf-8"))
        except OSError:
            pass
        raise
    finally:
        if session_id:
            try:
                request("DELETE", f"/session/{session_id}")
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
