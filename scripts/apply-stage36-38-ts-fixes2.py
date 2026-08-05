#!/usr/bin/env python3
"""Resolve remaining strict frontend contract names after TagSummary/history changes."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

p='src/components/TagInput.tsx';t=read(p).replace('suggestion.cardCount','suggestion.sourceNoteCount');write(p,t)
for p in ['src/components/TagInput.test.tsx','src/screens/AnalyticsScreen.test.tsx','src/screens/CardsScreen.test.tsx']:
    t=read(p).replace('cardCount:', 'sourceNoteCount:');write(p,t)
p='src/screens/practice/confidence.test.ts';t=read(p)
# add only when absent in the RecentReview fixture
if 'deckId:' not in t:t=t.replace('deckName: "Deck",','deckId: "d".repeat(32),\n    deckName: "Deck",',1)
if 'attributionSource:' not in t:t=t.replace('dueAfter: 2,','dueAfter: 2,\n    attributionSource: "captured",',1)
write(p,t)
print('Resolved remaining TagSummary and RecentReview strict contracts.')
