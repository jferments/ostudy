#!/usr/bin/env python3
"""Update frontend fixtures/mocks to the deliberate Stage 3.6–3.8 API break."""
from __future__ import annotations
import re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p): return (ROOT/p).read_text(encoding='utf-8')
def write(p,c): (ROOT/p).write_text(c,encoding='utf-8')

# Shared factories define the new unit-explicit DTOs.
p='src/test/factories.ts';t=read(p)
t=t.replace('''    cardCount: 1,
    dueCount: 1,
    createdAt: 1,''','''    sourceNoteCount: 1,
    generatedCardCount: 1,
    studyableDueCount: 1,
    createdAt: 1,
    archivedAt: null,''')
t=t.replace('''    shuffle: false,
    mode: "scheduled",''','''    cardCount: 1,
    hasMore: false,
    shuffle: false,
    mode: "scheduled",''')
t=t.replace('''      deckCount: 0,
      cardCount: 0,
      dueNow: 0,''','''      currentDeckCount: 0,
      sourceNoteCount: 0,
      generatedCardCount: 0,
      studyableCardCount: 0,
      studyableDueCount: 0,''')
t=t.replace('''    recentReviews: [],
    recentSessions: [],''','''    recentReviews: [],
    recentSessions: [],
    dimensions: { decks: [], tags: [] },
    attributionCoverage: { capturedReviewCount: 0, legacyBackfilledReviewCount: 0 },''')
write(p,t)

# Every explicit test Deck / deck/tag analytics row migrates without aliases.
for path in list((ROOT/'src').rglob('*.test.ts'))+list((ROOT/'src').rglob('*.test.tsx')):
    rel=str(path.relative_to(ROOT));text=path.read_text(encoding='utf-8')
    text=re.sub(r'cardCount: ([^,\n]+),\n(\s*)dueCount: ([^,\n]+),',r'sourceNoteCount: \1,\n\2generatedCardCount: \1,\n\2studyableDueCount: \3,',text)
    text=text.replace('dueCount: 0,','studyableDueCount: 0,')
    # Deck DTOs require archive state; add after updatedAt when a revision immediately follows.
    text=re.sub(r'(updatedAt: [^,\n]+,\n\s*)(revision:)',r'\1archivedAt: null,\n  \2',text)
    path.write_text(text,encoding='utf-8')

# Exact IPC wrapper test: intentional removal of old unbounded commands.
p='src/ipc.test.ts';t=read(p)
t=t.replace('  getOrganizedCard,','  getOrganizedCard,\n  getPracticeCardWindow,')
t=t.replace('  listCardTemplates,\n  listCards,\n  listDecks,','  listArchivedDecks,\n  listCardTemplateSummaries,\n  listDeckCards,\n  listDecks,')
t=t.replace('  analyzeBulkCards,','  analyzeBulkCards,\n  archiveDeck,')
t=t.replace('  updateGeneratedCard,','  updateGeneratedCard,\n  unarchiveDeck,')
t=t.replace('await expectInvocation(listDecks, "list_decks");','await expectInvocation(listDecks, "list_decks");\n    await expectInvocation(listArchivedDecks, "list_archived_decks");\n    await expectInvocation(() => archiveDeck(deckId), "archive_deck", { deckId });\n    await expectInvocation(() => unarchiveDeck(deckId), "unarchive_deck", { deckId });')
t=t.replace('await expectInvocation(() => listCards(deckId), "list_cards", { deckId });','await expectInvocation(() => listDeckCards(deckId), "list_deck_cards", { input: { deckId, query: "", limit: 50, offset: 0 } });')
t=t.replace('await expectInvocation(listCardTemplates, "list_card_templates");','await expectInvocation(listCardTemplateSummaries, "list_card_template_summaries");')
t=t.replace('{ input: { deckIds: [deckId] } },','{ input: { deckIds: [deckId], tagIds: [] } },')
t=t.replace('''      deckIds: [deckId],
      shuffle: true,''','''      deckIds: [deckId],
      tagIds: [],
      shuffle: true,''')
# Window invocation after custom start.
anchor='''    await expectInvocation(() => startCardPractice(cardStart), "start_card_practice", {
      input: cardStart,
    });'''
if 'get_practice_card_window' not in t:
    t=t.replace(anchor,anchor+'''\n    await expectInvocation(() => getPracticeCardWindow("2".repeat(32), 50), "get_practice_card_window", { input: { sessionId: "2".repeat(32), startPosition: 50, limit: 50 } });''')
write(p,t)

# Analytics test rewritten around current/history separation and native tag scope.
write('src/screens/AnalyticsScreen.test.tsx',r'''import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {beforeEach,describe,expect,it,vi} from "vitest";
import * as analyticsIpc from "../analytics-ipc";import * as ipc from "../ipc";import {AnalyticsScreen} from "./AnalyticsScreen";
vi.mock("../analytics-ipc",()=>({getAnalytics:vi.fn()}));vi.mock("../ipc",()=>({getErrorMessage:(e:unknown)=>e instanceof Error?e.message:"Unexpected error",listTags:vi.fn()}));vi.mock("./analytics/ProgressChart",()=>({ProgressChart:()=> <div role="img" aria-label="Learning progress chart"/>}));
const deck:ipc.Deck={id:"a".repeat(32),name:"Cardiology renamed",description:"",sourceNoteCount:10,generatedCardCount:12,studyableDueCount:4,createdAt:1,updatedAt:1,archivedAt:null,revision:1};
const settings:ipc.AppSettings={theme:"dark",uiFont:"system",cardFont:"system",uiScale:100,cardBaseSize:18,reduceMotion:false,confirmDeletions:true,practiceShuffle:true,desiredRetention:.9,dailyNewLimit:20,dailyReviewLimit:200};
const tagId="b".repeat(32);const analytics:analyticsIpc.Analytics={scope:{range:"7d",startAt:1,endAt:2,bucketGranularity:"day",deckIds:[],tagIds:[],highlightedSessionId:null},summary:{currentDeckCount:1,sourceNoteCount:10,generatedCardCount:12,studyableCardCount:12,studyableDueCount:4,needsMorePractice:2,cardsPracticed:8,reviewCount:10,correctCount:8,correctRate:.8,averageConfidence:1.9,averageResponseTimeMs:4200,totalStudyTimeMs:120000,sessionCount:2},dailyLimits:{newLimit:20,reviewLimit:200,newCompleted:3,reviewsCompleted:10,newRemaining:17,reviewsRemaining:190},ratingDistribution:{again:2,hard:2,good:4,easy:2},timeline:[{startAt:1,endAt:2,cardsPracticed:8,reviewCount:10,newCards:3,rememberedCount:8,correctRate:.8,averageConfidence:1.9,averageResponseTimeMs:4200,studyTimeMs:120000,sessionCount:2,containsHighlightedSession:false}],deckStats:[{deckId:deck.id,deckName:deck.name,archived:false,sourceNoteCount:10,generatedCardCount:12,studyableDueCount:4,needsMorePractice:2,cardsPracticed:8,reviewCount:10,correctCount:8,correctRate:.8,averageConfidence:1.9,averageResponseTimeMs:4200,totalStudyTimeMs:120000,lastPracticedAt:1}],tagStats:[{tagId,tagName:"Arrhythmias renamed",sourceNoteCount:6,generatedCardCount:7,studyableDueCount:3,needsMorePractice:2,cardsPracticed:4,reviewCount:7,correctCount:4,correctRate:4/7,averageConfidence:1.4,averageResponseTimeMs:5000,totalStudyTimeMs:35000,lastPracticedAt:1,trendDelta:null,status:"needs_attention"}],recentReviews:[{id:"c".repeat(32),deckId:deck.id,deckName:deck.name,cardId:"d".repeat(32),frontText:"What rhythm has no P waves?",tags:["Arrhythmias renamed"],outcome:"somewhat_confident",rating:3,responseTimeMs:4000,reviewedAt:1,dueAfter:2,attributionSource:"captured"}],recentSessions:[],dimensions:{decks:[{id:deck.id,name:deck.name,archived:false,historicalOnly:false}],tags:[{id:tagId,name:"Arrhythmias renamed",archived:false,historicalOnly:false}]},attributionCoverage:{capturedReviewCount:10,legacyBackfilledReviewCount:0}};
describe("AnalyticsScreen",()=>{beforeEach(()=>{vi.clearAllMocks();vi.mocked(analyticsIpc.getAnalytics).mockResolvedValue(analytics);vi.mocked(ipc.listTags).mockResolvedValue([{id:tagId,name:"Arrhythmias renamed",cardCount:6}])});it("shows current inventory separately from immutable history under current names",async()=>{render(<AnalyticsScreen decks={[deck]} settings={settings} launchRequest={null} onLaunchConsumed={vi.fn()} onPracticeCards={vi.fn()} onPracticeScope={vi.fn()} showToast={vi.fn()}/>);expect(await screen.findByText("Cards practiced")).toBeTruthy();expect(screen.getAllByText("Cardiology renamed").length).toBeGreaterThan(0);expect(screen.getAllByText("Arrhythmias renamed").length).toBeGreaterThan(0);expect(screen.getByText("4")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"View chart data"}));expect(screen.getByRole("table",{name:"Data represented in the learning progress chart"})).toBeTruthy()});it("changes range and launches an uncapped native tag scope",async()=>{const onPracticeScope=vi.fn();render(<AnalyticsScreen decks={[deck]} settings={settings} launchRequest={null} onLaunchConsumed={vi.fn()} onPracticeCards={vi.fn()} onPracticeScope={onPracticeScope} showToast={vi.fn()}/>);await screen.findAllByText("Arrhythmias renamed");fireEvent.click(screen.getByRole("button",{name:"90 days"}));await waitFor(()=>expect(analyticsIpc.getAnalytics).toHaveBeenLastCalledWith(expect.objectContaining({range:"90d"})));fireEvent.click(screen.getByRole("button",{name:"Practice due"}));expect(onPracticeScope).toHaveBeenCalledWith([],[tagId])})});
''')

# Schedule modal now sends intent, never browser noon timestamps.
p='src/screens/cards/ScheduleActionModal.test.tsx';t=read(p)
t=t.replace('resolves a preset due date at local noon on the next calendar day','sends a backend-authoritative next-local-day intent')
pattern=r'''    expect\(typeof submitted\?\.dueAt\)\.toBe\("number"\);\n\n    const due = new Date\(submitted\?\.dueAt \?\? Number\.NaN\);\n    expect\(due\.getHours\(\)\)\.toBe\(12\);\n    expect\(due\.getMinutes\(\)\)\.toBe\(0\);'''
t=re.sub(pattern,'    expect(submitted?.reschedule).toEqual({ kind: "tomorrow" });',t)
write(p,t)

# Practice test deliberate DTO and request changes.
p='src/screens/PracticeScreen.test.tsx';t=read(p)
t=t.replace('  getPracticeWorkload: vi.fn(),','  getPracticeCardWindow: vi.fn(),\n  getPracticeWorkload: vi.fn(),')
t=t.replace('''  shuffle: true,
  mode: "scheduled",''','''  cardCount: 1,
  hasMore: false,
  shuffle: true,
  mode: "scheduled",''')
t=t.replace('''        deckIds: [deck.id],
        shuffle: true,''','''        deckIds: [deck.id],
        tagIds: [],
        shuffle: true,''')
t=t.replace('''        deckIds: [noDueDeck.id],
        shuffle: true,''','''        deckIds: [noDueDeck.id],
        tagIds: [],
        shuffle: true,''')
t=t.replace('''        cardIds: [card.id],
        autoStart: true,''','''        cardIds: [card.id],
        tagIds: [],
        autoStart: true,''')
t=t.replace('vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(workload);','vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(workload);\n    vi.mocked(ipc.getPracticeCardWindow).mockResolvedValue({ startPosition: 50, cards: [], hasMore: false });')
write(p,t)

# Add new mock exports to screen tests; retain their behavioral assertions.
for p in ['src/screens/DecksScreen.test.tsx','src/screens/DecksScreen.extended.test.tsx','src/screens/DecksScreen.undo.test.tsx']:
    t=read(p)
    t=t.replace('  createDeck: vi.fn(),','  archiveDeck: vi.fn(),\n  createDeck: vi.fn(),')
    t=t.replace('  getDeck: vi.fn(),','  getDeck: vi.fn(),\n  getOrganizedCard: vi.fn(),')
    t=t.replace('  listCards: vi.fn(),','  listArchivedDecks: vi.fn(),\n  listDeckCards: vi.fn(),')
    t=t.replace('  updateDeck: vi.fn(),','  unarchiveDeck: vi.fn(),\n  updateDeck: vi.fn(),')
    t=t.replace('vi.mocked(ipc.listCards).mockResolvedValue([card]);','vi.mocked(ipc.listArchivedDecks).mockResolvedValue([]);\n    vi.mocked(ipc.listDeckCards).mockResolvedValue({ cards: [{ noteId: card.noteId, primaryCardId: card.id, questionExcerpt: card.frontText, answerExcerpt: card.backText, generatedCardCount: 1, imageCount: 0, updatedAt: card.updatedAt }], totalCount: 1 });\n    vi.mocked(ipc.getOrganizedCard).mockResolvedValue({ ...card, tags: [], dueAt: Date.now() });')
    # ensure default archived mock even when no card setup anchor exists
    marker='    vi.clearAllMocks();'
    if 'mocked(ipc.listArchivedDecks).mockResolvedValue' not in t:
        t=t.replace(marker,marker+'\n    vi.mocked(ipc.listArchivedDecks).mockResolvedValue([]);',1)
    write(p,t)

# Template and card-editor mocks use summaries and lazy detail.
for p in ['src/screens/TemplatesScreen.test.tsx','src/screens/TemplatesScreen.assets.test.tsx','src/screens/CardEditorScreen.test.tsx']:
    t=read(p)
    t=t.replace('  listCardTemplates: vi.fn(),','  getCardTemplate: vi.fn(),\n  listCardTemplateSummaries: vi.fn(),')
    t=t.replace('vi.mocked(ipc.listCardTemplates).mockResolvedValue([]);','vi.mocked(ipc.listCardTemplateSummaries).mockResolvedValue([]);')
    t=t.replace('vi.mocked(ipc.listCardTemplates).mockResolvedValue([template]);','vi.mocked(ipc.listCardTemplateSummaries).mockResolvedValue([{ id: template.id, name: template.name, description: template.description, fieldCount: template.fields.length, generatedCardRuleCount: template.rules.length, imageCount: template.assets.length, updatedAt: template.updatedAt, revision: template.revision }]);\n    vi.mocked(ipc.getCardTemplate).mockResolvedValue(template);')
    write(p,t)

# Analytics view tolerates old in-memory fixtures during loading; response types still require dimensions.
p='src/screens/AnalyticsScreen.tsx';t=read(p).replace('analytics?.dimensions.decks','analytics?.dimensions?.decks');write(p,t)

print('Updated Stage 3.6–3.8 frontend fixtures, mocks, and behavioral expectations.')
