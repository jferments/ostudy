"""Native Tutor checks follow accessible controls, not retired input/summary markup."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import tutor_smoke as tutor


class TutorSmokeSelectorTests(unittest.TestCase):
    def test_selected_chat_waits_for_its_own_header_and_enabled_settings(self):
        measurements = [None, {"title": "Another chat", "settingsEnabled": True},
                        {"title": "Saved chat", "settingsEnabled": False},
                        {"title": "Saved chat", "settingsEnabled": True}]
        with patch.object(tutor.ui, "execute", side_effect=measurements) as execute, \
                patch.object(tutor.time, "sleep"):
            tutor.wait_for_selected_conversation("session", "Saved chat", tutor.webdriver.deadline_after(1))
        self.assertEqual(execute.call_count, 4)
        for call in execute.call_args_list:
            script = call.args[1]
            self.assertIn("section[aria-label='Tutor conversation'] header", script)
            self.assertIn('button[aria-label="Chat actions"]', script)
            self.assertIn("!settings.disabled", script)

    def test_missing_wrong_or_busy_conversation_cannot_pass_readiness(self):
        for measurement in [None, {}, {"title": "Other", "settingsEnabled": True},
                            {"title": "Saved chat", "settingsEnabled": False}]:
            with self.subTest(measurement=measurement), \
                    patch.object(tutor.ui, "execute", return_value=measurement), \
                    patch.object(tutor.time, "monotonic", side_effect=[0, 2]), \
                    patch.object(tutor.time, "sleep"):
                with self.assertRaisesRegex(AssertionError, "did not become current"):
                    tutor.wait_for_selected_conversation("session", "Saved chat", 1)

    def test_ordinary_chat_privacy_scans_current_and_legacy_controls(self):
        with patch.object(tutor.ui, "execute", return_value=True) as execute:
            tutor.assert_ordinary_chat("session")
        script = execute.call_args.args[1]
        self.assertIn("section[aria-label='Tutor conversation']", script)
        self.assertIn("querySelectorAll('button, summary')", script)
        self.assertIn("if (!main) return null", script)
        for label in ["Conversation memory", "Search index maintenance", "Prepare selected material"]:
            self.assertIn(label, script)

    def test_ordinary_chat_rejects_leaked_or_unmeasurable_controls(self):
        for result in [False, None, {}, []]:
            with self.subTest(result=result), patch.object(tutor.ui, "execute", return_value=result):
                with self.assertRaisesRegex(AssertionError, "leaked"):
                    tutor.assert_ordinary_chat("session")

    def test_hint_privacy_uses_the_actual_hint_region_and_accessible_labels(self):
        with patch.object(tutor.ui, "execute", return_value={"sourceButtons": False, "memory": False, "practice": True}) as execute:
            tutor.assert_hint_controls_private("session")
        script = execute.call_args.args[1]
        self.assertIn('section[aria-label="Flashcard hints"]', script)
        self.assertIn("getAttribute('aria-label')", script)
        self.assertIn("querySelectorAll('button, summary')", script)
        self.assertIn("['Add sources', 'Chat actions', 'Chat settings']", script)
        self.assertIn("if (!dialog) return null", script)

    def test_hint_privacy_rejects_sources_memory_missing_region_or_replaced_practice(self):
        expected = {"sourceButtons": False, "memory": False, "practice": True}
        for result in [None, {}, {**expected, "sourceButtons": True},
                       {**expected, "memory": True}, {**expected, "practice": False}]:
            with self.subTest(result=result), patch.object(tutor.ui, "execute", return_value=result):
                with self.assertRaisesRegex(AssertionError, "inappropriate controls"):
                    tutor.assert_hint_controls_private("session")

    def run_workflow(self, changed_checkpoint=None):
        original = {"sessionId": "practice-id", "position": 2, "revealed": False}
        memory_reads = 0
        checkpoint_reads = 0

        def native(_session, command, arguments=None):
            nonlocal memory_reads, checkpoint_reads
            if command in ["list_ai_provider_profiles", "list_tutor_conversations"]:
                return []
            if command == "create_tutor_conversation":
                return {"id": "saved-id", "title": "Offline tutor continuity fixture"}
            if command == "get_tutor_memory":
                self.assertEqual(arguments, {"conversationId": "saved-id"})
                memory_reads += 1
                return {"historyEnabled": memory_reads == 1, "retrievalEnabled": False, "revision": memory_reads}
            if command == "get_tutor_conversation":
                self.assertEqual(arguments, {"input": {"conversationId": "saved-id", "beforeSequence": None}})
                return {"turns": []}
            if command == "get_ui_checkpoint":
                checkpoint_reads += 1
                return {"practice": {**original, **(changed_checkpoint or {})} if checkpoint_reads > 1 else original.copy()}
            raise AssertionError(f"Unexpected native command: {command}")

        with patch.object(tutor, "native", side_effect=native) as commands, \
                patch.object(tutor.ui, "execute", return_value=True), \
                patch.object(tutor.ui, "assert_no_horizontal_overflow"), \
                patch.object(tutor.ui, "wait_for_overlay_inactive"), \
                patch.object(tutor, "wait_for_selected_conversation"), \
                patch.object(tutor, "assert_ordinary_chat"), \
                patch.object(tutor, "assert_hint_controls_private"), \
                patch.object(tutor.webdriver, "wait_for_main_heading"), \
                patch.object(tutor.webdriver, "element_exists", return_value=True), \
                patch.object(tutor.webdriver, "find_element", return_value="element") as find, \
                patch.object(tutor.webdriver, "wait_for_enabled_element", return_value="element") as enabled, \
                patch.object(tutor.webdriver, "click"):
            tutor.verify_tutor_and_contextual_return("session", "Example deck")
        return commands, find, enabled

    def test_native_workflow_keeps_persistence_reads_and_targets_visible_shared_controls(self):
        commands, find, enabled = self.run_workflow()
        selectors = [call.args[2] for call in [*find.call_args_list, *enabled.call_args_list]]
        self.assertTrue(any("//button[@aria-expanded" in selector and "Conversation memory" in selector for selector in selectors))
        self.assertTrue(any("Use earlier turns automatically" in selector and "@role='checkbox'" in selector for selector in selectors))
        self.assertTrue(any("Example deck" in selector and "@role='checkbox'" in selector for selector in selectors))
        self.assertFalse(any("//input" in selector or "//summary" in selector for selector in selectors))
        called = [call.args[1] for call in commands.call_args_list]
        self.assertEqual(called.count("get_tutor_memory"), 2)
        self.assertEqual(called.count("get_ui_checkpoint"), 2)
        self.assertIn("get_tutor_conversation", called)
        self.assertNotIn("send_tutor_turn", called)

    def test_native_workflow_still_rejects_changed_session_position_or_revealed_answer(self):
        for changed in [{"sessionId": "different"}, {"position": 3}, {"revealed": True}]:
            with self.subTest(changed=changed), self.assertRaisesRegex(AssertionError, "Hint sheet changed practice"):
                self.run_workflow(changed)


if __name__ == "__main__":
    unittest.main()
