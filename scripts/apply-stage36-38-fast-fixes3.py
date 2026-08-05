#!/usr/bin/env python3
"""Make shareable-deck import and export result units explicit."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

p='src-tauri/src/studydeck.rs';t=read(p)
# second occurrence belongs to ImportResultDto
needle='pub card_count: i64,\n    pub asset_count: i64,'
if needle in t:t=t.replace(needle,'pub generated_card_count: i64,\n    pub asset_count: i64,',1)
write(p,t)
p='src-tauri/src/studydeck/import.rs';t=read(p).replace('card_count: existing.result_card_count.unwrap_or_default()','generated_card_count: existing.result_card_count.unwrap_or_default()').replace('card_count: result_card_count','generated_card_count: result_card_count');write(p,t)
p='src/ipc.ts';t=read(p).replace('''export interface ImportResult {
  deck: Deck;
  cardCount: number;
  assetCount: number;
}''','''export interface ImportResult {
  deck: Deck;
  generatedCardCount: number;
  assetCount: number;
}''');write(p,t)
for path in list((ROOT/'src').rglob('*.test.ts'))+list((ROOT/'src').rglob('*.test.tsx')):
    text=path.read_text(encoding='utf-8')
    text=text.replace('deck: imported,\n      cardCount:', 'deck: imported,\n      generatedCardCount:')
    text=text.replace('deck,\n      cardCount:', 'deck,\n      generatedCardCount:')
    path.write_text(text,encoding='utf-8')
print('Made studydeck import/export generated-card counts explicit.')
