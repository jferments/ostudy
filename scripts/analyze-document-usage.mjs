#!/usr/bin/env node

import fs from "node:fs";

const args = process.argv.slice(2);
const jsonOutput = args.includes("--json");
const paths = args.filter((value) => value !== "--json");

if (paths.length === 0) {
  console.error("Usage: node scripts/analyze-document-usage.mjs [--json] <ostudy-log> [more-logs...]");
  process.exit(2);
}

const records = [];
const usagePattern = /document_provider_usage.*provider=([^\s]+)\s+model=([^\s]+)\s+prompt_tokens=(\d+)\s+completion_tokens=(\d+)\s+cache_hit_tokens=(\d+)\s+cache_miss_tokens=(\d+)\s+reasoning_tokens=(\d+)/;
const planPattern = /document_ai_(additional_)?scale_plan.*document=([^\s]+).*provider=([^\s]+).*scale=([^\s]+).*?(?:pages|units)=(\d+).*source_tokens=(\d+).*generation_windows=(\d+).*estimated_job_tokens=(\d+).*ceiling=([^\s]+)(?:.*context=([^\s]+))?(?:.*calibration_samples=(\d+))?/;

for (const path of paths) {
  const text = fs.readFileSync(path, "utf8");
  for (const line of text.split(/\r?\n/)) {
    const usage = line.match(usagePattern);
    if (usage) {
      records.push({
        type: "usage",
        path,
        providerKind: usage[1],
        modelId: usage[2],
        promptTokens: Number(usage[3]),
        completionTokens: Number(usage[4]),
        cacheHitTokens: Number(usage[5]),
        cacheMissTokens: Number(usage[6]),
        reasoningTokens: Number(usage[7]),
      });
      continue;
    }
    const plan = line.match(planPattern);
    if (plan) {
      const ceilingLabel = plan[9];
      const ceilingTokens = /^\d+$/.test(ceilingLabel) ? Number(ceilingLabel) : null;
      records.push({
        type: "plan",
        path,
        pass: plan[1] ? "additional" : "initial",
        documentId: plan[2],
        providerKind: plan[3],
        scaleMode: plan[4],
        sourceUnitCount: Number(plan[5]),
        estimatedSourceTokens: Number(plan[6]),
        generationWindowCount: Number(plan[7]),
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
    promptTokens: 0,
    completionTokens: 0,
    reasoningTokens: 0,
    cacheHitTokens: 0,
    cacheMissTokens: 0,
  };
  group.calls += 1;
  group.promptTokens += usage.promptTokens;
  group.completionTokens += usage.completionTokens;
  group.reasoningTokens += usage.reasoningTokens;
  group.cacheHitTokens += usage.cacheHitTokens;
  group.cacheMissTokens += usage.cacheMissTokens;
  grouped.set(key, group);
}

const totals = [...grouped.values()].map((group) => ({
  ...group,
  aggregateTokens: group.promptTokens + group.completionTokens,
}));

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
        `${plan.generationWindowCount} generation windows, ${budget}, ` +
        `context=${plan.contextWindowTokens ?? "unknown"}, calibrationSamples=${plan.calibrationSampleCount}`,
    );
  }
}

console.log("Provider usage:");
if (totals.length === 0) {
  console.log("  No document_provider_usage records found.");
} else {
  for (const total of totals) {
    console.log(
      `  ${total.providerKind} ${total.modelId}: ${total.calls} calls, ` +
        `${total.promptTokens} prompt + ${total.completionTokens} completion = ` +
        `${total.aggregateTokens} aggregate tokens; reasoning=${total.reasoningTokens}, ` +
        `cacheHit=${total.cacheHitTokens}, cacheMiss=${total.cacheMissTokens}`,
    );
  }
}
