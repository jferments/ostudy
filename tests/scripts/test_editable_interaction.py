"""Native editing must reach a real, visible hit target before clearing it."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import smoke


class EditableInteractionTests(unittest.TestCase):
    def test_clear_focuses_before_the_native_clear_command(self):
        operations = []
        with patch.object(smoke, "click", side_effect=lambda *args: operations.append(("focus", args))), \
                patch.object(smoke, "request", side_effect=lambda *args: operations.append(("request", args))):
            smoke.clear("session", "editor")
        self.assertEqual(operations, [
            ("focus", ("session", "editor")),
            ("request", ("POST", "/session/session/element/editor/clear", {})),
        ])

    def test_a_covered_or_hidden_field_is_not_cleared(self):
        with patch.object(smoke, "click", side_effect=RuntimeError("not a stable hit target")), \
                patch.object(smoke, "request") as request:
            with self.assertRaisesRegex(RuntimeError, "stable hit target"):
                smoke.clear("session", "editor")
        request.assert_not_called()

    def test_clear_failure_is_not_silently_replayed_or_replaced_by_script(self):
        failure = smoke.WebDriverProtocolError(400, "element not interactable", "blocked")
        with patch.object(smoke, "click") as click, \
                patch.object(smoke, "request", side_effect=failure) as request:
            with self.assertRaises(smoke.WebDriverProtocolError):
                smoke.clear("session", "editor")
        click.assert_called_once_with("session", "editor")
        request.assert_called_once_with("POST", "/session/session/element/editor/clear", {})

    def test_rich_editor_still_requires_rendered_text_after_native_input(self):
        operations = []
        with patch.object(smoke, "find_elements", return_value=["question", "answer"]), \
                patch.object(smoke, "click", side_effect=lambda session, element: operations.append(("focus", element))), \
                patch.object(smoke, "request", side_effect=lambda *args: operations.append(("clear", args[1]))), \
                patch.object(smoke, "send_keys", side_effect=lambda session, element, text: operations.append(("type", element, text))), \
                patch.object(smoke, "element_text", return_value="Expected answer") as rendered:
            result = smoke.set_rich_editor_text("session", 1, "Expected answer", smoke.deadline_after(1))
        self.assertEqual(result, "answer")
        self.assertEqual(operations, [
            ("focus", "answer"),
            ("clear", "/session/session/element/answer/clear"),
            ("type", "answer", "Expected answer"),
        ])
        rendered.assert_called_once_with("session", "answer")

    def test_controlled_text_field_keeps_value_verification(self):
        with patch.object(smoke, "find_element", return_value="field"), \
                patch.object(smoke, "click") as click, patch.object(smoke, "request") as request, \
                patch.object(smoke, "send_keys") as send, \
                patch.object(smoke, "element_property", return_value="Expected") as value:
            result = smoke.set_input_text("session", "css selector", "input", "Expected", smoke.deadline_after(1))
        self.assertEqual(result, "field")
        click.assert_called_once_with("session", "field")
        request.assert_called_once_with("POST", "/session/session/element/field/clear", {})
        send.assert_called_once_with("session", "field", "Expected")
        value.assert_called_once_with("session", "field", "value")


if __name__ == "__main__":
    unittest.main()
