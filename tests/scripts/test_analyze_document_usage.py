import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ANALYZER = ROOT / "scripts" / "analyze-document-usage.mjs"


class AnalyzeDocumentUsageTests(unittest.TestCase):
    def test_structured_local_usage_plan_and_latency_are_reported(self) -> None:
        records = [
            {
                "event": "document_ai_scale_plan",
                "fields": {
                    "document": "doc",
                    "provider": "llama_cpp",
                    "model": "qwen-local",
                    "scale": "large",
                    "sourceUnits": 55,
                    "sourceTokens": 42000,
                    "outlineWindows": 4,
                    "generationWindows": 9,
                    "cardCapacity": 108,
                    "estimatedJobTokens": 150000,
                    "contextWindowTokens": 131072,
                    "calibrationSamples": 3,
                },
            },
            {
                "event": "document_ai_usage",
                "fields": {
                    "provider": "llama_cpp",
                    "model": "qwen-local",
                    "reported": True,
                    "promptTokens": 220,
                    "completionTokens": 45,
                    "totalTokens": 265,
                    "cachedPromptTokens": 180,
                    "cacheWriteTokens": None,
                    "reasoningTokens": 7,
                },
            },
            {
                "event": "document_ai_usage",
                "fields": {
                    "provider": "llama_cpp",
                    "model": "qwen-local",
                    "reported": False,
                    "promptTokens": None,
                    "completionTokens": None,
                    "totalTokens": None,
                    "cachedPromptTokens": None,
                    "cacheWriteTokens": None,
                    "reasoningTokens": None,
                },
            },
            {
                "event": "provider_stream_completed",
                "fields": {
                    "endpointHost": "127.0.0.1",
                    "upstreamProvider": None,
                    "model": "qwen-local",
                    "finishReason": "stop",
                    "durationMs": 12000,
                    "firstModelOutputMs": 2400,
                    "keepaliveEvents": 0,
                    "contentBytes": 900,
                    "toolArgumentBytes": 0,
                    "reasoningBytesObserved": 100,
                    "usageReported": True,
                },
            },
            {
                "event": "document_generation_window_completed",
                "fields": {
                    "provider": "llama_cpp",
                    "model": "qwen-local",
                    "window": 1,
                    "pageStart": 1,
                    "pageEnd": 7,
                    "sourceTokens": 4500,
                    "cards": 11,
                    "durationMs": 13000,
                },
            },
        ]
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "ostudy.jsonl"
            log.write_text("\n".join(json.dumps(record) for record in records) + "\n")
            completed = subprocess.run(
                ["node", str(ANALYZER), "--json", str(log)],
                cwd=ROOT,
                check=True,
                text=True,
                capture_output=True,
            )

        report = json.loads(completed.stdout)
        self.assertEqual(report["planCount"], 1)
        self.assertEqual(report["plans"][0]["outlineWindowCount"], 4)
        self.assertEqual(report["plans"][0]["generationWindowCount"], 9)
        self.assertEqual(report["plans"][0]["generationCardCapacity"], 108)

        usage = report["usageTotals"][0]
        self.assertEqual(usage["calls"], 2)
        self.assertEqual(usage["reportedCalls"], 1)
        self.assertEqual(usage["unreportedCalls"], 1)
        self.assertEqual(usage["promptTokens"], 220)
        self.assertEqual(usage["completionTokens"], 45)
        self.assertEqual(usage["totalTokens"], 265)
        self.assertEqual(usage["cacheHitTokens"], 180)
        self.assertEqual(usage["cacheMissTokens"], 40)
        self.assertEqual(usage["reasoningTokens"], 7)

        stream = report["streamTotals"][0]
        self.assertEqual(stream["calls"], 1)
        self.assertEqual(stream["averageDurationMs"], 12000)
        self.assertEqual(stream["averageFirstModelOutputMs"], 2400)
        self.assertEqual(stream["usageReportedCalls"], 1)

        window = report["windowTotals"][0]
        self.assertEqual(window["stage"], "generation")
        self.assertEqual(window["windows"], 1)
        self.assertEqual(window["cards"], 11)
        self.assertEqual(window["averageDurationMs"], 13000)


if __name__ == "__main__":
    unittest.main()
