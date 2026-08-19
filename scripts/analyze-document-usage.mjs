#!/usr/bin/env node

import fs from "node:fs";

const args = process.argv.slice(2);
const jsonOutput = args.includes("--json");
const paths = args.filter((value) => value !== "--json");

if (paths.length === 0) {
  console.error(
    "Usage: node scripts/analyze-document-usage.mjs [--json] <ostudy-log> [more-logs...]",
  );
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

function optionalString(value) {
  return typeof value === "string" && value.length > 0 ? value : null;
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
  switch (record.event) {
    case "document_ai_usage": {
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
    case "document_ai_scale_plan":
    case "document_ai_additional_scale_plan":
      records.push({
        type: "plan",
        path,
        pass: String(
          fields.pass ?? (record.event === "document_ai_additional_scale_plan" ? "additional" : "initial"),
        ),
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
    case "provider_stream_completed":
      records.push({
        type: "stream",
        path,
        endpointHost: String(fields.endpointHost ?? "unknown"),
        upstreamProvider: optionalString(fields.upstreamProvider),
        modelId: String(fields.model ?? "unknown"),
        finishReason: optionalString(fields.finishReason),
        durationMs: requiredNumber(fields.durationMs),
        firstModelOutputMs: optionalNumber(fields.firstModelOutputMs),
        keepaliveEvents: requiredNumber(fields.keepaliveEvents),
        contentBytes: requiredNumber(fields.contentBytes),
        toolArgumentBytes: requiredNumber(fields.toolArgumentBytes),
        reasoningBytesObserved: requiredNumber(fields.reasoningBytesObserved),
        usageReported: fields.usageReported === true,
      });
      return true;
    case "document_outline_window_completed":
    case "document_generation_window_completed":
      records.push({
        type: "window",
        path,
        stage: record.event === "document_outline_window_completed" ? "outline" : "generation",
        providerKind: String(fields.provider ?? "unknown"),
        modelId: String(fields.model ?? "unknown"),
        window: requiredNumber(fields.window),
        pageStart: requiredNumber(fields.pageStart),
        pageEnd: requiredNumber(fields.pageEnd),
        sourceTokens: requiredNumber(fields.sourceTokens),
        durationMs: requiredNumber(fields.durationMs),
        cards: optionalNumber(fields.cards),
      });
      return true;
    case "document_generation_window_failed":
      records.push({
        type: "failure",
        path,
        stage: "generation",
        providerKind: String(fields.provider ?? "unknown"),
        modelId: String(fields.model ?? "unknown"),
        window: requiredNumber(fields.window),
        pageStart: requiredNumber(fields.pageStart),
        pageEnd: requiredNumber(fields.pageEnd),
        durationMs: requiredNumber(fields.durationMs),
        code: String(fields.code ?? "unknown"),
      });
      return true;
    default:
      return false;
  }
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
const streams = records.filter((record) => record.type === "stream");
const windows = records.filter((record) => record.type === "window");
const failures = records.filter((record) => record.type === "failure");

function groupBy(items, keyFor, create, accumulate) {
  const grouped = new Map();
  for (const item of items) {
    const key = keyFor(item);
    const group = grouped.get(key) ?? create(item);
    accumulate(group, item);
    grouped.set(key, group);
  }
  return [...grouped.values()];
}

const usageTotals = groupBy(
  usages,
  (usage) => `${usage.providerKind}\u0000${usage.modelId}`,
  (usage) => ({
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
  }),
  (group, usage) => {
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
  },
);

const streamTotals = groupBy(
  streams,
  (stream) => `${stream.endpointHost}\u0000${stream.upstreamProvider ?? ""}\u0000${stream.modelId}`,
  (stream) => ({
    endpointHost: stream.endpointHost,
    upstreamProvider: stream.upstreamProvider,
    modelId: stream.modelId,
    calls: 0,
    totalDurationMs: 0,
    maxDurationMs: 0,
    firstModelOutputSamples: 0,
    totalFirstModelOutputMs: 0,
    maxFirstModelOutputMs: 0,
    keepaliveEvents: 0,
    usageReportedCalls: 0,
  }),
  (group, stream) => {
    group.calls += 1;
    group.totalDurationMs += stream.durationMs;
    group.maxDurationMs = Math.max(group.maxDurationMs, stream.durationMs);
    if (stream.firstModelOutputMs !== null) {
      group.firstModelOutputSamples += 1;
      group.totalFirstModelOutputMs += stream.firstModelOutputMs;
      group.maxFirstModelOutputMs = Math.max(group.maxFirstModelOutputMs, stream.firstModelOutputMs);
    }
    group.keepaliveEvents += stream.keepaliveEvents;
    if (stream.usageReported) group.usageReportedCalls += 1;
  },
).map((group) => ({
  ...group,
  averageDurationMs: group.calls === 0 ? 0 : Math.round(group.totalDurationMs / group.calls),
  averageFirstModelOutputMs:
    group.firstModelOutputSamples === 0
      ? null
      : Math.round(group.totalFirstModelOutputMs / group.firstModelOutputSamples),
}));

const windowTotals = groupBy(
  windows,
  (window) => `${window.providerKind}\u0000${window.modelId}\u0000${window.stage}`,
  (window) => ({
    providerKind: window.providerKind,
    modelId: window.modelId,
    stage: window.stage,
    windows: 0,
    totalDurationMs: 0,
    maxDurationMs: 0,
    cards: 0,
  }),
  (group, window) => {
    group.windows += 1;
    group.totalDurationMs += window.durationMs;
    group.maxDurationMs = Math.max(group.maxDurationMs, window.durationMs);
    group.cards += window.cards ?? 0;
  },
).map((group) => ({
  ...group,
  averageDurationMs: group.windows === 0 ? 0 : Math.round(group.totalDurationMs / group.windows),
}));

const report = {
  files: paths,
  planCount: plans.length,
  providerCallCount: usages.length,
  streamCount: streams.length,
  completedWindowCount: windows.length,
  failureCount: failures.length,
  plans,
  usageTotals,
  streamTotals,
  windowTotals,
  failures,
};

if (jsonOutput) {
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  process.exit(0);
}

function formatDuration(milliseconds) {
  if (milliseconds < 1_000) return `${milliseconds}ms`;
  if (milliseconds < 60_000) return `${(milliseconds / 1_000).toFixed(1)}s`;
  return `${(milliseconds / 60_000).toFixed(1)}m`;
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
if (usageTotals.length === 0) {
  console.log("  No document_ai_usage or legacy document_provider_usage records found.");
} else {
  for (const total of usageTotals) {
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

if (streamTotals.length > 0) {
  console.log("Provider stream latency:");
  for (const total of streamTotals) {
    console.log(
      `  ${total.endpointHost} ${total.upstreamProvider ?? "direct"} ${total.modelId}: ` +
        `${total.calls} calls, avg=${formatDuration(total.averageDurationMs)}, ` +
        `max=${formatDuration(total.maxDurationMs)}, first output avg=` +
        `${total.averageFirstModelOutputMs === null ? "unreported" : formatDuration(total.averageFirstModelOutputMs)}, ` +
        `first output max=${formatDuration(total.maxFirstModelOutputMs)}, ` +
        `heartbeats=${total.keepaliveEvents}, usage=${total.usageReportedCalls}/${total.calls}`,
    );
  }
}

if (windowTotals.length > 0) {
  console.log("Document window latency:");
  for (const total of windowTotals) {
    console.log(
      `  ${total.providerKind} ${total.modelId} ${total.stage}: ${total.windows} windows, ` +
        `avg=${formatDuration(total.averageDurationMs)}, max=${formatDuration(total.maxDurationMs)}, ` +
        `cards=${total.cards}`,
    );
  }
}

if (failures.length > 0) {
  console.log("Document window failures:");
  for (const failure of failures) {
    console.log(
      `  ${failure.providerKind} ${failure.modelId} ${failure.stage} window ${failure.window} ` +
        `pages ${failure.pageStart}-${failure.pageEnd}: ${failure.code} after ` +
        `${formatDuration(failure.durationMs)}`,
    );
  }
}
