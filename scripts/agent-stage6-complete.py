#!/usr/bin/env python3
"""Apply the remaining Stage 6 provenance and card-surface changes.

This is a one-shot repository-local migration helper. The accompanying workflow
runs it against the exact PR head, validates the result, commits the real source
changes, and removes this helper before pushing.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def regex_once(path: str, pattern: str, replacement: str, flags: int = 0) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise RuntimeError(f"{path}: expected one regex match, found {count}: {pattern}")
    write(path, updated)


# ---------------------------------------------------------------------------
# Typed card-search citation projection
# ---------------------------------------------------------------------------

cards_ts = "src/ipc/cards.ts"
text = read(cards_ts)
if "export interface CardSourceCitationSummary" not in text:
    marker = "export interface CardBrowserRow {"
    if marker not in text:
        raise RuntimeError(f"{cards_ts}: CardBrowserRow marker missing")
    summary = """export interface CardSourceCitationSummary {
  sourceTitle: string;
  originalFilename: string;
  sourceRemoved: boolean;
}

"""
    text = text.replace(marker, summary + marker, 1)
if "sourceCitation?: CardSourceCitationSummary | null;" not in text:
    text, count = re.subn(
        r"(export interface CardBrowserRow \{.*?\n\s*backText: string;)",
        r"\1\n  sourceCitation?: CardSourceCitationSummary | null;",
        text,
        count=1,
        flags=re.S,
    )
    if count != 1:
        raise RuntimeError(f"{cards_ts}: could not add sourceCitation to CardBrowserRow")
write(cards_ts, text)


authoring = "src-tauri/src/authoring/mod.rs"
text = read(authoring)
if "pub struct CardSourceCitationSummaryDto" not in text:
    marker = "#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct CardBrowserRowDto {"
    if marker not in text:
        raise RuntimeError(f"{authoring}: CardBrowserRowDto marker missing")
    citation_struct = """#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSourceCitationSummaryDto {
    pub source_title: String,
    pub original_filename: String,
    pub source_removed: bool,
}

"""
    text = text.replace(marker, citation_struct + marker, 1)
if "pub source_citation: Option<CardSourceCitationSummaryDto>," not in text:
    old = "    pub front_text: String,\n    pub back_text: String,\n    pub tags: Vec<String>,"
    new = "    pub front_text: String,\n    pub back_text: String,\n    pub source_citation: Option<CardSourceCitationSummaryDto>,\n    pub tags: Vec<String>,"
    if text.count(old) != 1:
        raise RuntimeError(f"{authoring}: CardBrowserRowDto field sequence changed")
    text = text.replace(old, new, 1)

# Add one-to-one citation joins to the card row query only. They are kept out of
# the count/selection queries because provenance is presentation metadata and
# must not change result cardinality.
if "citation.source_title" not in text:
    old = """             n.cloze_source, COALESCE(n.cloze_mode, 'one_at_a_time'),
             {signal_value}
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         {joins}
"""
    new = """             n.cloze_source, COALESCE(n.cloze_mode, 'one_at_a_time'),
             {signal_value},
             citation.source_title,
             citation.original_filename,
             citation.source_removed
         FROM notes n
         JOIN cards c ON c.note_id = n.id
         JOIN decks d ON d.id = c.deck_id
         JOIN card_schedules s ON s.card_id = c.id
         LEFT JOIN note_source_citations AS note_citation
           ON note_citation.note_id = n.id
         LEFT JOIN source_citation_snapshots AS citation
           ON citation.id = note_citation.citation_id
         {joins}
"""
    if text.count(old) != 1:
        raise RuntimeError(f"{authoring}: card row SELECT shape changed")
    text = text.replace(old, new, 1)

if "let source_citation = match row.get::<_, Option<String>>(22)?" not in text:
    marker = """            let grouped_tags: String = row.get(4)?;
            Ok(CardBrowserRowDto {
"""
    replacement = """            let grouped_tags: String = row.get(4)?;
            let source_citation = match row.get::<_, Option<String>>(22)? {
                Some(source_title) => Some(CardSourceCitationSummaryDto {
                    source_title,
                    original_filename: row.get(23)?,
                    source_removed: row.get::<_, Option<i64>>(24)?.unwrap_or(0) != 0,
                }),
                None => None,
            };
            Ok(CardBrowserRowDto {
"""
    if text.count(marker) != 1:
        raise RuntimeError(f"{authoring}: card row mapping marker changed")
    text = text.replace(marker, replacement, 1)
if "                source_citation," not in text:
    old = """                front_text: rendered.front_text,
                back_text: rendered.back_text,
                tags: split_tags(&grouped_tags),
"""
    new = """                front_text: rendered.front_text,
                back_text: rendered.back_text,
                source_citation,
                tags: split_tags(&grouped_tags),
"""
    if text.count(old) != 1:
        raise RuntimeError(f"{authoring}: CardBrowserRowDto construction changed")
    text = text.replace(old, new, 1)
write(authoring, text)


# ---------------------------------------------------------------------------
# Reusable compact citation treatment and card-detail provenance
# ---------------------------------------------------------------------------

write(
    "src/components/CardSourceCitationSummary.tsx",
    """import type { CardSourceCitationSummary as CardSourceCitation } from "../ipc/cards";
import styles from "./CardSourceCitationSummary.module.css";

interface CardSourceCitationSummaryProps {
  citation: CardSourceCitation;
}

/** Compact provenance for a card row; exact locators remain in card detail. */
export function CardSourceCitationSummary({ citation }: CardSourceCitationSummaryProps) {
  return (
    <small className={styles.summary} data-card-source-citation>
      <span>Source: {citation.sourceTitle}</span>
      <span aria-hidden="true"> · </span>
      <span>{citation.originalFilename}</span>
      {citation.sourceRemoved ? (
        <span className={styles.removed}> · source removed; citation preserved</span>
      ) : null}
    </small>
  );
}
""",
)
write(
    "src/components/CardSourceCitationSummary.module.css",
    """@layer components {
  .summary {
    display: block;
    min-width: 0;
    overflow: hidden;
    color: var(--ostudy-color-text-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .removed {
    color: var(--ostudy-color-status-warning);
    font-weight: 650;
  }
}
""",
)
write(
    "src/components/CardSourceCitationSummary.test.tsx",
    """import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CardSourceCitationSummary } from "./CardSourceCitationSummary";

describe("CardSourceCitationSummary", () => {
  it("shows compact live provenance", () => {
    render(
      <CardSourceCitationSummary
        citation={{
          sourceTitle: "Shock physiology",
          originalFilename: "shock.pdf",
          sourceRemoved: false,
        }}
      />,
    );

    expect(screen.getByText(/Source: Shock physiology/u)).toBeTruthy();
    expect(screen.getByText("shock.pdf")).toBeTruthy();
  });

  it("keeps a deleted source visibly attributable", () => {
    render(
      <CardSourceCitationSummary
        citation={{
          sourceTitle: "Shock physiology",
          originalFilename: "shock.pdf",
          sourceRemoved: true,
        }}
      />,
    );

    expect(screen.getByText(/source removed; citation preserved/u)).toBeTruthy();
  });
});
""",
)

workspace = "src/screens/cards/CardBrowserWorkspace.tsx"
text = read(workspace)
if 'import { CardSourceCitationSummary } from "../../components/CardSourceCitationSummary";' not in text:
    marker = 'import type { ShowToast } from "../../components/Toast";\n'
    if marker not in text:
        raise RuntimeError(f"{workspace}: Toast import marker missing")
    text = text.replace(
        marker,
        'import { CardSourceCitationSummary } from "../../components/CardSourceCitationSummary";\n'
        + marker,
        1,
    )
if 'import { SourceCitationPanel } from "../../components/SourceCitationPanel";' not in text:
    marker = 'import { CardSourceCitationSummary } from "../../components/CardSourceCitationSummary";\n'
    text = text.replace(
        marker,
        marker + 'import { SourceCitationPanel } from "../../components/SourceCitationPanel";\n',
        1,
    )

if "<SourceCitationPanel noteId={editingCard.noteId} compact />" not in text:
    old = """        <CardEditorScreen
          deck={deck}
"""
    new = """        {editingCard ? <SourceCitationPanel noteId={editingCard.noteId} compact /> : null}
        <CardEditorScreen
          deck={deck}
"""
    if text.count(old) != 1:
        raise RuntimeError(f"{workspace}: editor insertion marker changed")
    text = text.replace(old, new, 1)

if "<CardSourceCitationSummary citation={card.sourceCitation}" not in text:
    old = """        <span className={ownedClassNames(ownedStyleMaps, "organized-card-tags")}>
          {card.tags.length === 0 ? (
"""
    new = """        <span className={ownedClassNames(ownedStyleMaps, "organized-card-tags")}>
          {card.sourceCitation ? (
            <CardSourceCitationSummary citation={card.sourceCitation} />
          ) : null}
          {card.tags.length === 0 ? (
"""
    if text.count(old) != 1:
        raise RuntimeError(f"{workspace}: card tag/source insertion marker changed")
    text = text.replace(old, new, 1)
write(workspace, text)


# ---------------------------------------------------------------------------
# Ensure source citation tables participate in table-driven full-library copy
# paths. The ordinary full backup is a database image on current builds; this
# also protects any maintained table-order lists used by validation/import.
# ---------------------------------------------------------------------------

citation_literals = (
    '"source_citation_snapshots",\n'
    '        "deck_source_citations",\n'
    '        "note_source_citations",\n'
)
for path in sorted((ROOT / "src-tauri/src/library_backup").rglob("*.rs")):
    text = path.read_text(encoding="utf-8")
    if '"document_note_topics"' not in text or '"source_citation_snapshots"' in text:
        continue
    # Only modify a table-like string list, never SQL prose.
    pattern = re.compile(r'(?P<indent>\s*)"document_note_topics",')
    match = pattern.search(text)
    if not match:
        continue
    indent = match.group("indent")
    insertion = (
        match.group(0)
        + f'\n{indent}"source_citation_snapshots",'
        + f'\n{indent}"deck_source_citations",'
        + f'\n{indent}"note_source_citations",'
    )
    text = text[: match.start()] + insertion + text[match.end() :]
    path.write_text(text, encoding="utf-8")

print("Stage 6 patch applied.")
