#!/usr/bin/env python3
"""Apply exact Stage 3.3 Clippy fixes once, then remove this helper."""

from __future__ import annotations

from pathlib import Path


def split_top_level_arguments(value: str) -> list[str]:
    arguments: list[str] = []
    start = 0
    round_depth = square_depth = brace_depth = 0
    in_string = False
    escaped = False
    for index, character in enumerate(value):
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
        elif character == "(":
            round_depth += 1
        elif character == ")":
            round_depth -= 1
        elif character == "[":
            square_depth += 1
        elif character == "]":
            square_depth -= 1
        elif character == "{":
            brace_depth += 1
        elif character == "}":
            brace_depth -= 1
        elif character == "," and round_depth == square_depth == brace_depth == 0:
            arguments.append(value[start:index].strip())
            start = index + 1
    tail = value[start:].strip()
    if tail:
        arguments.append(tail)
    return arguments


def find_matching_parenthesis(text: str, opening: int) -> int:
    depth = 0
    in_string = False
    escaped = False
    for index in range(opening, len(text)):
        character = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
        elif character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                return index
    raise RuntimeError("unbalanced transition_asset invocation")


def rewrite_transition_calls(text: str) -> str:
    needle = "transition_asset("
    replacements: list[tuple[int, int, str]] = []
    cursor = 0
    while True:
        start = text.find(needle, cursor)
        if start < 0:
            break
        prefix = text[max(0, start - 3):start]
        opening = start + len("transition_asset")
        closing = find_matching_parenthesis(text, opening)
        cursor = closing + 1
        if prefix == "fn ":
            continue
        arguments = split_top_level_arguments(text[opening + 1:closing])
        if len(arguments) != 8:
            raise RuntimeError(
                f"expected 8 transition_asset arguments, found {len(arguments)} at {start}"
            )
        connection, asset, state, staged, quarantined, reason, now, event = arguments
        replacement = (
            "transition_asset(\n"
            f"            {connection},\n"
            f"            {asset},\n"
            "            AssetTransition {\n"
            f"                state: {state},\n"
            f"                staged_relative_path: {staged},\n"
            f"                quarantine_relative_path: {quarantined},\n"
            f"                reason: {reason},\n"
            f"                event: {event},\n"
            "            },\n"
            f"            {now},\n"
            "        )"
        )
        replacements.append((start, closing + 1, replacement))
    for start, end, replacement in reversed(replacements):
        text = text[:start] + replacement + text[end:]
    if not replacements:
        raise RuntimeError("no transition_asset call sites were rewritten")
    return text


asset_path = Path("src-tauri/src/asset_lifecycle.rs")
asset_text = asset_path.read_text(encoding="utf-8")
old_signature = '''fn transition_asset(
    connection: &Connection,
    asset: &AssetRecord,
    state: &str,
    staged_relative_path: Option<&str>,
    quarantine_relative_path: Option<&str>,
    reason: &str,
    now: i64,
    event: &str,
) -> Result<(), CommandError> {
'''
new_signature = '''struct AssetTransition<'a> {
    state: &'a str,
    staged_relative_path: Option<&'a str>,
    quarantine_relative_path: Option<&'a str>,
    reason: &'a str,
    event: &'a str,
}

fn transition_asset(
    connection: &Connection,
    asset: &AssetRecord,
    transition: AssetTransition<'_>,
    now: i64,
) -> Result<(), CommandError> {
    let AssetTransition {
        state,
        staged_relative_path,
        quarantine_relative_path,
        reason,
        event,
    } = transition;
'''
if asset_text.count(old_signature) != 1:
    raise SystemExit("transition_asset signature anchor was not found exactly once")
asset_text = asset_text.replace(old_signature, new_signature, 1)
asset_text = rewrite_transition_calls(asset_text)
asset_path.write_text(asset_text, encoding="utf-8")

deletion_path = Path("src-tauri/src/deletion.rs")
deletion_text = deletion_path.read_text(encoding="utf-8")
old_closure = ".and_then(|row| operation_from_values(row))"
if deletion_text.count(old_closure) != 1:
    raise SystemExit("redundant deletion closure was not found exactly once")
deletion_path.write_text(
    deletion_text.replace(old_closure, ".and_then(operation_from_values)", 1),
    encoding="utf-8",
)
