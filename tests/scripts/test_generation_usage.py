"""Behavioral regressions for content-free generation accounting and reuse diagnostics."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("generation_usage", ROOT / "scripts/generation-usage.py")
USAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(USAGE)


class GenerationUsageTests(unittest.TestCase):
    def test_input_and_output_percentages_have_independent_denominators(self):
        report = USAGE.summarize({"operationId": "a" * 32, "requests": [
            {"stage": "knowledge_extraction", "inputTokens": 800, "outputTokens": 100,
             "reasoningTokens": 20, "cachedInputTokens": 600, "totalTokens": 900,
             "attempt": 1, "status": "response"},
            {"stage": "collection_guide_authoring", "inputTokens": 200, "outputTokens": 400,
             "reasoningTokens": 100, "cachedInputTokens": 0, "totalTokens": 600,
             "attempt": 2, "status": "response"},
        ]})
        rows = {row["stage"]: row for row in report["stages"]}
        self.assertEqual(rows["knowledge_extraction"]["percentOfReportedInput"], 80)
        self.assertEqual(rows["knowledge_extraction"]["percentOfReportedOutput"], 20)
        self.assertEqual(report["totals"]["inputTokens"], 1000)
        self.assertEqual(report["totals"]["outputTokens"], 500)
        self.assertEqual(report["totals"]["totalTokens"], 1500)
        self.assertEqual(report["totals"]["reasoningPercentOfCoveredOutput"], 24)
        self.assertEqual(report["totals"]["cacheReadPercentOfCoveredInput"], 60)
        self.assertEqual(report["totals"]["repeatedLogicalRequests"], 1)
        self.assertTrue(report["usageComplete"])

    def test_operation_wall_clock_uses_durable_operation_bounds(self):
        report = USAGE.summarize({
            "operationId": "a" * 32,
            "operation": {
                "id": "a" * 32,
                "purpose": "document_generation",
                "state": "succeeded",
                "startedAt": 1_000,
                "finishedAt": 13_500,
            },
            "requests": [],
        })
        self.assertEqual(report["wallClockMs"], 12_500)
        self.assertIn("Operation wall clock: 12.50s", USAGE.render(report))

    def test_unknown_usage_is_not_zero_or_fictitious_cache_savings(self):
        report = USAGE.summarize({"requests": [
            {"stage": "knowledge_extraction", "inputTokens": 100, "outputTokens": 20,
             "cachedInputTokens": None, "reasoningTokens": None, "status": "response"},
            {"stage": "collection_guide_authoring", "inputTokens": None,
             "outputTokens": None, "status": "interrupted"},
        ], "localReuse": [{"stage": "collection_guide_authoring", "count": 4}]})
        self.assertFalse(report["usageComplete"])
        self.assertEqual(report["totals"]["inputTokensMissingRequests"], 1)
        self.assertIsNone(report["totals"]["cacheReadPercentOfCoveredInput"])
        self.assertIsNone(report["totals"]["reasoningPercentOfCoveredOutput"])
        self.assertEqual(report["localReuse"][0]["count"], 4)
        self.assertIn("reported usage only", USAGE.render(report))

    def test_stage_with_entirely_missing_usage_is_unknown_not_zero_percent(self):
        report = USAGE.summarize({"requests": [
            {"stage": "guide_authoring", "inputTokens": 120, "outputTokens": 30,
             "attempt": 1, "status": "response"},
            {"stage": "embedding_indexing", "inputTokens": None, "outputTokens": None,
             "attempt": 1, "status": "interrupted"},
        ]})
        rows = {row["stage"]: row for row in report["stages"]}
        self.assertIsNone(rows["embedding_indexing"]["inputTokens"])
        self.assertIsNone(rows["embedding_indexing"]["percentOfReportedInput"])
        self.assertIsNone(rows["embedding_indexing"]["percentOfReportedOutput"])

    def test_retry_shares_and_payload_byte_diagnostics_are_separate_from_token_shares(self):
        report = USAGE.summarize({"requests": [
            {"stage": "guide_authoring", "inputTokens": 100, "outputTokens": 20,
             "attempt": 1, "status": "failed",
             "payloadBytesJson": json.dumps({
                 "serializedRequestBytes": 1000,
                 "systemInstructionBytes": 100,
                 "userParts": {"knowledgeUnits": 400, "embeddingInput": 0},
             })},
            {"stage": "guide_authoring", "inputTokens": 100, "outputTokens": 30,
             "attempt": 2, "status": "response",
             "payloadBytesJson": {
                 "serializedRequestBytes": 1200,
                 "systemInstructionBytes": 100,
                 "userParts": {"knowledgeUnits": 500, "embeddingInput": 0},
                 "response": {"visibleContentBytes": 300},
             }},
        ]})
        total = report["totals"]
        self.assertEqual(total["repeatInputPercentOfReportedInput"], 50)
        self.assertEqual(total["repeatOutputPercentOfReportedOutput"], 60)
        payload = report["payloadByteDiagnostics"]
        self.assertEqual(payload["coverageRequests"], 2)
        self.assertEqual(payload["serializedRequestBytes"], 2200)
        self.assertEqual(payload["inputParts"]["knowledgeUnits"]["bytes"], 900)
        self.assertEqual(payload["visibleResponseBytes"], 300)

    def test_retry_report_measures_tokens_and_overlapping_provider_time_without_calling_it_saved_time(self):
        report = USAGE.summarize({
            "operationId": "a" * 32,
            "operation": {
                "id": "a" * 32,
                "purpose": "document_generation",
                "state": "succeeded",
                "startedAt": 0,
                "finishedAt": 10_000,
            },
            "requests": [
                {"stage": "knowledge", "inputTokens": 100, "outputTokens": 10,
                 "attempt": 1, "status": "response", "startedAt": 0, "finishedAt": 4_000},
                {"stage": "knowledge", "inputTokens": 80, "outputTokens": 8,
                 "attempt": 2, "status": "response", "startedAt": 4_000, "finishedAt": 7_000},
                {"stage": "knowledge", "inputTokens": 70, "outputTokens": 7,
                 "attempt": 2, "status": "response", "startedAt": 5_000, "finishedAt": 9_000},
            ],
        })
        total = report["totals"]
        self.assertEqual(total["repeatReportedInputTokens"], 150)
        self.assertEqual(total["repeatCumulativeRequestMs"], 7_000)
        self.assertEqual(total["repeatActiveWallClockMs"], 5_000)
        self.assertEqual(total["cumulativeRequestMs"], 11_000)
        self.assertAlmostEqual(total["repeatDurationPercentOfCumulativeRequestMs"], 7000 / 110)
        self.assertEqual(report["repeatActivePercentOfOperationWallClock"], 50)
        rendered = USAGE.render(report)
        self.assertIn("cumulative 7.00s", rendered)
        self.assertIn("active wall-clock 5.00s", rendered)

    def test_all_operation_report_attributes_provider_tokens_to_each_operation(self):
        report = USAGE.summarize({"operationId": "all", "requests": [
            {"operationId": "a" * 32, "stage": "guide_authoring",
             "inputTokens": 900, "outputTokens": 100, "attempt": 1, "status": "response"},
            {"operationId": "b" * 32, "stage": "embedding_indexing",
             "inputTokens": 100, "outputTokens": 0, "attempt": 1, "status": "response"},
        ]})
        rows = {row["operationId"]: row for row in report["operations"]}
        self.assertEqual(rows["a" * 32]["percentOfReportedInput"], 90)
        self.assertEqual(rows["a" * 32]["percentOfReportedOutput"], 100)
        self.assertEqual(rows["b" * 32]["percentOfReportedInput"], 10)
        self.assertEqual(rows["b" * 32]["percentOfReportedOutput"], 0)

    def test_zero_request_reuse_has_no_token_percentages(self):
        report = USAGE.summarize({"requests": [], "localReuse": [{"stage": "collection_guide", "count": 1}]})
        self.assertEqual(report["totals"]["inputTokens"], 0)
        self.assertEqual(report["totals"]["outputTokens"], 0)
        self.assertEqual(report["stages"], [])
        self.assertTrue(report["usageComplete"])

    def test_migration_preserves_unknown_usage_and_source_owned_cache_lifetime(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "library.sqlite3"
            connection = sqlite3.connect(path)
            connection.execute("PRAGMA foreign_keys=ON")
            connection.executescript("""
                CREATE TABLE documents(id TEXT PRIMARY KEY);
                CREATE TABLE document_collections(id TEXT PRIMARY KEY);
                CREATE TABLE knowledge_sources(id TEXT PRIMARY KEY,content_hash TEXT,active INTEGER,extraction_contract TEXT);
                CREATE TABLE study_target_guides(id TEXT PRIMARY KEY);
            """)
            sql = (ROOT / "src-tauri/migrations/0027_generation_reuse_and_usage.sql").read_text()
            connection.executescript(sql)
            operation = "a" * 32
            collection = "b" * 32
            source = "c" * 32
            key = "d" * 64
            connection.execute("INSERT INTO document_collections VALUES(?)", (collection,))
            connection.execute("INSERT INTO knowledge_sources VALUES(?,?,1,'v1')", (source, "e" * 64))
            connection.execute("INSERT INTO generation_operations VALUES(?,?,NULL,?,1,NULL,'running')",
                               (operation, "collection_bundle", collection))
            connection.execute("""INSERT INTO generation_requests(
                id,operation_id,stage,request_key,attempt,provider,model,status,started_at,payload_bytes_json,max_output_tokens
                ) VALUES(?,?,?, ?,1,'test','test','started',1,'{}',6144)""",
                               ("f" * 32, operation, "collection_guide_authoring", key))
            connection.execute("INSERT INTO collection_guide_checkpoints VALUES(?,?,?,?,?,1)",
                               (key, collection, "e" * 64, '{}', "e" * 64))
            connection.execute("INSERT INTO collection_guide_checkpoint_sources VALUES(?,?)", (key, source))
            connection.commit()
            before = hashlib.sha256(path.read_bytes()).hexdigest()
            report = USAGE.summarize(USAGE.read_database(path, operation, False))
            self.assertFalse(report["usageComplete"])
            self.assertEqual(report["totals"]["failedOrInterruptedRequests"], 1)
            self.assertEqual(before, hashlib.sha256(path.read_bytes()).hexdigest())
            connection.execute("UPDATE knowledge_sources SET content_hash=? WHERE id=?", ("9" * 64, source))
            self.assertEqual(connection.execute("SELECT COUNT(*) FROM collection_guide_checkpoints").fetchone()[0], 0)
            connection.execute("DELETE FROM document_collections WHERE id=?", (collection,))
            self.assertEqual(connection.execute("SELECT COUNT(*) FROM generation_requests").fetchone()[0], 0)
            connection.close()

    def test_report_does_not_add_reasoning_or_cached_tokens_to_totals(self):
        data = {"requests": [{"stage": "guide", "inputTokens": 10, "outputTokens": 5,
                              "reasoningTokens": 3, "cachedInputTokens": 8, "totalTokens": 15}]}
        report = USAGE.summarize(data)
        self.assertEqual(report["totals"]["totalTokens"], 15)
        self.assertEqual(report["totals"]["inputTokens"] + report["totals"]["outputTokens"], 15)
        json.dumps(report, allow_nan=False)

    def test_active_library_path_is_resolved_without_fill_in_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.dict("os.environ", {"OSTUDY_APP_DATA_DIR": directory}):
                self.assertEqual(
                    USAGE.default_database_path(),
                    Path(directory) / "library.sqlite3",
                )


if __name__ == "__main__":
    unittest.main()
