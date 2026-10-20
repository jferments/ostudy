#!/usr/bin/env python3
"""Study-guide evaluation: generate guides for a corpus, then score them.

Standard library only. `generate` runs the native `guide-eval` runner once per
corpus document; `score` computes deterministic metrics and a rubric score from
a model judge; `compare` puts two scored runs side by side. The rubric and the
workflow are in docs/development/study-guide-evaluation.md.

Provider keys come from the environment only: OSTUDY_EVAL_API_KEY for the
generating provider, OSTUDY_JUDGE_API_KEY for the judge.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNNER = ROOT / "src-tauri/target/debug/guide-eval"
RUBRIC_VERSION = 1
# Weights of the 1-5 rubric criteria in the 0-100 guide score. Cost is reported
# beside the score, never folded into it.
WEIGHTS = {
    "accuracy": 0.30,
    "coverage": 0.25,
    "sourceSupport": 0.15,
    "organization": 0.15,
    "clarity": 0.15,
}
MAX_SECTION_SOURCE_CHARS = 14_000
MAX_GUIDE_SOURCE_CHARS = 48_000
MAX_GUIDE_TEXT_CHARS = 48_000

SECTION_JUDGE = """You are grading one section of an AI-written study guide against the source pages it was written from. A student will learn from this guide, so errors and invented facts matter most.

Read the SOURCE EXCERPTS, then the GUIDE SECTION. Check every substantive statement in the section (definitions, mechanisms, numbers, cause and effect, qualifications) against the excerpts.

Score each criterion from 1 to 5 using these anchors:
- accuracy: 5 no factual errors; 4 one minor imprecision; 3 one major error or several minor ones; 2 several major errors; 1 mostly wrong or misleading.
- sourceSupport: share of substantive statements traceable to the excerpts. 5 all; 4 nearly all (90% or more); 3 most (70-90%); 2 about half; 1 little. Correct background knowledge that the excerpts do not contain still counts against support.
- clarity: 5 explains mechanisms and relationships in readable prose a student can learn from, with lists only where they help; 4 clear with small lapses; 3 understandable but terse, list-like or repetitive; 2 hard to follow; 1 confusing.

List each unsupported or wrong statement. severity is "major" when it is false or would mislead a student, "minor" when it is correct but not in the excerpts, or imprecise.

Reply with JSON only:
{"claimsChecked": <int>, "unsupportedClaims": [{"claim": "...", "severity": "major"|"minor", "why": "..."}], "accuracy": <1-5>, "sourceSupport": <1-5>, "clarity": <1-5>, "notes": "<one or two sentences>"}"""

GUIDE_JUDGE = """You are grading the coverage and organization of an AI-written study guide for one source document. A student should be able to learn what matters in the source from the guide.

Read the SOURCE (ordered excerpts with their headings), then the GUIDE (overview and sections in reading order).

First list the concepts in the source a student must learn ("core") and the ones that help but are secondary ("supporting"), then decide which the guide does not teach. Mentioning a term without explaining it does not count as teaching it. End-of-chapter questions, references and front matter in the source are not concepts.

Score each criterion from 1 to 5 using these anchors:
- coverage: 5 every core concept is taught; 4 one core concept or a few supporting ones are missing; 3 two or three core concepts are missing; 2 many are missing; 1 most of the source is missing.
- organization: 5 logical order for learning the topic, sensible section granularity, no duplicated content, an overview that frames the material; 4 good with small lapses; 3 usable but fragmented, duplicated or oddly ordered; 2 hard to navigate; 1 incoherent.

Reply with JSON only:
{"coreConcepts": ["..."], "missingConcepts": [{"concept": "...", "importance": "core"|"supporting"}], "duplicatedContent": ["..."], "coverage": <1-5>, "organization": <1-5>, "notes": "<one or two sentences>"}"""


# ---------------------------------------------------------------- generate --

def git_head() -> str:
    try:
        return subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], check=True,
                              capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_dirty() -> bool:
    try:
        output = subprocess.run(["git", "-C", str(ROOT), "status", "--porcelain"], check=True,
                                capture_output=True, text=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return True
    return bool(output.strip())


def load_corpus(path: Path) -> list[dict[str, Any]]:
    entries = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(entries, dict):
        entries = entries.get("documents", [])
    if not isinstance(entries, list) or not entries:
        raise SystemExit(f"{path}: expected a non-empty list of documents")
    for entry in entries:
        if not isinstance(entry, dict) or not entry.get("id") or not entry.get("file"):
            raise SystemExit(f"{path}: every document needs an id and a file")
        entry["path"] = (path.parent / entry["file"]).resolve()
    return entries


def command_generate(args: argparse.Namespace) -> int:
    corpus = load_corpus(args.corpus)
    if args.only:
        wanted = set(args.only.split(","))
        corpus = [entry for entry in corpus if entry["id"] in wanted]
    # Private documents may live outside the corpus folder until fetched, so
    # only the documents this run uses must be present.
    for entry in corpus:
        if not entry["path"].is_file():
            raise SystemExit(f"{entry['path']}: missing corpus file")
    run = args.out
    run.mkdir(parents=True, exist_ok=True)
    meta = {
        "label": run.name,
        "createdAt": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "commit": git_head(),
        "dirtyTree": git_dirty(),
        "provider": args.provider,
        "baseUrl": args.base_url,
        "model": args.model,
        "reasoning": args.reasoning,
        "corpus": str(args.corpus),
        "documents": [entry["id"] for entry in corpus],
    }
    (run / "run.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    failures = 0
    for entry in corpus:
        out = run / entry["id"]
        if (out / "outcome.json").exists():
            print(f"skip {entry['id']}: already generated")
            continue
        command = [str(args.runner), "--source", str(entry["path"]), "--out", str(out),
                   "--provider", args.provider, "--base-url", args.base_url,
                   "--model", args.model, "--reasoning", args.reasoning]
        if args.allow_insecure_http:
            command.append("--allow-insecure-http")
        print(f"generate {entry['id']} ...", flush=True)
        env = dict(os.environ)
        env.setdefault("OSTUDY_ROOT_DIR", str(ROOT))
        result = subprocess.run(command, env=env)
        if result.returncode != 0:
            failures += 1
        usage = usage_report(out / "library" / "library.sqlite3")
        if usage is not None:
            (out / "usage.json").write_text(json.dumps(usage, indent=2) + "\n", encoding="utf-8")
    print(f"{len(corpus) - failures}/{len(corpus)} guides generated in {run}")
    return 1 if failures else 0


def usage_report(database: Path) -> dict[str, Any] | None:
    if not database.is_file():
        return None
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts/generation-usage.py"), "--database", str(database),
         "--all", "--json"], capture_output=True, text=True)
    if result.returncode != 0:
        return {"error": result.stderr.strip()[-2000:]}
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        return {"error": "generation-usage.py did not print JSON"}
    report.pop("requests", None)
    return report


# ------------------------------------------------------------------- score --

def section_text(content: dict[str, Any]) -> str:
    """Everything a learner reads in one section, in reading order."""
    parts: list[str] = []
    add = parts.append
    if content.get("overview"):
        add(content["overview"])
    for point in content.get("keyPoints", []):
        add(f"- {point}")
    for item in content.get("explanations", []):
        add(f"{item.get('heading', '')}: {item.get('body', '')}")
    for item in content.get("importantTerms", []):
        add(f"Term {item.get('term', '')}: {item.get('explanation', '')}")
    for index, step in enumerate(content.get("processSteps", []), 1):
        add(f"Step {index}. {step}")
    for item in content.get("comparisons", []):
        add(f"{item.get('subjectA', '')} vs {item.get('subjectB', '')}: {item.get('relationship', '')}")
    for item in content.get("commonMisconceptions", []):
        add(f"Misconception: {item}")
    for item in content.get("callouts", []):
        add(f"{item.get('kind', 'note')} - {item.get('title', '')}: {item.get('body', '')}")
    for table in content.get("tables", []):
        add(f"Table {table.get('title', '')}: " + " | ".join(table.get("columns", [])))
        for row in table.get("rows", []):
            add("  " + " | ".join(row))
    for item in content.get("workedExamples", []):
        steps = " ".join(item.get("steps", []))
        add(f"Example {item.get('title', '')}: {item.get('prompt', '')} {steps} "
            f"Answer: {item.get('answer', '')} {item.get('takeaway', '')}")
    for item in content.get("retrievalChecks", []):
        add(f"Check: {item.get('prompt', '')} Answer: {item.get('answer', '')}")
    for item in content.get("visualExplanations", []):
        add(f"Figure {item.get('title', '')}: {item.get('explanation', '')}")
    return "\n".join(part for part in parts if part.strip())


def overview_text(guide: dict[str, Any]) -> str:
    text = guide.get("overviewText") or ""
    if not text and isinstance(guide.get("overview"), dict):
        text = json.dumps(guide["overview"], ensure_ascii=False)
    return text


def words(text: str) -> int:
    return len(re.findall(r"\w+", text))


def locator_units(locator: dict[str, Any] | None) -> set[int]:
    if not isinstance(locator, dict):
        return set()
    return {int(value) for value in locator.get("unitNumbers", []) if isinstance(value, int)}


def chunks_for(locator: dict[str, Any], chunks: list[dict[str, Any]]) -> list[dict[str, Any]]:
    units = locator_units(locator)
    return [chunk for chunk in chunks if units & locator_units(chunk.get("sourceLocator"))]


def clip(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[:limit] + "\n[... clipped ...]"


def sentences(text: str) -> list[str]:
    return [s.strip().lower() for s in re.split(r"(?<=[.!?])\s+", text) if words(s) >= 6]


def deterministic_metrics(outcome: dict[str, Any]) -> dict[str, Any]:
    guide = outcome.get("guide") or {}
    chunks = (outcome.get("source") or {}).get("chunks", [])
    sections = guide.get("sections", [])
    texts = [section_text(section.get("content", {})) for section in sections]
    source_units = set().union(*(locator_units(c.get("sourceLocator")) for c in chunks)) if chunks else set()
    covered_units = set().union(*(locator_units(s.get("sourceLocator")) for s in sections)) if sections else set()
    source_tokens = sum(int(c.get("tokenEstimate") or 0) for c in chunks)
    all_sentences = [s for text in texts for s in sentences(text)]
    repeated = len(all_sentences) - len(set(all_sentences))
    categories: dict[str, int] = {}
    for section in sections:
        for key, value in (section.get("content") or {}).items():
            if isinstance(value, list) and value:
                categories[key] = categories.get(key, 0) + 1
    return {
        "published": bool(sections),
        "sectionCount": len(sections),
        "guideWords": words(overview_text(guide)) + sum(words(text) for text in texts),
        "overviewWords": words(overview_text(guide)),
        "sectionWords": [words(text) for text in texts],
        "sourceUnits": len(source_units),
        "sourceTokens": source_tokens,
        "sourceUnitsCoveredBySections": len(source_units & covered_units),
        "sourceUnitCoverage": round(len(source_units & covered_units) / len(source_units), 3)
        if source_units else None,
        "repeatedSentences": repeated,
        "sectionsUsingCategory": categories,
    }


def section_prompt(title: str, section: dict[str, Any], chunks: list[dict[str, Any]]) -> str:
    excerpts = "\n\n".join(
        f"[{', '.join(str(u) for u in sorted(locator_units(c.get('sourceLocator'))))}] "
        f"{c.get('headingContext') or ''}\n{c.get('text', '')}" for c in chunks_for(section.get("sourceLocator"), chunks))
    return (f"{SECTION_JUDGE}\n\nDOCUMENT: {title}\n\nSOURCE EXCERPTS:\n"
            f"{clip(excerpts, MAX_SECTION_SOURCE_CHARS)}\n\nGUIDE SECTION: {section.get('title', '')}\n"
            f"{section_text(section.get('content', {}))}\n")


def first_sentence(text: str) -> str:
    match = re.search(r"[.!?](\s|$)", text)
    return text[:match.end()].strip() if match else text.strip()


def section_outline(content: dict[str, Any]) -> str:
    """A long guide's section as its overview, each explanation's heading and
    opening sentence, and the names of its other study aids."""
    parts = [content.get("overview", "")]
    parts += [f"- {item.get('heading', '')}: {first_sentence(item.get('body', ''))}"
              for item in content.get("explanations", [])]
    if content.get("tables"):
        parts.append("Tables: " + "; ".join(t.get("title", "") for t in content["tables"]))
    if content.get("processSteps"):
        parts.append(f"Process steps: {len(content['processSteps'])}")
    if content.get("importantTerms"):
        parts.append("Terms: " + ", ".join(t.get("term", "") for t in content["importantTerms"]))
    return "\n".join(part for part in parts if part)


def guide_prompt(title: str, guide: dict[str, Any], chunks: list[dict[str, Any]]) -> str:
    source = "\n\n".join(f"## {c.get('headingContext') or '(no heading)'}\n{c.get('text', '')}" for c in chunks)
    sections = guide.get("sections", [])
    body = "\n\n".join(f"## {s.get('title', '')}\n{section_text(s.get('content', {}))}" for s in sections)
    heading = "GUIDE SECTIONS:"
    if len(body) > MAX_GUIDE_TEXT_CHARS:
        # Clipping would hide the later sections, so a long guide is judged
        # from an outline of every section instead; each section's own
        # judge reads its full text.
        body = "\n\n".join(f"## {s.get('title', '')}\n{section_outline(s.get('content', {}))}"
                             for s in sections)
        heading = ("GUIDE SECTIONS (outline of a long guide: each explanation is shown by its heading "
                   "and opening sentence and teaches more than that sentence):")
    return (f"{GUIDE_JUDGE}\n\nDOCUMENT: {title}\n\nSOURCE:\n{clip(source, MAX_GUIDE_SOURCE_CHARS)}"
            f"\n\nGUIDE OVERVIEW:\n{overview_text(guide)}\n\n{heading}\n"
            f"{clip(body, MAX_GUIDE_TEXT_CHARS)}\n")


def call_judge(prompt: str, args: argparse.Namespace) -> dict[str, Any]:
    key = os.environ.get("OSTUDY_JUDGE_API_KEY", "").strip()
    body = {
        "model": args.judge_model,
        "temperature": 0,
        "max_tokens": 8192,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": prompt}],
    }
    request = urllib.request.Request(
        args.judge_base_url.rstrip("/") + "/chat/completions",
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json",
                 **({"Authorization": f"Bearer {key}"} if key else {})},
        method="POST")
    try:
        with urllib.request.urlopen(request, timeout=args.judge_timeout) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"judge HTTP {error.code}: {error.read()[:500]!r}") from error
    content = payload["choices"][0]["message"]["content"]
    verdict = parse_verdict(content)
    verdict["_usage"] = payload.get("usage")
    return verdict


def parse_verdict(content: str) -> dict[str, Any]:
    match = re.search(r"\{.*\}", content, re.S)
    if not match:
        raise RuntimeError("judge reply contained no JSON object")
    return json.loads(match.group(0))


def judged(path: Path, prompt: str, args: argparse.Namespace) -> dict[str, Any] | None:
    """Return the verdict for one prompt, asking the judge only when needed.

    Prompts and verdicts are kept beside the run, so a re-score reuses them and
    a manual judge (a person or an assistant session) can answer
    `<name>.prompt.txt` by writing `<name>.response.json`.
    """
    prompt_path = path.with_suffix(".prompt.txt")
    response_path = path.with_suffix(".response.json")
    prompt_path.write_text(prompt, encoding="utf-8")
    if response_path.exists():
        return parse_verdict(response_path.read_text(encoding="utf-8"))
    if args.judge == "manual":
        return None
    try:
        verdict = call_judge(prompt, args)
    except json.JSONDecodeError:
        # A cut-off verdict is asked once more rather than ending the score.
        verdict = call_judge(prompt, args)
    response_path.write_text(json.dumps(verdict, indent=2) + "\n", encoding="utf-8")
    return verdict


def scale(value: Any) -> float | None:
    if isinstance(value, (int, float)) and 1 <= value <= 5:
        return (float(value) - 1.0) / 4.0 * 100.0
    return None


def score_document(directory: Path, title: str, args: argparse.Namespace) -> dict[str, Any]:
    outcome = json.loads((directory / "outcome.json").read_text(encoding="utf-8"))
    metrics = deterministic_metrics(outcome)
    usage = None
    if (directory / "usage.json").exists():
        usage = json.loads((directory / "usage.json").read_text(encoding="utf-8")).get("totals")
    result: dict[str, Any] = {"document": directory.name, "metrics": metrics, "usage": usage,
                              "error": outcome.get("error"), "timingsMs": outcome.get("timingsMs")}
    guide = outcome.get("guide")
    if not guide:
        result["score"] = 0.0
        result["pending"] = []
        return result
    write_markdown(directory / "guide.md", title, guide)
    chunks = (outcome.get("source") or {}).get("chunks", [])
    judge_dir = directory / "judge"
    judge_dir.mkdir(exist_ok=True)
    pending: list[str] = []
    section_verdicts = []
    for index, section in enumerate(guide.get("sections", [])):
        verdict = judged(judge_dir / f"section-{index:02d}", section_prompt(title, section, chunks), args)
        if verdict is None:
            pending.append(f"section-{index:02d}")
            continue
        section_verdicts.append((metrics["sectionWords"][index], verdict))
    guide_verdict = judged(judge_dir / "guide", guide_prompt(title, guide, chunks), args)
    if guide_verdict is None:
        pending.append("guide")
    result["pending"] = pending
    criteria: dict[str, float | None] = {}
    for name in ("accuracy", "sourceSupport", "clarity"):
        weighted = [(max(weight, 1), scale(v.get(name))) for weight, v in section_verdicts]
        weighted = [(w, s) for w, s in weighted if s is not None]
        criteria[name] = (sum(w * s for w, s in weighted) / sum(w for w, _ in weighted)) if weighted else None
    for name in ("coverage", "organization"):
        criteria[name] = scale(guide_verdict.get(name)) if guide_verdict else None
    result["criteria"] = {k: (round(v, 1) if v is not None else None) for k, v in criteria.items()}
    claims = [c for _, v in section_verdicts for c in v.get("unsupportedClaims", [])]
    result["majorUnsupportedClaims"] = sum(c.get("severity") == "major" for c in claims)
    result["minorUnsupportedClaims"] = sum(c.get("severity") == "minor" for c in claims)
    if guide_verdict:
        missing = guide_verdict.get("missingConcepts", [])
        result["missingCoreConcepts"] = [m.get("concept") for m in missing if m.get("importance") == "core"]
    complete = not pending and all(v is not None for v in criteria.values())
    result["score"] = round(sum(WEIGHTS[k] * criteria[k] for k in WEIGHTS), 1) if complete else None
    return result


def write_markdown(path: Path, title: str, guide: dict[str, Any]) -> None:
    lines = [f"# {title}", "", overview_text(guide), ""]
    for section in guide.get("sections", []):
        units = ", ".join(str(u) for u in sorted(locator_units(section.get("sourceLocator"))))
        lines += [f"## {section.get('title', '')}", f"_Source units {units}_", "",
                  section_text(section.get("content", {})), ""]
    path.write_text("\n".join(lines), encoding="utf-8")


def command_score(args: argparse.Namespace) -> int:
    run = args.run
    meta = json.loads((run / "run.json").read_text(encoding="utf-8"))
    titles = {}
    if args.corpus:
        titles = {entry["id"]: entry.get("title") or entry["id"] for entry in load_corpus(args.corpus)}
    documents = []
    for doc_id in meta["documents"]:
        directory = run / doc_id
        if not (directory / "outcome.json").exists():
            documents.append({"document": doc_id, "score": 0.0, "error": {"stage": "missing"}})
            continue
        documents.append(score_document(directory, titles.get(doc_id, doc_id), args))
    scores = [d.get("score") for d in documents]
    complete = all(s is not None for s in scores)
    summary = {
        "rubricVersion": RUBRIC_VERSION,
        "run": meta,
        "judge": {"mode": args.judge, "model": args.judge_model, "baseUrl": args.judge_base_url},
        "meanScore": round(sum(scores) / len(scores), 1) if complete and scores else None,
        "documents": documents,
    }
    (run / "score.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    (run / "score.md").write_text(render_summary(summary), encoding="utf-8")
    print(render_summary(summary))
    pending = [f"{d['document']}/{p}" for d in documents for p in d.get("pending", [])]
    if pending:
        print(f"{len(pending)} judge prompts wait for a response.json: {', '.join(pending)}")
        return 3
    return 0


def fmt(value: Any) -> str:
    return "-" if value is None else (f"{value:.1f}" if isinstance(value, float) else str(value))


def render_summary(summary: dict[str, Any]) -> str:
    meta = summary["run"]
    lines = [f"# Study-guide evaluation: {meta['label']}", "",
             f"Commit `{meta['commit'][:10]}`{' (dirty tree)' if meta.get('dirtyTree') else ''}, "
             f"{meta['provider']} `{meta['model']}` (reasoning {meta['reasoning']}), "
             f"judge {summary['judge']['mode']} `{summary['judge']['model'] or '-'}`, "
             f"rubric v{summary['rubricVersion']}.", "",
             f"**Mean score: {fmt(summary['meanScore'])} / 100**", "",
             "| Document | Score | Accuracy | Coverage | Support | Organization | Clarity "
             "| Major errors | Sections | Words | Tokens | Cost |",
             "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for d in summary["documents"]:
        c = d.get("criteria", {})
        m = d.get("metrics", {})
        u = d.get("usage") or {}
        lines.append(" | ".join([
            f"| {d['document']}", fmt(d.get("score")), fmt(c.get("accuracy")), fmt(c.get("coverage")),
            fmt(c.get("sourceSupport")), fmt(c.get("organization")), fmt(c.get("clarity")),
            fmt(d.get("majorUnsupportedClaims")), fmt(m.get("sectionCount")), fmt(m.get("guideWords")),
            fmt(u.get("totalTokens")), fmt(u.get("cost")) + " |"]))
    for d in summary["documents"]:
        if d.get("error"):
            lines += ["", f"- {d['document']} failed at {d['error'].get('stage')}: "
                          f"{d['error'].get('message', '')} ({d['error'].get('code', '')})"]
        if d.get("missingCoreConcepts"):
            lines += ["", f"- {d['document']} misses: " + "; ".join(d["missingCoreConcepts"])]
    return "\n".join(lines) + "\n"


# ----------------------------------------------------------------- compare --

def command_compare(args: argparse.Namespace) -> int:
    before = json.loads((args.before / "score.json").read_text(encoding="utf-8"))
    after = json.loads((args.after / "score.json").read_text(encoding="utf-8"))
    if before["judge"]["model"] != after["judge"]["model"]:
        print("warning: the runs used different judges; scores are not comparable", file=sys.stderr)
    rows = {d["document"]: [d, None] for d in before["documents"]}
    for d in after["documents"]:
        rows.setdefault(d["document"], [None, None])[1] = d
    lines = [f"| Document | {before['run']['label']} | {after['run']['label']} | Change "
             "| Major errors | Tokens |", "| --- | --- | --- | --- | --- | --- |"]
    for doc_id, (b, a) in rows.items():
        bs, as_ = (b or {}).get("score"), (a or {}).get("score")
        change = f"{as_ - bs:+.1f}" if bs is not None and as_ is not None else "-"
        tokens = [((x or {}).get("usage") or {}).get("totalTokens") for x in (b, a)]
        errors = [(x or {}).get("majorUnsupportedClaims") for x in (b, a)]
        lines.append(f"| {doc_id} | {fmt(bs)} | {fmt(as_)} | {change} | "
                     f"{fmt(errors[0])} → {fmt(errors[1])} | {fmt(tokens[0])} → {fmt(tokens[1])} |")
    lines.append(f"| **Mean** | {fmt(before['meanScore'])} | {fmt(after['meanScore'])} | | | |")
    print("\n".join(lines))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)

    generate = commands.add_parser("generate", help="Create a guide for every corpus document")
    generate.add_argument("--corpus", type=Path, required=True, help="corpus.json")
    generate.add_argument("--out", type=Path, required=True, help="New run directory")
    generate.add_argument("--provider", required=True, choices=["llama_cpp", "deepseek", "openrouter"])
    generate.add_argument("--base-url", required=True)
    generate.add_argument("--model", required=True)
    generate.add_argument("--reasoning", default="automatic")
    generate.add_argument("--only", help="Comma-separated corpus ids")
    generate.add_argument("--runner", type=Path, default=DEFAULT_RUNNER)
    generate.add_argument("--allow-insecure-http", action="store_true")
    generate.set_defaults(handler=command_generate)

    score = commands.add_parser("score", help="Score a generated run")
    score.add_argument("--run", type=Path, required=True)
    score.add_argument("--corpus", type=Path, help="corpus.json, for document titles")
    score.add_argument("--judge", choices=["api", "manual"], default="api")
    score.add_argument("--judge-base-url", default="https://openrouter.ai/api/v1")
    score.add_argument("--judge-model")
    score.add_argument("--judge-timeout", type=int, default=300)
    score.set_defaults(handler=command_score)

    compare = commands.add_parser("compare", help="Compare two scored runs")
    compare.add_argument("before", type=Path)
    compare.add_argument("after", type=Path)
    compare.set_defaults(handler=command_compare)

    args = parser.parse_args()
    if args.command == "score" and args.judge == "api" and not args.judge_model:
        parser.error("--judge-model is required with --judge api")
    return args.handler(args)


if __name__ == "__main__":
    sys.exit(main())
