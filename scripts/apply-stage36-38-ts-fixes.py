#!/usr/bin/env python3
"""Resolve strict TypeScript findings from the deliberate projection API cutover."""
from pathlib import Path
import re
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

# Tag summary counts source notes, not generated cards.
p='src-tauri/src/authoring/types.rs';t=read(p).replace('pub card_count: i64,','pub source_note_count: i64,',1);write(p,t)
p='src-tauri/src/authoring/search_and_tags.rs';t=read(p).replace('card_count: row.get(2)?,','source_note_count: row.get(2)?,',1);write(p,t)
p='src/ipc.ts';t=read(p).replace('''export interface TagSummary {
  id: string;
  name: string;
  cardCount: number;
}''','''export interface TagSummary {
  id: string;
  name: string;
  sourceNoteCount: number;
}''');write(p,t)
for p in ['src/screens/AnalyticsScreen.tsx','src/screens/CardsScreen.tsx']:
    t=read(p).replace('tag.generatedCardCount','tag.sourceNoteCount').replace('tag.cardCount','tag.sourceNoteCount');write(p,t)
for path in list((ROOT/'src').rglob('*.test.ts'))+list((ROOT/'src').rglob('*.test.tsx')):
    text=path.read_text(encoding='utf-8')
    # Named tag fixtures only; do not touch Deck generated-card counts.
    text=text.replace('name: "Arrhythmias renamed", cardCount: 6','name: "Arrhythmias renamed", sourceNoteCount: 6')
    text=text.replace('name: "Medication", cardCount:', 'name: "Medication", sourceNoteCount:')
    text=text.replace('name: "Cardiology", cardCount:', 'name: "Cardiology", sourceNoteCount:')
    path.write_text(text,encoding='utf-8')

# Analytics now launches a native scope directly and no longer accepts a card-ID callback.
p='src/screens/AnalyticsScreen.tsx';t=read(p)
t=t.replace('  onPracticeCards: (cardIds: string[]) => void;\n','').replace('  onPracticeCards,\n','')
write(p,t)
p='src/App.tsx';t=read(p).replace('              onPracticeCards={practiceCards}\n              onPracticeScope','              onPracticeScope');write(p,t)
for p in ['src/screens/AnalyticsScreen.test.tsx']:
    t=read(p).replace(' onPracticeCards={vi.fn()}','');write(p,t)

# Deck page uses authoritative total and lazily loads full details from both click targets.
p='src/screens/DecksScreen.tsx';t=read(p)
t=t.replace('''                  {visibleCards.length === cards.length
                    ? `${cards.length} ${
                        cards.length === 1 ? "source note" : "source notes"
                      }`
                    : `${visibleCards.length} of ${cards.length} source notes`}''','''                  {search.trim() === ""
                    ? `${cardTotal} ${cardTotal === 1 ? "source note" : "source notes"}`
                    : `${visibleCards.length} matching source notes`}''')
t=t.replace('''                          onClick={() => {
                            setEditingCard(card);
                            setView("editor");
                          }}''','''                          onClick={() => {
                            void getOrganizedCard(card.noteId).then((loaded) => {
                              setEditingCard(loaded);
                              setView("editor");
                            }).catch((error: unknown) => showToast("error", getErrorMessage(error)));
                          }}''',1)
write(p,t)

# Practice operation identity includes tag scope, and reschedule intent never carries a browser timestamp.
p='src/screens/PracticeScreen.tsx';t=read(p)
t=t.replace('''function selectedStartKey(
  deckIds: string[],
  shuffle: boolean,
  mode: "scheduled" | "extra",
): string {
  const canonicalDeckIds = [...new Set(deckIds)].sort();
  return JSON.stringify({ kind: "selected", deckIds: canonicalDeckIds, shuffle, mode });
}''','''function selectedStartKey(
  deckIds: string[],
  tagIds: string[],
  shuffle: boolean,
  mode: "scheduled" | "extra",
): string {
  return JSON.stringify({
    kind: "selected",
    deckIds: [...new Set(deckIds)].sort(),
    tagIds: [...new Set(tagIds)].sort(),
    shuffle,
    mode,
  });
}''')
t=t.replace('async function manageCurrent(action: ScheduleAction, dueAt?: number): Promise<void> {','async function manageCurrent(action: ScheduleAction, reschedule?: import("../ipc").RescheduleIntent): Promise<void> {')
t=t.replace('        dueAt,\n        sessionId:', '        reschedule,\n        sessionId:')
write(p,t)
p='src/screens/practice/ActivePractice.tsx';t=read(p)
t=t.replace('import type { PracticeSession, ReviewPreviewSet, ScheduleAction } from "../../ipc";','import type { PracticeSession, RescheduleIntent, ReviewPreviewSet, ScheduleAction } from "../../ipc";')
t=t.replace('onScheduleAction: (action: ScheduleAction, dueAt?: number) => void;','onScheduleAction: (action: ScheduleAction, reschedule?: RescheduleIntent) => void;')
t=t.replace('onScheduleAction("reschedule", tomorrowAtNoon())','onScheduleAction("reschedule", { kind: "tomorrow" })')
start=t.find('\nfunction tomorrowAtNoon(): number {')
if start>=0:t=t[:start]+'\n'
write(p,t)

# Saved full templates are projected back to the summary list.
p='src/screens/TemplatesScreen.tsx';t=read(p)
t=t.replace('''        return [...withoutSaved, saved].sort((left, right) =>
          left.name.localeCompare(right.name, undefined, { sensitivity: "base" }),
        );''','''        const summary: CardTemplateSummary = {
          id: saved.id,
          name: saved.name,
          description: saved.description,
          fieldCount: saved.fields.length,
          generatedCardRuleCount: saved.rules.length,
          imageCount: saved.assets.length,
          updatedAt: saved.updatedAt,
          revision: saved.revision,
        };
        return [...withoutSaved, summary].sort((left, right) =>
          left.name.localeCompare(right.name, undefined, { sensitivity: "base" }),
        );''')
write(p,t)

# Remaining strict fixtures.
p='src/screens/cards/ScheduleActionModal.test.tsx';t=read(p);t=re.sub(r'\n\s*const startedOn = new Date\(\);','',t);write(p,t)
p='src/screens/practice/ActivePractice.test.tsx';t=read(p)
t=t.replace('''  shuffle: false,
  mode: "scheduled",''','''  cardCount: 1,
  hasMore: false,
  shuffle: false,
  mode: "scheduled",''')
write(p,t)
p='src/screens/practice/confidence.test.ts';t=read(p)
t=t.replace('''    id: id.repeat(32),
    deckName: "Deck",''','''    id: id.repeat(32),
    deckId: "d".repeat(32),
    deckName: "Deck",''')
t=t.replace('''    dueAfter: 2,
  };''','''    dueAfter: 2,
    attributionSource: "captured",
  };''')
write(p,t)

# Final missed inline studydeck import fixture.
p='src/screens/DecksScreen.extended.test.tsx';t=read(p).replace('{ deck: imported, cardCount: 1, assetCount: 0 }','{ deck: imported, generatedCardCount: 1, assetCount: 0 }');write(p,t)

print('Resolved strict TypeScript projection-contract findings.')
