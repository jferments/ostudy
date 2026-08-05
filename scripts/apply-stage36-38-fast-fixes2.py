#!/usr/bin/env python3
"""Finish exact deck import/export contract migration."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

p='src-tauri/src/studydeck.rs';t=read(p).replace('pub card_count: i64,\n    pub asset_count: i64,','pub generated_card_count: i64,\n    pub asset_count: i64,',1);write(p,t)
p='src-tauri/src/studydeck/export.rs';t=read(p).replace('card_count: i64::try_from(package_data.cards.len())','generated_card_count: i64::try_from(package_data.cards.len())',1);write(p,t)
p='src/ipc.ts';t=read(p).replace('''export interface ExportResult {
  destinationPath: string;
  cardCount: number;
  assetCount: number;
}''','''export interface ExportResult {
  destinationPath: string;
  generatedCardCount: number;
  assetCount: number;
}''');write(p,t)

p='src/screens/DecksScreen.extended.test.tsx';t=read(p)
t=t.replace('makeDeck({ name: "Imported pharmacology", cardCount: 3 })','makeDeck({ name: "Imported pharmacology", sourceNoteCount: 2, generatedCardCount: 3 })')
t=t.replace('''    vi.mocked(ipc.listCards).mockResolvedValue([
      makeFlashcard({ deckId: imported.id }),
    ]);''','''    const importedCard = makeFlashcard({ deckId: imported.id });
    vi.mocked(ipc.listDeckCards).mockResolvedValue({
      cards: [{ noteId: importedCard.noteId, primaryCardId: importedCard.id,
        questionExcerpt: importedCard.frontText, answerExcerpt: importedCard.backText,
        generatedCardCount: 1, imageCount: 0, updatedAt: importedCard.updatedAt }],
      totalCount: 1,
    });''')
t=t.replace('makeDeck({ name: "Imported deck", cardCount: 2 })','makeDeck({ name: "Imported deck", sourceNoteCount: 2, generatedCardCount: 2 })')
t=t.replace('vi.mocked(ipc.listCards).mockRejectedValue(new Error("Card list unavailable."));','vi.mocked(ipc.listDeckCards).mockRejectedValue(new Error("Card list unavailable."));')
t=t.replace('makeDeck({ name: "Recovered import", cardCount: 1 })','makeDeck({ name: "Recovered import", sourceNoteCount: 1, generatedCardCount: 1 })')
t=t.replace('''      destinationPath: "/tmp/cardiology.studydeck",
      cardCount: 3,''','''      destinationPath: "/tmp/cardiology.studydeck",
      generatedCardCount: 3,''')
write(p,t)

# Exact export DTO expectations elsewhere.
for path in list((ROOT/'src').rglob('*.test.ts'))+list((ROOT/'src').rglob('*.test.tsx')):
    text=path.read_text(encoding='utf-8')
    text=text.replace('destinationPath: "/tmp/deck.studydeck",\n          cardCount:', 'destinationPath: "/tmp/deck.studydeck",\n          generatedCardCount:')
    path.write_text(text,encoding='utf-8')

print('Corrected bounded import pages and explicit generated-card export count.')
