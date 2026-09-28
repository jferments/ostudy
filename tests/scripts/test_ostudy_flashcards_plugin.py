import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "integrations" / "ostudy-flashcards"
SKILL = PLUGIN / "skills" / "generate-ostudy-flashcards"
WRITER = SKILL / "scripts" / "write_ostudy.py"
EXAMPLE = SKILL / "references" / "all-card-types.ostudy.json"
BUNDLED_SCHEMA = SKILL / "references" / "ostudy-flashcards-v2.schema.json"
CANONICAL_SCHEMA = ROOT / "docs" / "ostudy-flashcards-v2.schema.json"
MARKETPLACE = ROOT / ".agents" / "plugins" / "marketplace.json"


class OStudyFlashcardsPluginTests(unittest.TestCase):
    def run_writer(self, document: dict, *, output: bool = False):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            draft = base / "draft.json"
            draft.write_text(json.dumps(document, ensure_ascii=False), encoding="utf-8")
            command = [sys.executable, str(WRITER), "--input", str(draft)]
            target = base / "deck.ostudy.json"
            if output:
                command += ["--output", str(target)]
            else:
                command += ["--validate-only"]
            completed = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            written = target.read_bytes() if target.exists() else None
            return completed, written

    def example(self) -> dict:
        return json.loads(EXAMPLE.read_text(encoding="utf-8"))

    def test_portable_manifest_and_repo_marketplace_are_skills_only(self):
        manifest = json.loads((PLUGIN / "plugin.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["name"], "ostudy-flashcards")
        self.assertEqual(manifest["extensions"]["com.openai"]["interface"]["displayName"], "oStudy Flashcards")
        self.assertTrue((SKILL / "SKILL.md").is_file())
        self.assertFalse((PLUGIN / "mcp.json").exists())

        marketplace = json.loads(MARKETPLACE.read_text(encoding="utf-8"))
        entry = marketplace["plugins"][0]
        self.assertEqual(entry["name"], "ostudy-flashcards")
        self.assertEqual(entry["source"]["path"], "./integrations/ostudy-flashcards")
        self.assertEqual(entry["category"], "Education")

    def test_bundled_v2_schema_matches_the_canonical_contract(self):
        canonical = CANONICAL_SCHEMA.read_bytes()
        self.assertEqual(BUNDLED_SCHEMA.read_bytes(), canonical)
        schema = json.loads(canonical)
        self.assertEqual(schema["properties"]["formatVersion"]["const"], 2)
        question_refs = schema["$defs"]["interactiveCard"]["properties"]["question"]["oneOf"]
        self.assertEqual(len(question_refs), 6)

    def test_all_card_types_fixture_passes_the_dependency_free_writer(self):
        completed = subprocess.run(
            [sys.executable, str(WRITER), "--input", str(EXAMPLE), "--validate-only"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("8 source cards", completed.stdout)

        kinds = {
            card["question"]["kind"]
            for card in self.example()["cards"]
            if card["type"] == "interactive"
        }
        self.assertEqual(
            kinds,
            {"single_choice", "select_all", "true_false", "matching", "grouping", "ordering"},
        )

    def test_writer_normalizes_and_revalidates_exact_written_bytes(self):
        document = self.example()
        document["deck"]["name"] = "  Normalized deck  "
        document["deck"]["description"] = "  Scope  "
        document["defaultTags"] = ["  shared  "]
        completed, written = self.run_writer(document, output=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIsNotNone(written)
        parsed = json.loads(written.decode("utf-8"))
        self.assertEqual(parsed["deck"]["name"], "Normalized deck")
        self.assertEqual(parsed["deck"]["description"], "Scope")
        self.assertEqual(parsed["defaultTags"], ["shared"])
        digest = hashlib.sha256(written).hexdigest()
        self.assertIn(digest, completed.stdout)
        self.assertEqual(parsed["formatVersion"], 2)

    def test_writer_rejects_invalid_cross_references_without_output(self):
        document = self.example()
        grouping = next(
            card for card in document["cards"]
            if card.get("question", {}).get("kind") == "grouping"
        )
        grouping["question"]["items"][0]["categoryId"] = "missing"
        completed, written = self.run_writer(document, output=True)
        self.assertEqual(completed.returncode, 2)
        self.assertIsNone(written)
        self.assertIn("categoryId", completed.stderr)
        self.assertIn("available category", completed.stderr)

    def test_writer_rejects_unsupported_markdown_and_malformed_cloze(self):
        table = self.example()
        table["cards"][0]["front"] = "| A | B |\n| --- | --- |\n| 1 | 2 |"
        completed, _ = self.run_writer(table)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("Markdown tables", completed.stderr)

        cloze = self.example()
        cloze["cards"][1]["text"] = "Broken {{c1::outer {{c2::inner}}}}"
        completed, _ = self.run_writer(cloze)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("Cloze", completed.stderr)

    def test_writer_rejects_unknown_properties_and_version_drift(self):
        document = self.example()
        document["cards"][0]["internalId"] = "forbidden"
        completed, _ = self.run_writer(document)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("unknown property", completed.stderr)

        document = self.example()
        document["formatVersion"] = 1
        completed, _ = self.run_writer(document)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("format version 2", completed.stderr)

        document = self.example()
        document["formatVersion"] = 2.0
        completed, _ = self.run_writer(document)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("integer format version 2", completed.stderr)

    def test_skill_requires_deterministic_validation_and_no_mcp(self):
        source = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("write_ostudy.py", source)
        self.assertIn("do not look for, require, or call an MCP server", source)
        self.assertIn("do not return an unvalidated replacement assembled by hand", source)
        self.assertIn("Return the validated `.ostudy.json` as a downloadable file/attachment", source)


if __name__ == "__main__":
    unittest.main()
