from __future__ import annotations

import importlib.util
import json
import re
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WEBDRIVER_DIR = ROOT / "tests" / "webdriver"


def load_module(name: str, filename: str):
    sys.path.insert(0, str(WEBDRIVER_DIR))
    try:
        spec = importlib.util.spec_from_file_location(name, WEBDRIVER_DIR / filename)
        if spec is None or spec.loader is None:
            raise RuntimeError(f"Could not load {filename}")
        module = importlib.util.module_from_spec(spec)
        # Dataclasses resolve their module through sys.modules while loading.
        sys.modules[name] = module
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.remove(str(WEBDRIVER_DIR))


report = load_module("ostudy_ui_screenshots_report", "ui_screenshots_report.py")
seed = load_module("ostudy_ui_screenshots_seed", "ui_screenshots_seed.py")
capture = load_module("ostudy_ui_screenshots", "ui_screenshots.py")

SCREENS = [
    {"id": "library", "title": "Flashcards library", "group": "Flashcards"},
    {"id": "settings-appearance", "title": "Settings: Appearance", "group": "Settings"},
]
VIEWPORTS = [
    {"width": 390, "height": 844, "label": "Phone", "layout": "compact"},
    {"width": 1440, "height": 900, "label": "Desktop", "layout": "wide"},
]


def manifest_for(sha: str, captured_at: str = "2026-10-04T08:00:00Z") -> dict:
    return report.new_manifest(
        source={"sha": sha, "ref": "test"},
        screens=SCREENS,
        themes=["light", "dark"],
        viewports=VIEWPORTS,
        captured_at=captured_at,
        capture_date=captured_at[:10],
        environment={"frontend": "embedded"},
    )


def write_capture(directory: Path, manifest: dict, screen: str, theme: str, width: int, content: bytes,
                  status: str = "ok") -> None:
    filename = report.capture_filename(screen, theme, width)
    (directory / filename).write_bytes(content)
    report.add_capture(
        manifest,
        screen=screen,
        theme=theme,
        width=width,
        height=844 if width == 390 else 900,
        file=filename,
        status=status,
        sha256=report.sha256_bytes(content),
        pixel_sha256=report.sha256_bytes(b"pixels:" + content),
        byte_size=len(content),
        seconds=1.25,
    )


def fake_pair_writer(calls: list[tuple[str, str, str]]):
    def write(left: Path, right: Path, output: Path) -> None:
        calls.append((left.name, right.name, output.name))
        output.write_bytes(b"PAIR:" + left.read_bytes() + b"|" + right.read_bytes())

    return write


def page_payload(index: Path) -> dict:
    text = index.read_text(encoding="utf-8")
    match = re.search(r'<script id="data" type="application/json">(.*?)</script>', text, re.DOTALL)
    assert match is not None
    return json.loads(match.group(1))


class ManifestTests(unittest.TestCase):
    def test_capture_filename_is_predictable_and_rejects_unsafe_names(self) -> None:
        self.assertEqual(report.capture_filename("deck", "synthwave", 1024), "deck--synthwave--1024.webp")
        for screen, theme in (("../deck", "dark"), ("Deck", "dark"), ("deck", "dark theme"), ("", "dark")):
            with self.assertRaises(report.ManifestError):
                report.capture_filename(screen, theme, 390)

    def test_manifest_round_trip_records_source_sha_on_every_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = manifest_for("a" * 40)
            write_capture(directory, manifest, "library", "dark", 390, b"one")
            report.write_manifest(directory, manifest)

            loaded = report.load_manifest(directory)

        self.assertEqual(loaded["tool"], report.TOOL_ID)
        self.assertEqual(loaded["schemaVersion"], report.MANIFEST_SCHEMA_VERSION)
        capture_entry = loaded["captures"][0]
        self.assertEqual(
            {key: capture_entry[key] for key in ("screen", "theme", "width", "file", "sourceSha")},
            {"screen": "library", "theme": "dark", "width": 390, "file": "library--dark--390.webp",
             "sourceSha": "a" * 40},
        )

    def test_validation_rejects_inconsistent_manifests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = manifest_for("b" * 40)
            write_capture(directory, manifest, "library", "dark", 390, b"one")

            duplicate = json.loads(json.dumps(manifest))
            duplicate["captures"].append(dict(duplicate["captures"][0]))
            with self.assertRaisesRegex(report.ManifestError, "Duplicate capture"):
                report.validate_manifest(duplicate, directory)

            missing = json.loads(json.dumps(manifest))
            missing["captures"][0]["file"] = "absent.webp"
            with self.assertRaisesRegex(report.ManifestError, "missing"):
                report.validate_manifest(missing, directory)

            nested = json.loads(json.dumps(manifest))
            nested["captures"][0]["file"] = "../library--dark--390.webp"
            with self.assertRaisesRegex(report.ManifestError, "plain name"):
                report.validate_manifest(nested, directory)

            unknown = json.loads(json.dumps(manifest))
            unknown["captures"][0]["status"] = "maybe"
            with self.assertRaisesRegex(report.ManifestError, "status"):
                report.validate_manifest(unknown, directory)

            foreign = json.loads(json.dumps(manifest))
            foreign["tool"] = "something-else"
            with self.assertRaisesRegex(report.ManifestError, "Not a"):
                report.validate_manifest(foreign, directory)

            with self.assertRaisesRegex(report.ManifestError, "No manifest.json"):
                report.load_manifest(directory / "nowhere")

    def test_failed_captures_need_no_file(self) -> None:
        manifest = manifest_for("c" * 40)
        report.add_capture(manifest, screen="library", theme="dark", width=390, height=844, file=None,
                           status="failed", note="Timed out")
        report.validate_manifest(manifest)
        with self.assertRaises(report.ManifestError):
            report.add_capture(manifest, screen="library", theme="light", width=390, height=844, file=None,
                               status="broken")


class ComparisonTests(unittest.TestCase):
    def build_sides(self, root: Path) -> tuple[Path, Path]:
        before_dir, after_dir = root / "before", root / "after"
        before_dir.mkdir()
        after_dir.mkdir()
        before, after = manifest_for("1" * 40), manifest_for("2" * 40)
        # Unchanged: identical pixels on both sides.
        write_capture(before_dir, before, "library", "dark", 390, b"same")
        write_capture(after_dir, after, "library", "dark", 390, b"same")
        # Changed.
        write_capture(before_dir, before, "library", "dark", 1440, b"old")
        write_capture(after_dir, after, "library", "dark", 1440, b"new")
        # Removed and added.
        write_capture(before_dir, before, "settings-appearance", "light", 390, b"gone")
        write_capture(after_dir, after, "settings-appearance", "dark", 1440, b"fresh")
        # Failed after: the before image is still shown.
        write_capture(before_dir, before, "library", "light", 1440, b"was here")
        report.add_capture(after, screen="library", theme="light", width=1440, height=900, file=None,
                           status="failed", note="Timed out waiting for the library.")
        report.write_manifest(before_dir, before)
        report.write_manifest(after_dir, after)
        return before_dir, after_dir

    def test_classifies_pairs_by_pixel_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            before_dir, after_dir = self.build_sides(Path(temporary))
            items = report.compare_manifests(report.load_manifest(before_dir), report.load_manifest(after_dir))

        states = {item.key: item.state for item in items}
        self.assertEqual(states, {
            "library--dark--390": "unchanged",
            "library--dark--1440": "changed",
            "library--light--1440": "failed",
            "settings-appearance--light--390": "removed",
            "settings-appearance--dark--1440": "added",
        })
        # Catalogue order: screens first, then theme order from the matrix, then width.
        self.assertEqual([item.key for item in items][:3],
                         ["library--light--1440", "library--dark--390", "library--dark--1440"])

    def test_identical_encoded_files_with_different_pixels_still_count_as_changed(self) -> None:
        before, after = manifest_for("1" * 40), manifest_for("2" * 40)
        for manifest, pixels in ((before, "p1"), (after, "p2")):
            report.add_capture(manifest, screen="library", theme="dark", width=390, height=844,
                               file="library--dark--390.webp", sha256="same-file", pixel_sha256=pixels)
        self.assertEqual(report.compare_manifests(before, after)[0].state, "changed")

    def test_selection_limits_screens_themes_and_widths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            before_dir, after_dir = self.build_sides(Path(temporary))
            selection = report.Selection(screens=frozenset({"library"}), themes=frozenset({"dark"}),
                                         widths=frozenset({1440}))
            items = report.compare_manifests(report.load_manifest(before_dir), report.load_manifest(after_dir),
                                             selection)
        self.assertEqual([item.key for item in items], ["library--dark--1440"])

    def test_comparison_page_is_self_contained_and_collapses_unchanged_screens(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before_dir, after_dir = self.build_sides(root)
            output = root / "compare"
            calls: list[tuple[str, str, str]] = []

            summary = report.build_comparison(before_dir, after_dir, output,
                                              pair_writer=fake_pair_writer(calls), workers=1)

            self.assertEqual(summary["counts"], {"changed": 1, "unchanged": 1, "added": 1, "removed": 1,
                                                 "failed": 1})
            self.assertEqual(calls, [("library--dark--1440.webp", "library--dark--1440.webp",
                                      "library--dark--1440.pair.webp")])
            images = sorted(path.name for path in (output / "images").iterdir())
            self.assertEqual(images, [
                "library--dark--1440.pair.webp",
                "library--dark--390.webp",
                "library--light--1440.webp",
                "settings-appearance--dark--1440.webp",
                "settings-appearance--light--390.webp",
            ])
            self.assertEqual(summary["files"], len(images) + 1)
            self.assertEqual((output / "images" / "library--light--1440.webp").read_bytes(), b"was here")

            html = (output / "index.html").read_text(encoding="utf-8")
            self.assertNotRegex(html, r"<script[^>]+src=")
            self.assertNotRegex(html, r"<link[^>]+href=")
            self.assertNotIn("http://", html)
            self.assertNotIn("https://", html)
            self.assertIn("No change (", html)
            self.assertIn("@media (max-width: 900px)", html)

            payload = page_payload(output / "index.html")
            by_key = {f"{item['screen']}--{item['theme']}--{item['width']}": item for item in payload["items"]}
            self.assertEqual(by_key["library--dark--390"]["state"], "unchanged")
            self.assertEqual(by_key["library--dark--390"]["output"], {"single": "images/library--dark--390.webp"})
            self.assertEqual(by_key["library--dark--1440"]["output"],
                             {"pair": "images/library--dark--1440.pair.webp"})
            self.assertEqual(by_key["library--light--1440"]["note"], "Timed out waiting for the library.")
            self.assertEqual(payload["themes"], ["light", "dark"])
            self.assertEqual([width["label"] for width in payload["widths"]], ["Phone", "Desktop"])
            for item in payload["items"]:
                for path in item["output"].values():
                    self.assertTrue(path.startswith("images/"), path)
                    self.assertTrue((output / path).is_file(), path)

    def test_rebuilding_into_the_same_directory_replaces_old_images(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before_dir, after_dir = self.build_sides(root)
            output = root / "compare"
            (output / "images").mkdir(parents=True)
            (output / "images" / "stale.webp").write_bytes(b"old run")
            report.build_comparison(before_dir, after_dir, output, pair_writer=fake_pair_writer([]), workers=1)
            self.assertFalse((output / "images" / "stale.webp").exists())

    def test_file_limit_and_size_budget(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before_dir, after_dir = self.build_sides(root)
            with self.assertRaisesRegex(RuntimeError, "limit 3"):
                report.build_comparison(before_dir, after_dir, root / "small", pair_writer=fake_pair_writer([]),
                                        max_files=3, workers=1)
            summary = report.build_comparison(before_dir, after_dir, root / "budget",
                                              pair_writer=fake_pair_writer([]), max_bytes=10, workers=1)
            self.assertIn("above the", summary["warning"])

    def test_page_warns_about_different_capture_days_and_escapes_script_text(self) -> None:
        before, after = manifest_for("1" * 40), manifest_for("2" * 40, captured_at="2026-10-05T09:00:00Z")
        for manifest, pixels in ((before, "p1"), (after, "p2")):
            report.add_capture(manifest, screen="library", theme="dark", width=390, height=844,
                               file="library--dark--390.webp", pixel_sha256=pixels,
                               note="</script><script>alert(1)</script>")
        items = report.compare_manifests(before, after)
        page = report.render_comparison_page(before, after, items)
        self.assertEqual(page.count("</script>"), 2)
        self.assertIn("captured on different days", page)


class CatalogueTests(unittest.TestCase):
    def test_screen_catalogue_ids_make_valid_file_names(self) -> None:
        ids = [screen.id for screen in capture.SCREENS]
        self.assertEqual(len(ids), len(set(ids)))
        for screen in capture.SCREENS:
            for theme in capture.THEMES:
                for width in capture.screen_widths(screen, tuple(capture.VIEWPORTS)):
                    report.capture_filename(screen.id, theme, width)
        for screen in capture.SCREENS:
            # Page screens open through OPENERS; dialog and practice screens through their own flows.
            self.assertEqual(screen.flow == "page", screen.id in capture.OPENERS, screen.id)
        self.assertTrue(set(capture.SETTINGS_TABS) <= set(ids))

    def test_full_matrix_fits_the_comparison_file_budget(self) -> None:
        widths = tuple(capture.VIEWPORTS)
        total = sum(len(capture.screen_widths(screen, widths)) for screen in capture.SCREENS) * len(capture.THEMES)
        self.assertLess(total + 1, report.DEFAULT_MAX_FILES)

    def test_filters_accept_aliases_and_keep_catalogue_order(self) -> None:
        self.assertEqual(capture.parse_list("desktop,phone", tuple(capture.VIEWPORTS), "width",
                                            capture.width_alias), (390, 1440))
        self.assertEqual(capture.parse_screens("settings")[0], "settings-appearance")
        self.assertEqual(capture.parse_screens("practice,library"),
                         ("library", "practice-question", "practice-answer", "practice-summary"))
        with self.assertRaises(SystemExit):
            capture.parse_list("sepia", capture.THEMES, "theme")
        with self.assertRaises(SystemExit):
            capture.parse_list("800", tuple(capture.VIEWPORTS), "width", capture.width_alias)


class SeedContentTests(unittest.TestCase):
    def test_seed_content_is_consistent(self) -> None:
        decks = {deck.key for deck in seed.DECKS}
        keys = [note.key for note in seed.NOTES]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertTrue(30 <= len(seed.NOTES) <= 60)
        self.assertTrue(all(note.deck in decks for note in seed.NOTES))
        self.assertTrue({deck.folder for deck in seed.DECKS if deck.folder} <= set(seed.FOLDERS))
        renal = [note.key for note in seed.NOTES if note.deck == seed.PRACTICE_DECK]
        self.assertEqual(sorted(renal), sorted(seed.PRACTICE_ORDER))
        self.assertEqual(len(seed.PRACTICE_RATINGS), len(seed.PRACTICE_ORDER))
        for _days_ago, _minute, deck, note_keys in seed.HISTORY:
            self.assertIn(deck, decks)
            self.assertTrue(set(note_keys) <= set(keys))
        self.assertEqual(seed.note_spec(seed.EDITOR_NOTE).deck, seed.DETAIL_DECK)
        self.assertTrue(seed.question_text(seed.note_spec(seed.EDITOR_NOTE)))

    def test_identifiers_are_stable(self) -> None:
        self.assertEqual(seed.stable_hex("note", "x"), seed.stable_hex("note", "x"))
        self.assertNotEqual(seed.stable_hex("note", "x"), seed.stable_hex("note", "y"))
        self.assertRegex(seed.stable_hex("anything"), r"^[0-9a-f]{32}$")

    def test_tag_ids_become_fixed_and_follow_tag_names(self) -> None:
        def library(random_ids: list[str]) -> sqlite3.Connection:
            connection = sqlite3.connect(":memory:")
            connection.execute("PRAGMA foreign_keys = ON")
            connection.executescript(
                """
                CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT NOT NULL, normalized_name TEXT NOT NULL UNIQUE);
                CREATE TABLE note_tags (note_id TEXT NOT NULL,
                    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE, PRIMARY KEY(note_id, tag_id));
                CREATE TABLE practice_session_card_tag_snapshots (session_id TEXT NOT NULL, card_id TEXT NOT NULL,
                    tag_id TEXT NOT NULL, PRIMARY KEY(session_id, card_id, tag_id));
                CREATE TABLE review_event_tag_attributions (review_event_id TEXT NOT NULL, tag_id TEXT NOT NULL,
                    PRIMARY KEY(review_event_id, tag_id));
                CREATE TABLE practice_sessions (id TEXT PRIMARY KEY, selected_tag_ids_json TEXT NOT NULL);
                """
            )
            with connection:
                for tag_id, name in zip(random_ids, names, strict=True):
                    connection.execute("INSERT INTO tags VALUES (?1, ?2, ?3)", (tag_id, name, name.lower()))
                    connection.execute("INSERT INTO note_tags VALUES ('note', ?1)", (tag_id,))
                    connection.execute("INSERT INTO practice_session_card_tag_snapshots VALUES ('s', 'c', ?1)",
                                       (tag_id,))
                    connection.execute("INSERT INTO review_event_tag_attributions VALUES ('r', ?1)", (tag_id,))
                connection.execute("INSERT INTO practice_sessions VALUES ('s', ?1)", (json.dumps(random_ids[:1]),))
                connection.execute("INSERT INTO practice_sessions VALUES ('t', '[]')")
            return connection

        def tag_line(connection: sqlite3.Connection) -> str:
            # The deck card list's query: group_concat follows the tag-id index.
            return connection.execute(
                "SELECT group_concat(t.name, ' ') FROM note_tags nt JOIN tags t ON t.id = nt.tag_id"
                " WHERE nt.note_id = 'note' ORDER BY t.name"
            ).fetchone()[0]

        names = ["exam-1", "Cardiovascular", "renal"]
        first = library(["f" * 32, "e" * 32, "0" * 32])
        second = library(["1" * 32, "a" * 32, "b" * 32])
        self.assertNotEqual(tag_line(first), tag_line(second))
        for connection in (first, second):
            seed.stabilize_tag_ids(connection)
            self.assertEqual(tag_line(connection), "Cardiovascular exam-1 renal")
            self.assertEqual(connection.execute("PRAGMA foreign_key_check").fetchall(), [])
        ids = {
            table: first.execute(f"SELECT {column} FROM {table} ORDER BY {column}").fetchall()
            for table, column in seed.TAG_ID_COLUMNS
        }
        self.assertEqual(ids, {
            table: second.execute(f"SELECT {column} FROM {table} ORDER BY {column}").fetchall()
            for table, column in seed.TAG_ID_COLUMNS
        })
        fixed = [row[0] for row in ids["tags"]]
        self.assertEqual(fixed, seed.fixed_tag_ids(["cardiovascular", "exam-1", "renal"]))
        self.assertTrue(all(re.fullmatch(r"[0-9a-f]{32}", value) for value in fixed))
        selected = dict(first.execute("SELECT id, selected_tag_ids_json FROM practice_sessions").fetchall())
        self.assertEqual(selected, {"s": json.dumps([fixed[1]], separators=(",", ":")), "t": "[]"})

    def test_seeded_pdf_has_a_consistent_cross_reference_table(self) -> None:
        pdf = seed.build_pdf("Sample", (("Page one", ("Line A", "Line B")), ("Page two", ("Line C",))))
        self.assertTrue(pdf.startswith(b"%PDF-1."))
        self.assertTrue(pdf.rstrip().endswith(b"%%EOF"))
        self.assertEqual(pdf, seed.build_pdf("Sample", (("Page one", ("Line A", "Line B")),
                                                        ("Page two", ("Line C",)))))
        startxref = int(pdf[pdf.rindex(b"startxref") + len(b"startxref"):].split()[0])
        self.assertTrue(pdf[startxref:].startswith(b"xref"))
        offsets = [int(line.split()[0]) for line in pdf[startxref:].split(b"\n")[3:] if line.endswith((b" n ", b" n"))]
        for number, offset in enumerate(offsets, start=1):
            self.assertTrue(pdf[offset:].startswith(f"{number} 0 obj".encode()), number)


class FakeApp:
    """Answers the seeding commands the way the native layer shapes its results."""

    def __init__(self) -> None:
        self.revision = 7
        self.folders: list[dict] = []
        self.cards: list[dict] = []
        self.calls: list[str] = []
        self.moves: list[dict] = []
        self.counter = 0

    def new_id(self, prefix: str) -> str:
        self.counter += 1
        return f"{prefix}-{self.counter}"

    def organization(self) -> dict:
        return {"revision": self.revision, "folders": list(self.folders)}

    def batch(self, steps: list[dict]) -> list:
        results: list = []
        last = None
        for step in steps:
            command, args = step["command"], json.loads(json.dumps(step["args"]))
            self.calls.append(command)
            if step["chainRevision"]:
                # The page copies the previous result's revision into the input.
                assert last is not None
                args["input"]["expectedOrganizationRevision"] = last["revision"]
                if args["input"]["expectedOrganizationRevision"] != self.revision:
                    raise AssertionError(f"{command} used a stale organization revision")
            last = self.run(command, args)
            results.append(last)
        return results

    def run(self, command: str, args: dict):
        if command == "get_deck_library_organization":
            return self.organization()
        if command == "create_deck_folder":
            self.folders.append({"id": self.new_id("folder"), "name": args["input"]["name"]})
            self.revision += 1
            return self.organization()
        if command == "create_deck":
            return {"id": self.new_id("deck"), "name": args["input"]["name"]}
        if command == "move_deck_in_library":
            self.moves.append(args["input"])
            self.revision += 1
            return self.organization()
        if command in ("create_card_note", "save_question_note"):
            note_id = self.new_id("note")
            source = args["input"]
            if command == "save_question_note":
                labels = ["Question"]
            elif source["cardType"] == "cloze":
                labels = sorted(set(re.findall(r"\{\{(c\d+)::", source["clozeSource"])), reverse=True)
            else:
                labels = ["Reverse", "Forward"] if source["reverseEnabled"] else ["Forward"]
            card_ids = []
            for label in labels:
                card_ids.append(self.new_id("card"))
                self.cards.insert(0, {"noteId": note_id, "cardLabel": label, "cardId": card_ids[-1]})
            return {"noteId": note_id, "cardIds": card_ids} if command == "save_question_note" else {"noteId": note_id}
        if command == "search_cards":
            offset, limit = args["input"]["offset"], args["input"]["limit"]
            return {"cards": self.cards[offset:offset + limit], "totalCount": len(self.cards)}
        if command in ("create_card_template", "create_tutor_conversation"):
            return {"id": self.new_id(command)}
        raise AssertionError(f"unexpected command {command}")

    def practice(self, plans: list[dict]) -> list[dict]:
        self.calls.append(f"practice x{len(plans)}")
        for plan in plans:
            self.assert_plan(plan)
        return [{"sessionId": self.new_id("session"), "order": list(plan["cardIds"])} for plan in plans]

    @staticmethod
    def assert_plan(plan: dict) -> None:
        cards = plan["cardIds"]
        assert cards and len(set(cards)) == len(cards)
        assert set(plan["outcomes"]) == set(cards) == set(plan["responseTimes"])
        assert set(plan["outcomes"].values()) <= set(seed.OUTCOMES)
        assert len(plan["submissionIds"]) == len(cards)


class SeedOrchestrationTests(unittest.TestCase):
    def test_seeding_batches_commands_and_maps_every_note_to_cards(self) -> None:
        app = FakeApp()
        state = seed.seed_through_ipc(app.batch, app.practice, chunk=9)

        self.assertEqual(set(state.folder_ids), set(seed.FOLDERS))
        self.assertEqual(set(state.deck_ids), {deck.key for deck in seed.DECKS})
        self.assertEqual(set(state.card_ids), {note.key for note in seed.NOTES})
        self.assertTrue(all(state.card_ids.values()))
        self.assertEqual(len(state.sessions), len(seed.HISTORY))
        self.assertTrue(state.template_id and state.tutor_conversation_id)
        # Cloze and reverse notes list their cards by presentation label.
        self.assertEqual(len(state.card_ids["cardio-output"]), 2)
        # Folder moves fill each folder from position zero in catalogue order.
        for folder in seed.FOLDERS:
            decks = [deck.key for deck in seed.DECKS if deck.folder == folder]
            moves = [move for move in app.moves if move["targetFolderId"] == state.folder_ids[folder]]
            self.assertEqual([move["targetIndex"] for move in moves], list(range(len(decks))))
            self.assertEqual([move["deckId"] for move in moves], [state.deck_ids[key] for key in decks])
        # Practice runs in a few large batches rather than one round trip per card.
        practice_calls = [call for call in app.calls if call.startswith("practice")]
        self.assertEqual(len(practice_calls), -(-len(seed.HISTORY) // 9))
        self.assertEqual(seed.SeedState.from_json(json.loads(json.dumps(state.to_json()))), state)

    def test_practice_plans_are_reproducible(self) -> None:
        app = FakeApp()
        state = seed.seed_through_ipc(app.batch, app.practice)
        self.assertEqual(seed.practice_plans(state), seed.practice_plans(state))
        first = seed.practice_plans(state)[0]
        self.assertEqual(first["operationId"], seed.stable_hex("practice", "0"))


if __name__ == "__main__":
    unittest.main()
