"""Tutor's real native/local integration, without provider accounts or API calls.

Executed by the shared card-browser smoke after it creates an ordinary card.
The main runner owns the isolated library, driver process and failure artifacts.
"""

from __future__ import annotations

import json
import time
import uuid
from typing import Any

import smoke as webdriver
import ui_platform_smoke as ui


def native(session_id: str, command: str, arguments: dict[str, Any] | None = None) -> Any:
    result = ui.execute_async(
        session_id,
        """
        const done = arguments[arguments.length - 1];
        window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])
          .then(value => done({value}))
          .catch(error => done({error}));
        """,
        [command, arguments or {}],
    )
    if not isinstance(result, dict) or "error" in result or "value" not in result:
        raise AssertionError(f"Native {command} failed: {result!r}")
    return result["value"]


def click_css(session_id: str, selector: str, deadline: float) -> None:
    webdriver.click(session_id, webdriver.wait_for_enabled_element(
        session_id, "css selector", selector, deadline,
    ))


def wait_for_selected_conversation(
    session_id: str,
    conversation_id: str,
    deadline: float,
) -> None:
    """Wait for React/native conversation selection, not a transient DOM value.

    The Tutor select is controlled. Dispatching ``change`` starts an async native
    read and React may immediately restore the previous controlled value until
    that read resolves. The durable postcondition is the selected id plus the
    conversation-only controls becoming available.
    """
    last: Any = None
    while time.monotonic() < deadline:
        last = ui.execute(session_id, f"""
            const select = document.querySelector("main select[id$='-conversation']");
            const memory = [...document.querySelectorAll('main summary')]
              .some(item => item.textContent.trim() === 'Conversation memory');
            const canDelete = [...document.querySelectorAll('main button')]
              .some(item => item.textContent.trim() === 'Delete' && !item.disabled);
            return {{
              value: select instanceof HTMLSelectElement ? select.value : null,
              disabled: select instanceof HTMLSelectElement ? select.disabled : null,
              memory,
              canDelete,
            }};
        """)
        if (
            isinstance(last, dict)
            and last.get("value") == conversation_id
            and last.get("disabled") is False
            and last.get("memory") is True
            and last.get("canDelete") is True
        ):
            return
        time.sleep(0.1)
    raise AssertionError(
        "The saved Tutor conversation did not become current after its change event: "
        f"{last!r}"
    )


def verify_tutor_and_contextual_return(session_id: str, deck_name: str) -> None:
    deadline = webdriver.deadline_after(60)
    click_css(session_id, "button[data-testid='nav-tutor']", deadline)
    webdriver.wait_for_main_heading(session_id, "Tutor", deadline)
    webdriver.find_element(session_id, "css selector", "main [data-testid='tutor-screen'] textarea", deadline)
    ui.assert_no_horizontal_overflow(session_id, "Tutor without a provider")
    if native(session_id, "list_ai_provider_profiles"):
        raise AssertionError("The isolated tutor smoke must not contain a provider profile")

    conversation = native(session_id, "create_tutor_conversation", {"input": {
        "operationId": uuid.uuid4().hex,
        "scope": {"kind": "general", "id": None},
        "title": "Offline tutor continuity fixture",
    }})
    conversation_id = conversation["id"]
    # Tutor initialization is asynchronous. Wait for the visible workspace to
    # become interactive before asking it to refresh the conversation list.
    refresh = webdriver.wait_for_enabled_element(
        session_id, "xpath", "//main//header//button[normalize-space(.)='Refresh']", deadline,
    )
    webdriver.click(session_id, refresh)
    webdriver.wait_for_enabled_element(
        session_id, "css selector", "main select[id$='-conversation']", deadline,
    )
    webdriver.find_element(session_id, "css selector", f"main select option[value='{conversation_id}']", deadline)
    # OPTION elements are not independently hit-testable in native WebKit.
    # Drive the enabled visible SELECT through its ordinary change event. Since
    # it is React-controlled, selection completion is verified separately below.
    dispatched = ui.execute(session_id, f"""
        const select = document.querySelector("main select[id$='-conversation']");
        if (!(select instanceof HTMLSelectElement) || select.disabled) return false;
        select.value = {json.dumps(conversation_id)};
        select.dispatchEvent(new Event('change', {{ bubbles: true }}));
        return true;
    """)
    if dispatched is not True:
        raise AssertionError("The saved Tutor conversation change event could not be dispatched")
    wait_for_selected_conversation(session_id, conversation_id, deadline)
    summary = webdriver.find_element(session_id, "xpath", "//main//summary[normalize-space(.)='Conversation memory']", deadline)
    webdriver.click(session_id, summary)
    checkbox = webdriver.wait_for_enabled_element(
        session_id, "xpath", "//main//label[contains(.,'Use earlier turns automatically')]//input", deadline,
    )
    initial = native(session_id, "get_tutor_memory", {"conversationId": conversation_id})
    if not initial["historyEnabled"] or initial["retrievalEnabled"]:
        raise AssertionError(f"Unexpected memory defaults: {initial!r}")
    webdriver.click(session_id, checkbox)
    while time.monotonic() < deadline:
        saved = native(session_id, "get_tutor_memory", {"conversationId": conversation_id})
        if not saved["historyEnabled"] and saved["revision"] > initial["revision"]:
            break
        time.sleep(0.1)
    else:
        raise AssertionError("The UI memory change did not persist through the native command")
    # Reading and editing local memory must never create a chat turn.
    detail = native(session_id, "get_tutor_conversation", {"input": {
        "conversationId": conversation_id, "beforeSequence": None,
    }})
    if detail["turns"]:
        raise AssertionError("Inspecting memory unexpectedly created a provider-backed turn")
    delete = webdriver.wait_for_enabled_element(session_id, "xpath", "//main//button[normalize-space(.)='Delete']", deadline)
    webdriver.click(session_id, delete)
    confirmation = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//*[@role='alertdialog']//button[normalize-space(.)='Delete conversation']",
        deadline,
    )
    webdriver.click(session_id, confirmation)
    ui.wait_for_overlay_inactive(session_id, deadline, "Tutor deletion confirmation")
    if any(item["id"] == conversation_id for item in native(session_id, "list_tutor_conversations")):
        raise AssertionError("Confirmed conversation deletion did not persist")

    # Return to the existing card and open its contextual hint sheet. The sheet
    # must not reveal an answer, replace Practice, or start a different session.
    click_css(session_id, "button[data-testid='nav-decks']", deadline)
    deck = webdriver.find_element(session_id, "xpath", f"//main//*[@role='button' and @aria-label={json.dumps('Open ' + deck_name)}]", deadline)
    webdriver.click(session_id, deck)
    webdriver.wait_for_main_heading(session_id, deck_name, deadline)
    practice = webdriver.wait_for_enabled_element(session_id, "xpath", "//main//button[normalize-space(.)='Practice']", deadline)
    webdriver.click(session_id, practice)
    hint = webdriver.wait_for_enabled_element(session_id, "css selector", "main button[aria-label='Get a flashcard hint']", deadline)
    checkpoint = None
    while time.monotonic() < deadline:
        checkpoint = native(session_id, "get_ui_checkpoint")
        if checkpoint and checkpoint.get("practice"):
            break
        time.sleep(0.1)
    if not checkpoint or not checkpoint.get("practice"):
        raise AssertionError("Practice did not persist a session checkpoint")
    original = checkpoint["practice"]
    if original["revealed"]:
        raise AssertionError("A newly started practice card was already revealed")
    webdriver.click(session_id, hint)
    webdriver.find_element(session_id, "css selector", "[role='dialog'] section[aria-label='Flashcard hints']", deadline)
    snapshot = ui.execute(session_id, """
        const dialog = document.querySelector('[role="dialog"]');
        return {
          sourceButtons: [...dialog.querySelectorAll('button')].some(b => b.textContent.trim().startsWith('Source ')),
          memory: [...dialog.querySelectorAll('summary')].some(s => s.textContent.trim() === 'Conversation memory'),
          practice: Boolean(document.querySelector('main button[aria-label="Get a flashcard hint"]'))
        };
    """)
    if snapshot != {"sourceButtons": False, "memory": False, "practice": True}:
        raise AssertionError(f"Hidden-answer panel exposed inappropriate controls: {snapshot!r}")
    click_css(session_id, "[role='dialog'] button[aria-label='Close dialog']", deadline)
    ui.wait_for_overlay_inactive(session_id, deadline, "return from contextual hint")
    restored = native(session_id, "get_ui_checkpoint")["practice"]
    for key in ("sessionId", "position", "revealed"):
        if restored[key] != original[key]:
            raise AssertionError(f"Hint sheet changed practice {key}: {original!r} -> {restored!r}")
    print("Tutor smoke passed: offline native memory controls, confirmed deletion, and exact contextual Practice return.")
