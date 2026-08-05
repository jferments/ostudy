#!/usr/bin/env python3
"""Correct first fast-gate findings after the Stage 3.6–3.8 API cutover."""
from __future__ import annotations
import re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

# Imported deck display uses the same bounded summary projection as every deck detail.
p='src/screens/DecksScreen.tsx';t=read(p)
t=t.replace('''      const [deck, loadedCards] = await Promise.all([
        committedDeck ? Promise.resolve(committedDeck) : getDeck(deckId),
        listCards(deckId),
      ]);
      setSelectedDeck(deck);
      setCards(loadedCards);''','''      const [deck, page] = await Promise.all([
        committedDeck ? Promise.resolve(committedDeck) : getDeck(deckId),
        listDeckCards(deckId, "", 50, 0),
      ]);
      setSelectedDeck(deck);
      setCards(page.cards);
      setCardTotal(page.totalCount);''')
write(p,t)

# Remove an over-broad fixture migration artifact, then require archive state only on Deck objects.
for path in list((ROOT/'src').rglob('*.test.ts'))+list((ROOT/'src').rglob('*.test.tsx')):
    text=path.read_text(encoding='utf-8')
    text=re.sub(r'\n\s*archivedAt: null,','',text)
    def deck_block(match):
        block=match.group(0)
        if 'archivedAt:' not in block:
            block=block.replace('revision:', 'archivedAt: null,\n  revision:',1)
        return block
    text=re.sub(r'const\s+\w+\s*:\s*(?:ipc\.)?Deck\s*=\s*\{.*?\n\};',deck_block,text,flags=re.S)
    path.write_text(text,encoding='utf-8')

# Extended deck tests call the bounded page and use unit-explicit factory overrides.
p='src/screens/DecksScreen.extended.test.tsx';t=read(p)
t=t.replace('makeDeck({ name: "Cardiology", cardCount: 1 })','makeDeck({ name: "Cardiology", sourceNoteCount: 1, generatedCardCount: 1 })')
t=t.replace('cardCount: 0','sourceNoteCount: 0, generatedCardCount: 0')
t=t.replace('vi.mocked(ipc.listCards).mockResolvedValue([]);','vi.mocked(ipc.listDeckCards).mockResolvedValue({ cards: [], totalCount: 0 });')
write(p,t)

# The reschedule assertion is intentionally on the canonical request intent.
p='src/screens/cards/ScheduleActionModal.test.tsx';t=read(p)
start=t.find('    expect(typeof submitted?.dueAt).toBe("number");')
if start>=0:
    end=t.find('\n  });',start)
    t=t[:start]+'    expect(submitted?.reschedule).toEqual({ kind: "tomorrow" });'+t[end:]
write(p,t)

# Analytics due count appears in both the current summary and grouped rows.
p='src/screens/AnalyticsScreen.test.tsx';t=read(p).replace('expect(screen.getByText("4")).toBeTruthy();','expect(screen.getAllByText("4").length).toBeGreaterThan(0);');write(p,t)

print('Corrected bounded import hydration and first-pass frontend expectations.')
