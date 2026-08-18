#!/usr/bin/env python3
"""One-shot refactor for structured document usage/plan analysis."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    count = value.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(value.replace(old, new, 1))


replace_once(
    "src-tauri/src/ai/document_intelligence.rs",
    '''fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let reported = total_tokens.is_some();
    if let Ok(mut usage) = task.usage.lock() {
        if let Some(total) = total_tokens {
            usage.total_tokens = usage
                .total_tokens
                .saturating_add(i64::try_from(total).unwrap_or(i64::MAX));
            usage.reported_requests = usage.reported_requests.saturating_add(1);
        } else {
            usage.unreported_requests = usage.unreported_requests.saturating_add(1);
        }
    }
    crate::logging::debug_target(
        "ai.document.usage",
        "document_ai_usage",
        "Recorded provider token usage without logging prompt or model content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "reported": reported,
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
        }),
    );
}''',
    '''fn record_document_usage(task: &DocumentTaskContext, response: &Value) {
    let (prompt_tokens, completion_tokens, total_tokens) = response_token_usage(response);
    let cached_prompt_tokens = response
        .pointer("/usage/prompt_tokens_details/cached_tokens")
        .and_then(nonnegative_json_integer);
    let cache_write_tokens = response
        .pointer("/usage/prompt_tokens_details/cache_write_tokens")
        .and_then(nonnegative_json_integer);
    let reasoning_tokens = response
        .pointer("/usage/completion_tokens_details/reasoning_tokens")
        .and_then(nonnegative_json_integer);
    let reported = total_tokens.is_some();
    if let Ok(mut usage) = task.usage.lock() {
        if let Some(total) = total_tokens {
            usage.total_tokens = usage
                .total_tokens
                .saturating_add(i64::try_from(total).unwrap_or(i64::MAX));
            usage.reported_requests = usage.reported_requests.saturating_add(1);
        } else {
            usage.unreported_requests = usage.unreported_requests.saturating_add(1);
        }
    }
    crate::logging::debug_target(
        "ai.document.usage",
        "document_ai_usage",
        "Recorded provider token usage without logging prompt or model content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "reported": reported,
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
            "cachedPromptTokens": cached_prompt_tokens,
            "cacheWriteTokens": cache_write_tokens,
            "reasoningTokens": reasoning_tokens,
        }),
    );
}''',
    "complete structured usage diagnostics",
)

replace_once(
    "src-tauri/src/document_more.rs",
    '''    crate::logging::info(
        "document_ai_additional_scale_plan",
        format!(
            "document={} format={} provider={} model={} scale={} units={} source_tokens={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={} context={} calibration_samples={}",
            input.document_id,
            source.format.as_str(),
            task.provider_kind,
            task.model_id,
            plan.label(),
            source.unit_count,
            estimated_source_tokens,
            windows.len(),
            budget.generation_card_capacity,
            budget.estimated_tokens,
            budget.ceiling_tokens,
            budget.context_window_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            budget.calibration_sample_count,
        ),
    );''',
    '''    crate::logging::debug_target(
        "ai.document.plan",
        "document_ai_additional_scale_plan",
        "Planned bounded additional document-AI work without logging source content.",
        serde_json::json!({
            "pass": "additional",
            "document": input.document_id,
            "format": source.format.as_str(),
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "scale": plan.label(),
            "sourceUnits": source.unit_count,
            "sourceTokens": estimated_source_tokens,
            "outlineWindows": 0,
            "generationWindows": windows.len(),
            "cardCapacity": budget.generation_card_capacity,
            "estimatedJobTokens": budget.estimated_tokens,
            "aggregateBudgetMode": budget.ceiling_tokens.to_string(),
            "contextWindowTokens": budget.context_window_tokens,
            "calibrationSamples": budget.calibration_sample_count,
        }),
    );''',
    "structured additional-generation plan diagnostics",
)

analyzer = r'''#!/usr/bin/env node

import fs from "node:fs";

const args = process.argv.slice(2);
const jsonOutput = args.includes("--json");
const paths = args.filter((value) => value !== "--json");

if (paths.length === 0) {
  console.error("Usage: node scripts/analyze-document-usage.mjs [--json] <ostudy-log> [more-logs...]");
  process.exit(2);
}

const records = [];
const legacyUsagePattern = /document_provider_usage.*provider=([^\s]+)\s+model=([^\s]+)\s+prompt_tokens=(\d+)\s+completion_tokens=(\d+)\s+cache_hit_tokens=(\d+)\s+cache_miss_tokens=(\d+)\s+reasoning_tokens=(\d+)/;
const legacyPlanPattern = /document_ai_(additional_)?scale_plan.*document=([^\s]+).*provider=([^\s]+).*scale=([^\s]+).*?(?:pages|units)=(\d+).*source_tokens=(\d+).*generation_windows=(\d+).*estimated_job_tokens=(\d+).*ceiling=([^\s]+)(?:.*context=([^\s]+))?(?:.*calibration_samples=(\d+))?/;

function optionalNumber(value) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function requiredNumber(value) {
  return optionalNumber(value) ?? 0;
}

function parseStructuredRecord(line, path) {
  let record;
  try {
    record = JSON.parse(line);
  } catch {
    return false;
  }
  if (!record || typeof record !== "object" || !record.fields || typeof record.fields !== "object") {
    return false;
  }
  const fields = record.fields;
  if (record.event === "document_ai_usage") {
    const promptTokens = optionalNumber(fields.promptTokens);
    const cachedPromptTokens = optionalNumber(fields.cachedPromptTokens) ?? 0;
    records.push({
      type: "usage",
      path,
      providerKind: String(fields.provider ?? "unknown"),
      modelId: String(fields.model ?? "unknown"),
      reported: fields.reported === true,
      promptTokens,
      completionTokens: optionalNumber(fields.completionTokens),
      totalTokens: optionalNumber(fields.totalTokens),
      cacheHitTokens: cachedPromptTokens,
      cacheMissTokens: promptTokens === null ? 0 : Math.max(0, promptTokens - cachedPromptTokens),
      cacheWriteTokens: optionalNumber(fields.cacheWriteTokens) ?? 0,
      reasoningTokens: optionalNumber(fields.reasoningTokens) ?? 0,
    });
    return true;
  }
  if (record.event === "document_ai_scale_plan" || record.event === "document_ai_additional_scale_plan") {
    records.push({
      type: "plan",
      path,
      pass: String(fields.pass ?? (record.event.includes("additional") ? "additional" : "initial")),
      documentId: String(fields.document ?? "unknown"),
      providerKind: String(fields.provider ?? "unknown"),
      modelId: String(fields.model ?? "unknown"),
      scaleMode: String(fields.scale ?? "unknown"),
      sourceUnitCount: requiredNumber(fields.sourceUnits),
      estimatedSourceTokens: requiredNumber(fields.sourceTokens),
      outlineWindowCount: requiredNumber(fields.outlineWindows),
      generationWindowCount: requiredNumber(fields.generationWindows),
      generationCardCapacity: requiredNumber(fields.cardCapacity),
      estimatedJobTokens: requiredNumber(fields.estimatedJobTokens),
      ceilingTokens: optionalNumber(fields.ceilingTokens),
      aggregateBudgetMode: String(fields.aggregateBudgetMode ?? "windowed"),
      contextWindowTokens: optionalNumber(fields.contextWindowTokens),
      calibrationSampleCount: requiredNumber(fields.calibrationSamples),
    });
    return true;
  }
  return false;
}

for (const path of paths) {
  const text = fs.readFileSync(path, "utf8");
  for (const line of text.split(/\r?\n/)) {
    if (parseStructuredRecord(line, path)) continue;

    const usage = line.match(legacyUsagePattern);
    if (usage) {
      const promptTokens = Number(usage[3]);
      const completionTokens = Number(usage[4]);
      records.push({
        type: "usage",
        path,
        providerKind: usage[1],
        modelId: usage[2],
        reported: true,
        promptTokens,
        completionTokens,
        totalTokens: promptTokens + completionTokens,
        cacheHitTokens: Number(usage[5]),
        cacheMissTokens: Number(usage[6]),
        cacheWriteTokens: 0,
        reasoningTokens: Number(usage[7]),
      });
      continue;
    }

    const plan = line.match(legacyPlanPattern);
    if (plan) {
      const ceilingLabel = plan[9];
      const ceilingTokens = /^\d+$/.test(ceilingLabel) ? Number(ceilingLabel) : null;
      records.push({
        type: "plan",
        path,
        pass: plan[1] ? "additional" : "initial",
        documentId: plan[2],
        providerKind: plan[3],
        modelId: "unknown",
        scaleMode: plan[4],
        sourceUnitCount: Number(plan[5]),
        estimatedSourceTokens: Number(plan[6]),
        outlineWindowCount: 0,
        generationWindowCount: Number(plan[7]),
        generationCardCapacity: 0,
        estimatedJobTokens: Number(plan[8]),
        ceilingTokens,
        aggregateBudgetMode: ceilingTokens === null ? ceilingLabel : "capped",
        contextWindowTokens: plan[10] && plan[10] !== "unknown" ? Number(plan[10]) : null,
        calibrationSampleCount: plan[11] ? Number(plan[11]) : 0,
      });
    }
  }
}

const usages = records.filter((record) => record.type === "usage");
const plans = records.filter((record) => record.type === "plan");

const grouped = new Map();
for (const usage of usages) {
  const key = `${usage.providerKind}\u0000${usage.modelId}`;
  const group = grouped.get(key) ?? {
    providerKind: usage.providerKind,
    modelId: usage.modelId,
    calls: 0,
    reportedCalls: 0,
    unreportedCalls: 0,
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
    reasoningTokens: 0,
    cacheHitTokens: 0,
    cacheMissTokens: 0,
    cacheWriteTokens: 0,
  };
  group.calls += 1;
  if (usage.reported) group.reportedCalls += 1;
  else group.unreportedCalls += 1;
  group.promptTokens += usage.promptTokens ?? 0;
  group.completionTokens += usage.completionTokens ?? 0;
  group.totalTokens += usage.totalTokens ?? 0;
  group.reasoningTokens += usage.reasoningTokens;
  group.cacheHitTokens += usage.cacheHitTokens;
  group.cacheMissTokens += usage.cacheMissTokens;
  group.cacheWriteTokens += usage.cacheWriteTokens;
  grouped.set(key, group);
}

const totals = [...grouped.values()];
const report = {
  files: paths,
  planCount: plans.length,
  providerCallCount: usages.length,
  plans,
  totals,
};

if (jsonOutput) {
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  process.exit(0);
}

if (plans.length > 0) {
  console.log("Document AI plans:");
  for (const plan of plans) {
    const budget = plan.ceilingTokens === null
      ? `${plan.estimatedJobTokens} aggregate tokens, ${plan.aggregateBudgetMode} execution`
      : `estimated ${plan.estimatedJobTokens}/${plan.ceilingTokens} aggregate tokens`;
    console.log(
      `  ${plan.pass} ${plan.providerKind} ${plan.scaleMode}: ` +
        `${plan.sourceUnitCount} units, ~${plan.estimatedSourceTokens} source tokens, ` +
        `${plan.outlineWindowCount} outline + ${plan.generationWindowCount} generation windows, ` +
        `${budget}, context=${plan.contextWindowTokens ?? "unknown"}, ` +
        `calibrationSamples=${plan.calibrationSampleCount}`,
    );
  }
}

console.log("Provider usage:");
if (totals.length === 0) {
  console.log("  No document_ai_usage or legacy document_provider_usage records found.");
} else {
  for (const total of totals) {
    console.log(
      `  ${total.providerKind} ${total.modelId}: ${total.calls} calls ` +
        `(${total.reportedCalls} reported, ${total.unreportedCalls} unreported), ` +
        `${total.promptTokens} prompt + ${total.completionTokens} completion = ` +
        `${total.totalTokens} total tokens; reasoning=${total.reasoningTokens}, ` +
        `cacheHit=${total.cacheHitTokens}, cacheMiss=${total.cacheMissTokens}, ` +
        `cacheWrite=${total.cacheWriteTokens}`,
    );
  }
}
'''
Path("scripts/analyze-document-usage.mjs").write_text(analyzer)

test = r'''import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ANALYZER = ROOT / "scripts" / "analyze-document-usage.mjs"


class AnalyzeDocumentUsageTests(unittest.TestCase):
    def test_structured_local_usage_and_plan_are_reported(self) -> None:
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
                    "cardCapacity": 90,
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
        self.assertEqual(report["totals"][0]["calls"], 2)
        self.assertEqual(report["totals"][0]["reportedCalls"], 1)
        self.assertEqual(report["totals"][0]["unreportedCalls"], 1)
        self.assertEqual(report["totals"][0]["promptTokens"], 220)
        self.assertEqual(report["totals"][0]["completionTokens"], 45)
        self.assertEqual(report["totals"][0]["totalTokens"], 265)
        self.assertEqual(report["totals"][0]["cacheHitTokens"], 180)
        self.assertEqual(report["totals"][0]["cacheMissTokens"], 40)
        self.assertEqual(report["totals"][0]["reasoningTokens"], 7)


if __name__ == "__main__":
    unittest.main()
'''
Path("tests/scripts/test_analyze_document_usage.py").write_text(test)

print("PR #54 structured usage diagnostics refactor applied.")
