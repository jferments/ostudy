#!/usr/bin/env python3
"""Compare two content-free oStudy generation reports for performance acceptance.

Performance and quality remain separate. The 50% wall-clock/token reductions are
engineering targets, while acceptance readiness requires comparable complete
measurements and no regression in the supplied quality rubric.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "generation_usage", ROOT / "scripts/generation-usage.py")
USAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(USAGE)

RUN_CLASSES = ("cold", "warm", "resume", "unchanged")
QUALITY_FIELDS = (
    "sourceFidelity",
    "importantTopicCoverage",
    "qualificationsAndUnits",
    "answerKeyCorrectness",
    "questionTypeDiversity",
    "visualTableInterpretation",
)


def valid_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0


def load_report(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    if isinstance(value.get("totals"), dict):
        return value
    return USAGE.summarize(value)


def reduction_percent(baseline: Any, current: Any) -> float | None:
    if not valid_number(baseline) or not valid_number(current) or baseline == 0:
        return None
    return 100.0 * (baseline - current) / baseline


def reported_tokens(report: dict[str, Any]) -> int | float | None:
    totals = report.get("totals")
    if not isinstance(totals, dict) or report.get("usageComplete") is not True:
        return None
    input_tokens = totals.get("inputTokens")
    output_tokens = totals.get("outputTokens")
    if not valid_number(input_tokens) or not valid_number(output_tokens):
        return None
    return input_tokens + output_tokens


def successful_operation(report: dict[str, Any]) -> bool:
    operation = report.get("operation")
    if not isinstance(operation, dict):
        return False
    return operation.get("state") == "succeeded"


def load_quality(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {
            "complete": False,
            "equivalentOrBetter": False,
            "applicableFields": [],
            "fields": {},
            "regressions": [],
            "note": "No quality evidence file was supplied.",
        }
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("quality evidence must be a JSON object")
    applicable = value.get("applicable", list(QUALITY_FIELDS))
    if not isinstance(applicable, list) or not applicable:
        raise ValueError("quality applicable must be a non-empty array")
    if any(field not in QUALITY_FIELDS for field in applicable):
        raise ValueError("quality applicable contains an unknown rubric field")
    baseline = value.get("baseline")
    current = value.get("current")
    if not isinstance(baseline, dict) or not isinstance(current, dict):
        raise ValueError("quality evidence requires baseline and current objects")

    fields: dict[str, Any] = {}
    regressions: list[str] = []
    complete = True
    for field in applicable:
        before = baseline.get(field)
        after = current.get(field)
        if not valid_number(before) or not valid_number(after):
            complete = False
            fields[field] = {"baseline": before, "current": after, "delta": None}
            continue
        if not 0 <= before <= 1 or not 0 <= after <= 1:
            raise ValueError(f"quality score {field} must be between 0 and 1")
        delta = after - before
        fields[field] = {"baseline": before, "current": after, "delta": delta}
        if delta < 0:
            regressions.append(field)
    return {
        "complete": complete,
        "equivalentOrBetter": complete and not regressions,
        "applicableFields": applicable,
        "fields": fields,
        "regressions": regressions,
        "note": (
            "Scores are benchmark/human evaluation evidence; card count is not a quality metric."
        ),
    }


def compare(
    baseline: dict[str, Any],
    current: dict[str, Any],
    run_class: str,
    quality: dict[str, Any],
) -> dict[str, Any]:
    if run_class not in RUN_CLASSES:
        raise ValueError(f"run class must be one of {', '.join(RUN_CLASSES)}")

    baseline_wall = baseline.get("wallClockMs")
    current_wall = current.get("wallClockMs")
    baseline_tokens = reported_tokens(baseline)
    current_tokens = reported_tokens(current)
    baseline_totals = baseline.get("totals", {})
    current_totals = current.get("totals", {})

    comparable = (
        valid_number(baseline_wall)
        and valid_number(current_wall)
        and baseline.get("usageComplete") is True
        and current.get("usageComplete") is True
        and successful_operation(baseline)
        and successful_operation(current)
    )
    wall_reduction = reduction_percent(baseline_wall, current_wall)
    token_reduction = reduction_percent(baseline_tokens, current_tokens)
    request_reduction = reduction_percent(
        baseline_totals.get("requests"), current_totals.get("requests"))
    retry_reduction = reduction_percent(
        baseline_totals.get("repeatedLogicalRequests"),
        current_totals.get("repeatedLogicalRequests"))

    performance = {
        "comparable": comparable,
        "baseline": {
            "wallClockMs": baseline_wall,
            "reportedTokens": baseline_tokens,
            "requests": baseline_totals.get("requests"),
            "retries": baseline_totals.get("repeatedLogicalRequests"),
            "cacheReadPercentOfCoveredInput": baseline_totals.get(
                "cacheReadPercentOfCoveredInput"),
        },
        "current": {
            "wallClockMs": current_wall,
            "reportedTokens": current_tokens,
            "requests": current_totals.get("requests"),
            "retries": current_totals.get("repeatedLogicalRequests"),
            "cacheReadPercentOfCoveredInput": current_totals.get(
                "cacheReadPercentOfCoveredInput"),
        },
        "change": {
            "wallClockReductionPercent": wall_reduction,
            "reportedTokenReductionPercent": token_reduction,
            "requestReductionPercent": request_reduction,
            "retryReductionPercent": retry_reduction,
            "cacheReadPercentagePointChange": (
                current_totals.get("cacheReadPercentOfCoveredInput")
                - baseline_totals.get("cacheReadPercentOfCoveredInput")
                if valid_number(current_totals.get("cacheReadPercentOfCoveredInput"))
                and valid_number(baseline_totals.get("cacheReadPercentOfCoveredInput"))
                else None
            ),
        },
        "engineeringTargets": {
            "wallClockReductionAtLeast50Percent": (
                wall_reduction is not None and wall_reduction >= 50
            ),
            "reportedTokenReductionAtLeast50Percent": (
                token_reduction is not None and token_reduction >= 50
            ),
        },
    }
    acceptance_ready = (
        comparable
        and quality.get("complete") is True
        and quality.get("equivalentOrBetter") is True
    )
    return {
        "schemaVersion": 1,
        "runClass": run_class,
        "performance": performance,
        "quality": quality,
        "acceptanceReady": acceptance_ready,
        "engineeringTargetsMet": (
            performance["engineeringTargets"]["wallClockReductionAtLeast50Percent"]
            and performance["engineeringTargets"]["reportedTokenReductionAtLeast50Percent"]
        ),
        "notes": [
            "Compare cold with cold, warm with warm, resume with resume, and unchanged with unchanged.",
            "The 50% reductions are engineering targets, not permission to omit requested content.",
            "Acceptance readiness requires complete usage, successful operations, wall-clock bounds, and no applicable quality regression.",
            "Card count is reported elsewhere and is not a quality score.",
        ],
    }


def render(result: dict[str, Any]) -> str:
    perf = result["performance"]
    change = perf["change"]
    quality = result["quality"]

    def percent(value: Any) -> str:
        return "unknown" if value is None else f"{value:.2f}%"

    return "\n".join([
        f"Generation acceptance comparison ({result['runClass']})",
        f"Wall-clock reduction: {percent(change['wallClockReductionPercent'])}",
        f"Reported-token reduction: {percent(change['reportedTokenReductionPercent'])}",
        f"Request reduction: {percent(change['requestReductionPercent'])}",
        f"Retry reduction: {percent(change['retryReductionPercent'])}",
        f"Quality evidence complete: {'yes' if quality['complete'] else 'no'}",
        f"Quality equivalent or better: {'yes' if quality['equivalentOrBetter'] else 'NO'}",
        f"Quality regressions: {', '.join(quality['regressions']) or 'none'}",
        f"Acceptance ready: {'yes' if result['acceptanceReady'] else 'NO'}",
        f"50% time/token engineering targets both met: {'yes' if result['engineeringTargetsMet'] else 'no'}",
    ])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True,
                        help="Baseline raw diagnostic JSON or generation-usage --json report")
    parser.add_argument("--current", type=Path, required=True,
                        help="Current raw diagnostic JSON or generation-usage --json report")
    parser.add_argument("--run-class", choices=RUN_CLASSES, required=True)
    parser.add_argument("--quality", type=Path,
                        help="Quality rubric JSON with baseline/current scores and applicable fields")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--require-ready", action="store_true",
                        help="Exit 1 unless comparable measurements show no quality regression")
    args = parser.parse_args()
    try:
        result = compare(
            load_report(args.baseline),
            load_report(args.current),
            args.run_class,
            load_quality(args.quality),
        )
        print(json.dumps(result, indent=2, allow_nan=False) if args.json else render(result))
        if args.require_ready and not result["acceptanceReady"]:
            return 1
        return 0
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        print(f"Could not evaluate generation acceptance: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
