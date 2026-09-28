#!/usr/bin/env python3
"""Validate and write oStudy generated-flashcard v2 import files.

This script is intentionally dependency-free so a skills-only ChatGPT plugin can
perform deterministic preflight without an MCP server or remote service. oStudy's
native Rust importer remains authoritative for the complete restricted-Markdown
and persistence contract.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
from urllib.parse import urlparse
from typing import Any

MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_CARDS = 10_000
MAX_DECK_NAME_CHARS = 200
MAX_DECK_DESCRIPTION_CHARS = 2_000
MAX_TAGS = 32
MAX_TAG_CHARS = 64
MAX_TEXT_CHARS = 200_000
MAX_QUESTION_BYTES = 1_000_000
MAX_ITEMS = 32
LOCAL_ID = re.compile(r"^[A-Za-z0-9_-]{1,64}$")
CLOZE = re.compile(r"\{\{c([1-9][0-9]{0,4})::([^{}]+)\}\}")
LINK = re.compile(r"(?<!!)\[([^\]\n]+)\]\(([^)\n]+)\)")
HTML = re.compile(r"<\s*/?\s*[A-Za-z][^>\n]*>")
TABLE_SEPARATOR = re.compile(
    r"^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$"
)

TOP_KEYS = {"format", "formatVersion", "deck", "defaultTags", "cards"}
DECK_KEYS = {"mode", "name", "description"}
CARD_KEYS = {
    "basic": {"type", "front", "back", "reverseEnabled", "tags"},
    "cloze": {"type", "text", "mode", "tags"},
    "interactive": {"type", "question", "tags"},
}
QUESTION_KEYS = {
    "single_choice": {"kind", "prompt", "options", "correctOptionId", "explanation"},
    "select_all": {"kind", "prompt", "options", "correctOptionIds", "explanation"},
    "true_false": {"kind", "statement", "correct", "explanation", "correctedStatement"},
    "matching": {"kind", "prompt", "pairs", "explanation"},
    "grouping": {"kind", "prompt", "categories", "items", "explanation"},
    "ordering": {"kind", "prompt", "items", "explanation"},
}


class ValidationError(ValueError):
    def __init__(self, path: str, message: str) -> None:
        super().__init__(f"{path}: {message}")
        self.path = path
        self.message = message


def fail(path: str, message: str) -> None:
    raise ValidationError(path, message)


def require_object(value: Any, path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(path, "expected an object")
    return value


def require_list(value: Any, path: str) -> list[Any]:
    if not isinstance(value, list):
        fail(path, "expected an array")
    return value


def exact_keys(value: dict[str, Any], allowed: set[str], required: set[str], path: str) -> None:
    missing = sorted(required - value.keys())
    if missing:
        fail(path, f"missing required property {missing[0]!r}")
    unknown = sorted(value.keys() - allowed)
    if unknown:
        fail(path, f"unknown property {unknown[0]!r}")


def text(value: Any, path: str, *, maximum: int = MAX_TEXT_CHARS, allow_empty: bool = False) -> str:
    if not isinstance(value, str):
        fail(path, "expected a string")
    normalized = value.strip()
    if not allow_empty and not normalized:
        fail(path, "must contain visible text")
    if len(normalized) > maximum:
        fail(path, f"must not exceed {maximum} characters")
    return normalized


def optional_rich(value: Any, path: str) -> str | None:
    if value is None:
        return None
    normalized = text(value, path)
    validate_markdown(normalized, path)
    return normalized


def local_id(value: Any, path: str) -> str:
    if not isinstance(value, str) or not LOCAL_ID.fullmatch(value):
        fail(path, "must be 1–64 ASCII letters, digits, '_' or '-'")
    return value


def tags(value: Any, path: str) -> list[str]:
    if value is None:
        fail(path, "expected an array, not null")
    values = require_list(value, path)
    if len(values) > MAX_TAGS:
        fail(path, f"may contain at most {MAX_TAGS} tags")
    result: list[str] = []
    seen: set[str] = set()
    for index, raw in enumerate(values):
        item = text(raw, f"{path}[{index}]", maximum=MAX_TAG_CHARS)
        if item.startswith("#"):
            fail(f"{path}[{index}]", "omit the leading '#' from tag names")
        key = item.casefold()
        if key in seen:
            fail(f"{path}[{index}]", "duplicate tag after normalization")
        seen.add(key)
        result.append(item)
    return result


def mask_code(source: str, path: str) -> str:
    """Blank code payload while preserving newlines for Markdown syntax checks."""
    output: list[str] = []
    index = 0
    in_fence = False
    while index < len(source):
        if source.startswith("```", index):
            in_fence = not in_fence
            output.append("   ")
            index += 3
            continue
        char = source[index]
        if in_fence:
            output.append("\n" if char == "\n" else " ")
            index += 1
            continue
        if char == "`":
            close = source.find("`", index + 1)
            if close == -1:
                fail(path, "unclosed inline-code span")
            output.append(" " * (close - index + 1))
            index = close + 1
            continue
        output.append(char)
        index += 1
    if in_fence:
        fail(path, "unclosed fenced-code block")
    return "".join(output)


def validate_markdown(source: str, path: str) -> None:
    visible = mask_code(source, path)
    lower = visible.lower()
    if "<!--" in visible or "<!doctype" in lower or "<?" in visible or HTML.search(visible):
        fail(path, "raw HTML is not supported")
    if re.search(r"!\[[^\]]*\]\([^)]*\)", visible):
        fail(path, "Markdown images are not supported")
    lines = visible.splitlines()
    for index in range(1, len(lines)):
        if "|" in lines[index - 1] and TABLE_SEPARATOR.fullmatch(lines[index]):
            fail(path, "Markdown tables are not supported")
    matched_spans: list[tuple[int, int]] = []
    for match in LINK.finditer(visible):
        matched_spans.append(match.span())
        target = match.group(2).strip()
        parsed = urlparse(target)
        if parsed.scheme.lower() not in {"http", "https", "mailto", "tel"}:
            fail(path, "links must use absolute http, https, mailto, or tel URLs")
    # A dangling Markdown-link opener is rejected rather than guessed at. Ignore
    # valid spans so literal code and ordinary parentheses remain unaffected.
    scrubbed = list(visible)
    for start, end in matched_spans:
        scrubbed[start:end] = " " * (end - start)
    remainder = "".join(scrubbed)
    if re.search(r"\[[^\]\n]+\]\(", remainder):
        fail(path, "malformed Markdown link")


def rich(value: Any, path: str) -> str:
    normalized = text(value, path)
    validate_markdown(normalized, path)
    return normalized


def validate_cloze(value: Any, path: str) -> str:
    source = text(value, path)
    matches = list(CLOZE.finditer(source))
    if not matches:
        fail(path, "must contain at least one {{cN::hidden text}} marker")
    for match in matches:
        group = int(match.group(1))
        if group < 1 or group > 10_000:
            fail(path, "Cloze group numbers must be between 1 and 10000")
        if not match.group(2).strip():
            fail(path, "Cloze hidden text must not be empty")
    remainder = CLOZE.sub("", source)
    if "{{" in remainder or "}}" in remainder:
        fail(path, "contains malformed, nested, or stray Cloze braces")
    return source


def distinct_ids(values: list[dict[str, Any]], path: str) -> None:
    seen: set[str] = set()
    for index, item in enumerate(values):
        key = item["id"]
        if key in seen:
            fail(f"{path}[{index}].id", "component IDs must be unique within this collection")
        seen.add(key)


def distinct_content(values: list[dict[str, Any]], key: str, path: str) -> None:
    seen: set[str] = set()
    for index, item in enumerate(values):
        normalized = item[key]
        if normalized in seen:
            fail(f"{path}[{index}].{key}", "items on the same side must not have identical content")
        seen.add(normalized)


def validate_option(value: Any, path: str) -> dict[str, Any]:
    item = require_object(value, path)
    exact_keys(item, {"id", "content", "rationale"}, {"id", "content"}, path)
    result: dict[str, Any] = {
        "id": local_id(item["id"], f"{path}.id"),
        "content": rich(item["content"], f"{path}.content"),
    }
    if "rationale" in item:
        rationale = optional_rich(item["rationale"], f"{path}.rationale")
        if rationale is not None:
            result["rationale"] = rationale
    return result


def validate_item(value: Any, path: str) -> dict[str, Any]:
    item = require_object(value, path)
    exact_keys(item, {"id", "content"}, {"id", "content"}, path)
    return {
        "id": local_id(item["id"], f"{path}.id"),
        "content": rich(item["content"], f"{path}.content"),
    }


def validate_pair(value: Any, path: str) -> dict[str, Any]:
    item = require_object(value, path)
    exact_keys(item, {"id", "left", "right", "rationale"}, {"id", "left", "right"}, path)
    result: dict[str, Any] = {
        "id": local_id(item["id"], f"{path}.id"),
        "left": rich(item["left"], f"{path}.left"),
        "right": rich(item["right"], f"{path}.right"),
    }
    if "rationale" in item:
        rationale = optional_rich(item["rationale"], f"{path}.rationale")
        if rationale is not None:
            result["rationale"] = rationale
    return result


def validate_grouped_item(value: Any, path: str) -> dict[str, Any]:
    item = require_object(value, path)
    exact_keys(
        item,
        {"id", "content", "categoryId", "rationale"},
        {"id", "content", "categoryId"},
        path,
    )
    result: dict[str, Any] = {
        "id": local_id(item["id"], f"{path}.id"),
        "content": rich(item["content"], f"{path}.content"),
        "categoryId": local_id(item["categoryId"], f"{path}.categoryId"),
    }
    if "rationale" in item:
        rationale = optional_rich(item["rationale"], f"{path}.rationale")
        if rationale is not None:
            result["rationale"] = rationale
    return result


def collection(
    raw: Any,
    path: str,
    *,
    minimum: int,
    maximum: int,
    validator,
) -> list[dict[str, Any]]:
    values = require_list(raw, path)
    if not minimum <= len(values) <= maximum:
        fail(path, f"must contain between {minimum} and {maximum} items")
    return [validator(item, f"{path}[{index}]") for index, item in enumerate(values)]


def validate_question(value: Any, path: str) -> dict[str, Any]:
    question = require_object(value, path)
    kind = question.get("kind")
    if not isinstance(kind, str) or kind not in QUESTION_KEYS:
        fail(f"{path}.kind", "unsupported generated question kind")
    exact_keys(question, QUESTION_KEYS[kind], required_question_keys(kind), path)
    result: dict[str, Any] = {"kind": kind}

    if kind in {"single_choice", "select_all"}:
        result["prompt"] = rich(question["prompt"], f"{path}.prompt")
        options = collection(
            question["options"], f"{path}.options", minimum=2, maximum=MAX_ITEMS, validator=validate_option
        )
        distinct_ids(options, f"{path}.options")
        distinct_content(options, "content", f"{path}.options")
        result["options"] = options
        allowed = {option["id"] for option in options}
        if kind == "single_choice":
            correct = local_id(question["correctOptionId"], f"{path}.correctOptionId")
            if correct not in allowed:
                fail(f"{path}.correctOptionId", "must reference an available option")
            result["correctOptionId"] = correct
        else:
            raw_ids = require_list(question["correctOptionIds"], f"{path}.correctOptionIds")
            if len(raw_ids) > len(options):
                fail(f"{path}.correctOptionIds", "cannot contain more entries than options")
            ids: list[str] = []
            seen: set[str] = set()
            for index, raw in enumerate(raw_ids):
                current = local_id(raw, f"{path}.correctOptionIds[{index}]")
                if current not in allowed:
                    fail(f"{path}.correctOptionIds[{index}]", "must reference an available option")
                if current in seen:
                    fail(f"{path}.correctOptionIds[{index}]", "correct option references must be unique")
                seen.add(current)
                ids.append(current)
            result["correctOptionIds"] = ids

    elif kind == "true_false":
        result["statement"] = rich(question["statement"], f"{path}.statement")
        if type(question["correct"]) is not bool:
            fail(f"{path}.correct", "expected a Boolean")
        result["correct"] = question["correct"]
        if "correctedStatement" in question:
            corrected = optional_rich(question["correctedStatement"], f"{path}.correctedStatement")
            if corrected is not None:
                result["correctedStatement"] = corrected

    elif kind == "matching":
        result["prompt"] = rich(question["prompt"], f"{path}.prompt")
        pairs = collection(
            question["pairs"], f"{path}.pairs", minimum=2, maximum=16, validator=validate_pair
        )
        distinct_ids(pairs, f"{path}.pairs")
        distinct_content(pairs, "left", f"{path}.pairs")
        distinct_content(pairs, "right", f"{path}.pairs")
        result["pairs"] = pairs

    elif kind == "grouping":
        result["prompt"] = rich(question["prompt"], f"{path}.prompt")
        categories = collection(
            question["categories"], f"{path}.categories", minimum=2, maximum=8, validator=validate_item
        )
        items = collection(
            question["items"], f"{path}.items", minimum=2, maximum=MAX_ITEMS, validator=validate_grouped_item
        )
        distinct_ids(categories, f"{path}.categories")
        distinct_ids(items, f"{path}.items")
        distinct_content(categories, "content", f"{path}.categories")
        distinct_content(items, "content", f"{path}.items")
        allowed = {category["id"] for category in categories}
        for index, item in enumerate(items):
            if item["categoryId"] not in allowed:
                fail(f"{path}.items[{index}].categoryId", "must reference an available category")
        result["categories"] = categories
        result["items"] = items

    elif kind == "ordering":
        result["prompt"] = rich(question["prompt"], f"{path}.prompt")
        items = collection(
            question["items"], f"{path}.items", minimum=2, maximum=MAX_ITEMS, validator=validate_item
        )
        distinct_ids(items, f"{path}.items")
        distinct_content(items, "content", f"{path}.items")
        result["items"] = items

    if "explanation" in question:
        explanation = optional_rich(question["explanation"], f"{path}.explanation")
        if explanation is not None:
            result["explanation"] = explanation

    encoded = json.dumps(result, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_QUESTION_BYTES:
        fail(path, f"question transport exceeds the {MAX_QUESTION_BYTES}-byte preflight budget")
    return result


def required_question_keys(kind: str) -> set[str]:
    return {
        "single_choice": {"kind", "prompt", "options", "correctOptionId"},
        "select_all": {"kind", "prompt", "options", "correctOptionIds"},
        "true_false": {"kind", "statement", "correct"},
        "matching": {"kind", "prompt", "pairs"},
        "grouping": {"kind", "prompt", "categories", "items"},
        "ordering": {"kind", "prompt", "items"},
    }[kind]


def validate_card(value: Any, path: str, default_tags: list[str]) -> dict[str, Any]:
    card = require_object(value, path)
    card_type = card.get("type")
    if not isinstance(card_type, str) or card_type not in CARD_KEYS:
        fail(f"{path}.type", "unsupported generated card type")
    required = {
        "basic": {"type", "front", "back"},
        "cloze": {"type", "text"},
        "interactive": {"type", "question"},
    }[card_type]
    exact_keys(card, CARD_KEYS[card_type], required, path)

    card_tags = tags(card.get("tags", []), f"{path}.tags")
    if len(default_tags) + len(card_tags) > MAX_TAGS:
        fail(f"{path}.tags", f"defaultTags plus card tags may contain at most {MAX_TAGS} entries")
    combined = [tag.casefold() for tag in default_tags + card_tags]
    if len(set(combined)) > MAX_TAGS:
        fail(f"{path}.tags", f"combined normalized tags may contain at most {MAX_TAGS} unique values")

    result: dict[str, Any] = {"type": card_type}
    if card_type == "basic":
        result["front"] = rich(card["front"], f"{path}.front")
        result["back"] = rich(card["back"], f"{path}.back")
        reverse = card.get("reverseEnabled", False)
        if type(reverse) is not bool:
            fail(f"{path}.reverseEnabled", "expected a Boolean")
        if reverse:
            result["reverseEnabled"] = True
    elif card_type == "cloze":
        result["text"] = validate_cloze(card["text"], f"{path}.text")
        mode = card.get("mode", "one_at_a_time")
        if not isinstance(mode, str) or mode not in {"one_at_a_time", "all_together"}:
            fail(f"{path}.mode", "must be 'one_at_a_time' or 'all_together'")
        if mode != "one_at_a_time":
            result["mode"] = mode
    else:
        result["question"] = validate_question(card["question"], f"{path}.question")

    if card_tags:
        result["tags"] = card_tags
    return result


def validate_document(value: Any) -> dict[str, Any]:
    document = require_object(value, "$")
    exact_keys(document, TOP_KEYS, {"format", "formatVersion", "deck", "cards"}, "$")
    if document["format"] != "ostudy-flashcards":
        fail("$.format", "must equal 'ostudy-flashcards'")
    if type(document["formatVersion"]) is not int or document["formatVersion"] != 2:
        fail("$.formatVersion", "the skills-only generator writes integer format version 2")

    deck = require_object(document["deck"], "$.deck")
    exact_keys(deck, DECK_KEYS, {"mode", "name"}, "$.deck")
    mode = deck["mode"]
    if not isinstance(mode, str) or mode not in {"create", "append"}:
        fail("$.deck.mode", "must be 'create' or 'append'")
    deck_name = text(deck["name"], "$.deck.name", maximum=MAX_DECK_NAME_CHARS)
    description = text(
        deck.get("description", ""),
        "$.deck.description",
        maximum=MAX_DECK_DESCRIPTION_CHARS,
        allow_empty=True,
    )
    if mode == "append" and description:
        fail("$.deck.description", "must be omitted or empty when deck.mode is 'append'")

    default_tags = tags(document.get("defaultTags", []), "$.defaultTags")
    cards = require_list(document["cards"], "$.cards")
    if not 1 <= len(cards) <= MAX_CARDS:
        fail("$.cards", f"must contain between 1 and {MAX_CARDS} source cards")

    result: dict[str, Any] = {
        "format": "ostudy-flashcards",
        "formatVersion": 2,
        "deck": {"mode": mode, "name": deck_name},
    }
    if mode == "create" and description:
        result["deck"]["description"] = description
    if default_tags:
        result["defaultTags"] = default_tags
    result["cards"] = [
        validate_card(card, f"$.cards[{index}]", default_tags) for index, card in enumerate(cards)
    ]
    return result


def load_json(path: Path) -> Any:
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise ValidationError(str(path), f"could not read input: {error}") from error
    if not raw or len(raw) > MAX_FILE_BYTES:
        fail(str(path), f"input must be 1–{MAX_FILE_BYTES} bytes")
    try:
        return json.loads(raw.decode("utf-8"))
    except UnicodeDecodeError as error:
        fail(str(path), f"input must be UTF-8: {error}")
    except json.JSONDecodeError as error:
        fail(str(path), f"invalid JSON at line {error.lineno}, column {error.colno}")


def encode(document: dict[str, Any]) -> bytes:
    raw = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    if len(raw) > MAX_FILE_BYTES:
        fail("$", f"serialized file exceeds the {MAX_FILE_BYTES}-byte oStudy limit")
    return raw


def write_atomic(output: Path, raw: bytes) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{output.name}.", suffix=".tmp", dir=output.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(raw)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, output)
    except Exception:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate and write an oStudy v2 flashcard import file.")
    parser.add_argument("--input", required=True, type=Path, help="Draft UTF-8 JSON file.")
    parser.add_argument("--output", type=Path, help="Destination ending in .ostudy.json.")
    parser.add_argument("--validate-only", action="store_true", help="Validate without writing output.")
    args = parser.parse_args(argv)

    if not args.validate_only and args.output is None:
        parser.error("--output is required unless --validate-only is used")
    if args.output is not None and not str(args.output).endswith(".ostudy.json"):
        parser.error("--output must end in .ostudy.json")

    try:
        normalized = validate_document(load_json(args.input))
        raw = encode(normalized)
        # Reparse/revalidate the exact serialized bytes before any publication.
        final_document = validate_document(json.loads(raw.decode("utf-8")))
        final_raw = encode(final_document)
        digest = hashlib.sha256(final_raw).hexdigest()
        if args.validate_only:
            print(f"Valid oStudy v2 deck: {len(final_document['cards'])} source cards, sha256 {digest}")
            return 0
        assert args.output is not None
        write_atomic(args.output, final_raw)
        # Verify the bytes that are actually present after the atomic replace.
        written = args.output.read_bytes()
        if written != final_raw:
            fail(str(args.output), "written bytes did not match the validated artifact")
        validate_document(json.loads(written.decode("utf-8")))
        print(f"Wrote {args.output} ({len(final_document['cards'])} source cards, sha256 {digest})")
        return 0
    except ValidationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"ERROR: file operation failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
