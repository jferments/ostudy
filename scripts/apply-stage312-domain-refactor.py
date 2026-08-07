#!/usr/bin/env python3
from __future__ import annotations

import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
RUST = ROOT / "src-tauri" / "src"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one occurrence, found {count}: {old[:100]!r}")
    write(path, text.replace(old, new, 1))


def regex_once(path: str, pattern: str, replacement: str, flags: int = 0) -> None:
    text = read(path)
    result, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(f"{path}: expected regex once, found {count}: {pattern}")
    write(path, result)


# Move transport tests beside their final domain gateways before relative import rewriting.
for old, new in {
    "src/ipc.test.ts": "src/ipc/domain-contracts.test.ts",
    "src/analytics-ipc.test.ts": "src/ipc/analytics.test.ts",
    "src/generatedImport.test.ts": "src/ipc/generated-import.test.ts",
    "src/diagnostics.test.ts": "src/ipc/diagnostics.test.ts",
}.items():
    source = ROOT / old
    target = ROOT / new
    if not source.exists():
        raise SystemExit(f"missing expected transport test: {old}")
    target.parent.mkdir(parents=True, exist_ok=True)
    source.rename(target)

# Semantic TypeScript renames. Word boundaries intentionally avoid names such as
# FlashcardImportResult.
TS_RENAMES = {
    "OrganizedCard": "CardNote",
    "Flashcard": "CardPresentation",
    "CreateGeneratedCardInput": "CreateCardNoteInput",
    "UpdateGeneratedCardInput": "UpdateCardNoteInput",
    "getOrganizedCard": "getCardNote",
    "createGeneratedCard": "createCardNote",
    "updateGeneratedCard": "updateCardNote",
    "makeOrganizedCard": "makeCardNote",
    "makeFlashcard": "makeCardPresentation",
}
for path in SRC.rglob("*"):
    if path.suffix not in {".ts", ".tsx"}:
        continue
    text = path.read_text(encoding="utf-8")
    for old, new in TS_RENAMES.items():
        text = re.sub(rf"\b{re.escape(old)}\b", new, text)
    text = text.replace('"get_organized_card"', '"get_card_note"')
    text = text.replace('"create_generated_card"', '"create_card_note"')
    text = text.replace('"update_generated_card"', '"update_card_note"')
    path.write_text(text, encoding="utf-8")

# The complete public symbol -> domain map for the former src/ipc.ts gateway.
DOMAINS = {
    # common
    **{name: "common" for name in [
        "RichDocument", "DeletionResult", "UndoDeletionResult", "CommandErrorPayload",
        "createOperationId", "createSubmissionId", "createMutationId", "getErrorCode", "getErrorMessage",
    ]},
    # assets
    **{name: "assets" for name in [
        "Asset", "AssetDraft", "beginAssetDraft", "cancelAssetDraft", "syncAssetDraft", "importImage",
    ]},
    # decks / studydeck
    **{name: "decks" for name in [
        "DeckSummary", "CreateDeckInput", "UpdateDeckInput", "SetDeckArchivedInput",
        "DeckCardSummaryQuery", "DeckCardSummary", "DeckCardSummaryPage", "ExportResult", "ImportResult",
        "listDecks", "getDeck", "createDeck", "updateDeck", "setDeckArchived", "deleteDeck",
        "listDeckCardSummaries", "exportDeck", "importDeck",
    ]},
    # cards / organization
    **{name: "cards" for name in [
        "BuiltInCardType", "ClozeMode", "CardNote", "CardPresentation", "CreateCardNoteInput", "UpdateCardNoteInput",
        "TagSummary", "AvailabilityFilter", "CardStatusFilter", "ReviewedFilter", "CardSort", "CardSearchInput",
        "CardBrowserRow", "CardSearchPage", "SelectionCard", "DuplicateCheckInput", "DuplicateCard",
        "BulkCardRowInput", "AnalyzeBulkCardsInput", "BulkAnalyzeResult", "BulkCreateCardsInput", "BulkCreateResult",
        "TransferCardsMode", "TransferCardsInput", "TransferCardsResult", "TagChangeMode", "BulkTagCardsInput",
        "BulkActionResult", "listTags", "searchCards", "listMatchingCards", "getCardNote", "createCardNote",
        "updateCardNote", "findDuplicateCards", "analyzeBulkCards", "bulkCreateCards", "transferCards",
        "bulkTagCards", "deleteCard", "deleteCards", "undoDeletion",
    ]},
    # templates
    **{name: "templates" for name in [
        "CardTemplateField", "CardTemplateRule", "CardTemplate", "CardTemplateSummary", "CardTemplateFieldInput",
        "CardTemplateRuleInput", "CreateCardTemplateInput", "UpdateCardTemplateInput", "TemplateValueInput",
        "TemplateCardPreview", "PreviewTemplateCardsInput", "GenerateTemplateCardsInput", "GenerateTemplateCardsResult",
        "listCardTemplateSummaries", "getCardTemplate", "createCardTemplate", "updateCardTemplate",
        "deleteCardTemplate", "previewTemplateCards", "generateCardsFromTemplate",
    ]},
    # settings
    **{name: "settings" for name in ["AppSettings", "AppInfo", "getSettings", "saveSettings", "getAppInfo"]},
    # practice
    **{name: "practice" for name in [
        "PracticeMode", "PracticeCard", "PracticeCardWindow", "PracticeSession", "PracticeWorkloadOverview",
        "StartPracticeInput", "StartCardPracticeInput", "PracticeCardWindowInput", "ReviewOutcome", "IntervalPreview",
        "ReviewPreviewSet", "SubmitReviewInput", "ReviewResult", "getPracticeWorkload", "startSelectedPractice",
        "startCardPractice", "getPracticeCardWindow", "markPracticeCardViewed", "markPracticeCardRevealed",
        "submitPracticeReview", "completePracticeSession",
    ]},
    # scheduling
    **{name: "scheduling" for name in [
        "ScheduleAction", "RescheduleIntent", "ScheduleActionInput", "ScheduleActionResult", "applyScheduleAction",
    ]},
    # backups
    **{name: "backup" for name in [
        "LibraryBackupResult", "LibraryRestoreResult", "exportLibraryBackup", "prepareLibraryRestore",
    ]},
}

IMPORT_RE = re.compile(
    r'import\s+(?P<alltype>type\s+)?\{(?P<body>[\s\S]*?)\}\s+from\s+["\'](?P<source>[^"\']+)["\'];'
)


def module_path(source_file: Path, domain: str) -> str:
    target = SRC / "ipc" / domain
    rel = os.path.relpath(target, source_file.parent).replace(os.sep, "/")
    return rel if rel.startswith(".") else f"./{rel}"


def split_items(body: str) -> list[str]:
    # Existing IPC imports contain simple named bindings only; aliases and `type`
    # modifiers are supported, but no nested syntax is legal inside the braces.
    return [item.strip() for item in body.split(",") if item.strip()]


def rewrite_imports(path: Path, text: str) -> str:
    def repl(match: re.Match[str]) -> str:
        source = match.group("source")
        leaf = source.rsplit("/", 1)[-1]
        if leaf in {"analytics-ipc", "generatedImport", "diagnostics"}:
            domain = {
                "analytics-ipc": "analytics",
                "generatedImport": "generated-import",
                "diagnostics": "diagnostics",
            }[leaf]
            return f'import {"type " if match.group("alltype") else ""}{{{match.group("body")}}} from "{module_path(path, domain)}";'
        if leaf != "ipc":
            return match.group(0)

        grouped: dict[str, list[tuple[str, bool]]] = {}
        all_type = match.group("alltype") is not None
        for item in split_items(match.group("body")):
            is_type = all_type or item.startswith("type ")
            cleaned = item[5:].strip() if item.startswith("type ") else item
            symbol = cleaned.split(" as ", 1)[0].strip()
            if symbol in {"getCardTypeInfo", "CardTypeInfo"}:
                continue
            domain = DOMAINS.get(symbol)
            if domain is None:
                raise SystemExit(f"{path.relative_to(ROOT)}: unmapped IPC symbol {symbol}")
            grouped.setdefault(domain, []).append((cleaned, is_type))

        imports: list[str] = []
        for domain in sorted(grouped):
            bindings = grouped[domain]
            rendered = ",\n  ".join((f"type {name}" if is_type else name) for name, is_type in bindings)
            imports.append(f'import {{\n  {rendered},\n}} from "{module_path(path, domain)}";')
        return "\n".join(imports)

    return IMPORT_RE.sub(repl, text)


for path in SRC.rglob("*"):
    if path.suffix not in {".ts", ".tsx"}:
        continue
    text = path.read_text(encoding="utf-8")
    text = rewrite_imports(path, text)
    path.write_text(text, encoding="utf-8")

# Moved tests now refer to sibling domain modules, not their retired filenames.
for test, old, new in [
    ("src/ipc/analytics.test.ts", "./analytics-ipc", "./analytics"),
    ("src/ipc/generated-import.test.ts", "./generatedImport", "./generated-import"),
    ("src/ipc/diagnostics.test.ts", "./diagnostics", "./diagnostics"),
]:
    text = read(test)
    text = text.replace(old, new)
    write(test, text)

# Card Editor consumes the complete discriminated CardNote response directly;
# the old follow-up card-type request disappears.
path = "src/screens/CardEditorScreen.tsx"
text = read(path)
text = text.replace('  const [selectedTemplateKey, setSelectedTemplateKey] = useState("basic");',
                    '  const [selectedTemplateKey, setSelectedTemplateKey] = useState(card?.cardType ?? "basic");')
text = text.replace('  const [isLoadingType, setIsLoadingType] = useState(existing);\n', '')
text = text.replace('  const [reverseEnabled, setReverseEnabled] = useState(false);',
                    '  const [reverseEnabled, setReverseEnabled] = useState(\n    card?.cardType === "basic" ? card.reverseEnabled : false,\n  );')
text = text.replace('  const [clozeSource, setClozeSource] = useState("");',
                    '  const [clozeSource, setClozeSource] = useState(\n    card?.cardType === "cloze" ? card.clozeSource : "",\n  );')
text = text.replace('  const [clozeMode, setClozeMode] = useState<ClozeMode>("one_at_a_time");',
                    '  const [clozeMode, setClozeMode] = useState<ClozeMode>(\n    card?.cardType === "cloze" ? card.clozeMode : "one_at_a_time",\n  );')
text = text.replace(
'''  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.frontDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    card ? cloneDocument(card.backDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(card?.frontText ?? "");
  const [backText, setBackText] = useState(card?.backText ?? "");
  const [assets, setAssets] = useState<Asset[]>(card?.assets ?? []);''',
'''  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    card?.cardType === "basic" ? cloneDocument(card.frontDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    card?.cardType === "basic" ? cloneDocument(card.backDoc) : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(card?.cardType === "basic" ? card.frontText : "");
  const [backText, setBackText] = useState(card?.cardType === "basic" ? card.backText : "");
  const [assets, setAssets] = useState<Asset[]>(card?.cardType === "basic" ? card.assets : []);''')
text = text.replace('    !isLoadingType &&\n', '')
text, count = re.subn(
    r'\n  useEffect\(\(\) => \{\n    if \(!existing \|\| !card\) \{[\s\S]*?\n  \}, \[card, existing\]\);\n',
    '\n', text, count=1)
if count != 1:
    raise SystemExit("CardEditor: expected old card-type loading effect")
write(path, text)

# Test factories express the new domains explicitly.
path = "src/test/factories.ts"
text = read(path)
text = text.replace('  CardPresentation,\n  CardNote,', '  CardPresentation,\n  CardNote,')
# Replace the old presentation factory body after semantic renaming.
text, count = re.subn(
    r'export function makeCardPresentation\(overrides: Partial<CardPresentation> = \{\}\): CardPresentation \{[\s\S]*?\n\}\n\nexport function makeCardNote\([\s\S]*?\n\}\n',
'''export function makeCardPresentation(
  overrides: Partial<CardPresentation> = {},
): CardPresentation {
  const deckId = overrides.deckId ?? testEntityId();
  return {
    cardId: testEntityId(),
    noteId: testEntityId(),
    deckId,
    direction: "forward",
    frontDoc: paragraphDocument("Question"),
    backDoc: paragraphDocument("Answer"),
    frontText: "Question",
    backText: "Answer",
    assets: [],
    createdAt: 1,
    updatedAt: 1,
    noteRevision: 1,
    ...overrides,
  };
}

export function makeCardNote(
  overrides: Partial<Extract<CardNote, { cardType: "basic" }>> = {},
): CardNote {
  return {
    cardType: "basic",
    noteId: testEntityId(),
    deckId: testEntityId(),
    frontDoc: paragraphDocument("Question"),
    backDoc: paragraphDocument("Answer"),
    frontText: "Question",
    backText: "Answer",
    reverseEnabled: false,
    assets: [],
    tags: [],
    generatedCardCount: 1,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}
''', text, count=1)
if count != 1:
    raise SystemExit("factories: expected presentation/note factory pair")
text = text.replace('          card,\n          deckName:', '          presentation: card,\n          deckName:')
write(path, text)

# Practice UI consumes the presentation property. Limit the replacement to the
# practice feature and its tests so unrelated `card` domain variables are intact.
practice_paths = [SRC / "screens" / "PracticeScreen.tsx"]
practice_paths += list((SRC / "screens" / "practice").glob("*.ts*"))
practice_paths += list((SRC / "screens").glob("PracticeScreen*.test.tsx"))
for p in practice_paths:
    if not p.exists():
        continue
    text = p.read_text(encoding="utf-8")
    text = text.replace('.card.', '.presentation.')
    text = text.replace('.card)', '.presentation)')
    text = text.replace('.card,', '.presentation,')
    p.write_text(text, encoding="utf-8")

# Rust presentation naming. Keep internal field names stable while serializing
# explicit transport names.
for p in RUST.rglob("*.rs"):
    text = p.read_text(encoding="utf-8")
    text = re.sub(r'\bCardDto\b', 'CardPresentationDto', text)
    p.write_text(text, encoding="utf-8")

path = "src-tauri/src/cards.rs"
text = read(path)
text = text.replace('pub struct CardPresentationDto {\n    pub id: String,',
                    'pub struct CardPresentationDto {\n    #[serde(rename = "cardId")]\n    pub id: String,')
text = text.replace('    pub revision: i64,\n}\n\npub(crate) struct ValidatedContent',
                    '    #[serde(rename = "noteRevision")]\n    pub revision: i64,\n}\n\npub(crate) struct ValidatedContent')
write(path, text)

path = "src-tauri/src/practice.rs"
replace_once(path, '    pub card: CardPresentationDto,', '    pub presentation: CardPresentationDto,')

path = "src-tauri/src/projections/practice.rs"
text = read(path)
text = re.sub(r'(PracticeCardDto \{\s*)card,', r'\1presentation: card,', text)
write(path, text)

# Parent authoring module: remove the ambiguous DTO/hydrator and register the
# dedicated source-note module.
path = "src-tauri/src/authoring/mod.rs"
text = read(path)
text, count = re.subn(
    r'\n#\[derive\(Clone, Debug, Serialize\)\]\n#\[serde\(rename_all = "camelCase"\)\]\npub struct OrganizedCardDto \{[\s\S]*?\n\}\n',
    '\n', text, count=1)
if count != 1:
    raise SystemExit("authoring/mod.rs: expected OrganizedCardDto")
text, count = re.subn(
    r'\nfn hydrate_organized_card\([\s\S]*?\nfn load_tag_names\(',
    '\nfn load_tag_names(', text, count=1)
if count != 1:
    raise SystemExit("authoring/mod.rs: expected organized hydrator block")
text = text.replace(
    '    cards::{get_card_from_connection, validate_content, CardPresentationDto, ValidatedContent},',
    '    cards::{validate_content, ValidatedContent},')
text = text.replace(
    'pub(crate) mod card_commands;\npub(crate) mod card_types_and_templates;',
    'pub(crate) mod card_commands;\nmod card_note;\npub(crate) mod card_types_and_templates;')
text = text.replace(
    'pub(crate) use card_commands::backfill_duplicate_hashes_in_connection;',
    'pub(crate) use card_commands::backfill_duplicate_hashes_in_connection;\npub(crate) use card_note::CardNoteDto;\nuse card_note::hydrate_card_note;')
write(path, text)

# Public card-note read command.
path = "src-tauri/src/authoring/card_commands.rs"
text = read(path)
text = text.replace('pub fn get_organized_card(', 'pub fn get_card_note(')
text = text.replace(') -> Result<OrganizedCardDto, CommandError> {', ') -> Result<CardNoteDto, CommandError> {')
text = text.replace('hydrate_organized_card(connection, &paths, &note_id)', 'hydrate_card_note(connection, &paths, &note_id)')
write(path, text)

# Card-type authoring commands now expose semantic source-note names and return
# the complete CardNote response; the follow-up CardTypeInfo command is removed.
path = "src-tauri/src/authoring/card_types_and_templates.rs"
text = read(path)
text, count = re.subn(
    r'\n#\[derive\(Clone, Debug, Serialize\)\]\n#\[serde\(rename_all = "camelCase"\)\]\npub struct CardTypeInfoDto \{[\s\S]*?\n\}\n',
    '\n', text, count=1)
if count != 1:
    raise SystemExit("card_types: expected CardTypeInfoDto")
text, count = re.subn(
    r'\n#\[tauri::command\]\npub fn get_card_type_info\([\s\S]*?\n\}\n\n#\[tauri::command\]\npub fn create_generated_card',
    '\n#[tauri::command]\npub fn create_card_note', text, count=1)
if count != 1:
    raise SystemExit("card_types: expected get_card_type_info/create_generated_card boundary")
for old, new in {
    "CreateGeneratedCardInput": "CreateCardNoteInput",
    "UpdateGeneratedCardInput": "UpdateCardNoteInput",
    "create_generated_card_in_database": "create_card_note_in_database",
    "update_generated_card_in_database": "update_card_note_in_database",
    "update_generated_card": "update_card_note",
    "OrganizedCardDto": "CardNoteDto",
    "hydrate_organized_card": "hydrate_card_note",
}.items():
    text = re.sub(rf'\b{re.escape(old)}\b', new, text)
text, count = re.subn(
    r'\nfn load_card_type_info\([\s\S]*?\n\}\n\nenum CardNoteDraft',
    '\n\nenum CardNoteDraft', text, count=1)
if count != 1:
    raise SystemExit("card_types: expected load_card_type_info block")
write(path, text)

# Handler registry and durable obsolete-command policy.
path = "src-tauri/src/lib.rs"
text = read(path)
text = text.replace('authoring::card_commands::get_organized_card,', 'authoring::card_commands::get_card_note,')
text = text.replace('            authoring::card_types_and_templates::get_card_type_info,\n', '')
text = text.replace('authoring::card_types_and_templates::create_generated_card,', 'authoring::card_types_and_templates::create_card_note,')
text = text.replace('authoring::card_types_and_templates::update_generated_card,', 'authoring::card_types_and_templates::update_card_note,')
write(path, text)

# Rename command strings/references in Rust tests and documentation source where
# they describe the active bundled IPC surface.
for p in RUST.rglob("*.rs"):
    text = p.read_text(encoding="utf-8")
    text = re.sub(r'\bCreateGeneratedCardInput\b', 'CreateCardNoteInput', text)
    text = re.sub(r'\bUpdateGeneratedCardInput\b', 'UpdateCardNoteInput', text)
    text = re.sub(r'\bcreate_generated_card\b', 'create_card_note', text)
    text = re.sub(r'\bupdate_generated_card\b', 'update_card_note', text)
    text = re.sub(r'\bget_organized_card\b', 'get_card_note', text)
    p.write_text(text, encoding="utf-8")

# IPC verifier: direct invoke belongs only to typed domain gateways, and the old
# source-note/card-type commands are explicitly obsolete.
path = "scripts/verify-ipc-contract.mjs"
text = read(path)
needle = '''    const command = match[1];
    const locations = invoked.get(command) ?? [];
    locations.push(displayPath);
    invoked.set(command, locations);'''
replacement = '''    const command = match[1];
    if (!displayPath.startsWith("src/ipc/")) {
      failures.push(`Direct Tauri invoke is outside a typed IPC gateway: ${displayPath} (${command})`);
    }
    const locations = invoked.get(command) ?? [];
    locations.push(displayPath);
    invoked.set(command, locations);'''
if text.count(needle) != 1:
    raise SystemExit("verify-ipc-contract: expected invoke collection body")
text = text.replace(needle, replacement, 1)
text = text.replace(
    '  "start_practice",\n]) {',
    '  "start_practice",\n  "get_organized_card",\n  "get_card_type_info",\n  "create_generated_card",\n  "update_generated_card",\n]) {',
    1,
)
write(path, text)

# Retired monolithic/ad-hoc transport files are removed only after every import
# has been rewritten successfully.
for retired in [
    "src/ipc.ts",
    "src/analytics-ipc.ts",
    "src/generatedImport.ts",
    "src/diagnostics.ts",
]:
    target = ROOT / retired
    if not target.exists():
        raise SystemExit(f"missing expected retired IPC file: {retired}")
    target.unlink()

print("Applied Stage 3.12 domain IPC/source-note refactor.")
