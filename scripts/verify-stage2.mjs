import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const readJoined = async (...paths) =>
  (await Promise.all(paths.map((parts) => read(...parts)))).join("\n");

async function readTree(relativeParts, suffix) {
  const directory = join(root, ...relativeParts);
  const entries = await readdir(directory, { withFileTypes: true });
  const contents = [];
  for (const entry of entries) {
    const childParts = [...relativeParts, entry.name];
    if (entry.isDirectory()) contents.push(await readTree(childParts, suffix));
    else if (entry.isFile() && entry.name.endsWith(suffix)) contents.push(await read(...childParts));
  }
  return contents.join("\n");
}

const [migration4,migration5,database,rustCommands,authoring,cardsCore,practice,
  practiceSelection,ipc,app,cardsScreen,editor,bulkEntry,cardStyles,studydeckTransfer,
  rustSourceTree,workflowEntries] = await Promise.all([
  read("src-tauri","migrations","0004_card_organization.sql"),
  read("src-tauri","migrations","0005_remove_legacy_duplicate_trigger.sql"),
  read("src-tauri","src","database.rs"), read("src-tauri","src","lib.rs"),
  readJoined(["src-tauri","src","authoring.rs"],["src-tauri","src","deletion_commands.rs"],
    ["src-tauri","src","authoring","types.rs"],["src-tauri","src","authoring","commands.rs"],
    ["src-tauri","src","authoring","search_and_tags.rs"],
    ["src-tauri","src","authoring","writes_and_transfer.rs"],
    ["src-tauri","src","authoring","stage3","transfer.rs"],
    ["src-tauri","src","authoring","normalization.rs"],
    ["src-tauri","src","authoring","tests.rs"]),
  read("src-tauri","src","cards.rs"), read("src-tauri","src","practice.rs"),
  read("src-tauri","src","practice_selection.rs"), read("src","ipc.ts"),
  read("src","App.tsx"), read("src","screens","CardsScreen.tsx"),
  read("src","screens","CardEditorScreen.tsx"),
  read("src","screens","cards","BulkEntryModal.tsx"),
  readJoined(["src","card-organization.css"],["src","stage36-38.css"]),
  readJoined(["src-tauri","src","studydeck.rs"],["src-tauri","src","studydeck","export.rs"],
    ["src-tauri","src","studydeck","import.rs"],["src-tauri","src","studydeck","read.rs"]),
  readTree(["src-tauri","src"],".rs"), readdir(join(root,".github","workflows")),
]);
const failures=[];
for(const [version,migration] of [[4,migration4],[5,migration5]]){
  const checksum=createHash("sha256").update(migration).digest("hex");
  if(!database.includes(`checksum: "${checksum}"`)) failures.push(`database.rs checksum mismatch for migration 000${version}: ${checksum}`);
}
const schemaVersion=Number(database.match(/LATEST_SCHEMA_VERSION: i64 = (\d+)/u)?.[1]);
if(!Number.isInteger(schemaVersion)||schemaVersion<5) failures.push("database schema version is older than completed Stage 2 schema");
for(const fragment of ["CREATE TABLE tags","CREATE TABLE note_tags","CREATE VIRTUAL TABLE card_search_fts USING fts5","duplicate_hash"])
  if(!migration4.includes(fragment)) failures.push(`migration 0004 missing: ${fragment}`);
if(!migration4.includes("notes_duplicate_hash_after_legacy_update")) failures.push("migration 0004 was rewritten instead of preserved");
if(!migration5.includes("DROP TRIGGER IF EXISTS notes_duplicate_hash_after_legacy_update")) failures.push("migration 0005 does not remove the obsolete trigger");
for(const forbidden of ["repair_obsolete_card_organization_trigger","CARD_ORGANIZATION_WITH_OBSOLETE_TRIGGER_CHECKSUM","UPDATE schema_migrations\n             SET checksum"])
  if(database.includes(forbidden)) failures.push(`database contains migration-history shim: ${forbidden}`);
if(!database.includes("schema_four_upgrades_forward_without_rewriting_history_or_losing_cards")) failures.push("schema 4 forward migration lacks preservation test");

for(const command of ["list_tags","search_cards","list_matching_cards","get_organized_card","create_organized_card","update_organized_card","find_duplicate_cards","analyze_bulk_cards","bulk_create_cards","transfer_cards","bulk_tag_cards","delete_cards"]){
  if(!rustCommands.includes(command)) failures.push(`Rust command not registered: ${command}`);
  if(!ipc.includes(`"${command}"`)) failures.push(`frontend command missing: ${command}`);
}
if(!rustCommands.includes("practice_selection::start_card_practice")||!ipc.includes("startCardPractice")||!ipc.includes("start_card_practice")) failures.push("filtered cards cannot start temporary practice");
for(const fragment of ["bm25(card_search_fts)","reviewed_filter","availability_filter","deck_ids","MAX_SEARCH_SELECTION","MAX_BULK_ROWS","TransferMode::Move","TransferMode::Copy","TagChangeMode::Add","backfill_duplicate_hashes"])
  if(!authoring.includes(fragment)) failures.push(`authoring boundary missing: ${fragment}`);
for(const inputName of ["ListTagsInput","SearchCardsInput","CreateOrganizedCardInput","UpdateOrganizedCardInput","DuplicateCheckInput","BulkCardRowInput","AnalyzeBulkCardsInput","BulkCreateCardsInput","TransferCardsInput","BulkTagCardsInput"]){
  const strict=new RegExp(`#\\[serde\\(rename_all = "camelCase", deny_unknown_fields\\)\\]\\s*pub struct ${inputName}\\b`,`u`);
  if(!strict.test(authoring)) failures.push(`${inputName} must reject unknown IPC fields`);
}
const bulk=authoring.match(/pub struct BulkCreateCardsInput\s*\{(?<body>[\s\S]*?)\n\}/u)?.groups?.body??"";
if(bulk.includes("skip_duplicates")) failures.push("BulkCreateCardsInput retains removed skipDuplicates");
if(authoring.includes("legacy_deck_id")||/pub deck_id:\s*Option<String>/u.test(authoring)) failures.push("authoring retains removed single-deck input");
if(!authoring.includes("removed_authoring_fields_are_rejected")) failures.push("removed authoring fields lack strict tests");
for(const removed of ["cards::get_card,","cards::create_card,","cards::update_card,","practice::start_practice,"])
  if(rustCommands.includes(removed)) failures.push(`obsolete Rust command remains: ${removed}`);
for(const removed of ["export interface CreateCardInput","export interface UpdateCardInput","export async function getCard(","export async function createCard(","export async function updateCard(","export async function startPractice("])
  if(ipc.includes(removed)) failures.push(`obsolete frontend IPC remains: ${removed}`);
for(const removed of ["pub fn get_card(","pub fn create_card(","pub fn update_card(","create_card_in_connection(","update_card_in_connection("])
  if(cardsCore.includes(removed)) failures.push(`obsolete card API remains: ${removed}`);
if(practice.includes("pub fn start_practice(")||practice.includes("start_practice_in_connection(")) failures.push("obsolete strict-due practice remains");
const suppressions=rustSourceTree.match(/#\[(?:allow|expect)\s*\(\s*(?:dead_code|unused|unused_imports|unused_variables)\s*\)\]/gu)??[];
if(suppressions.length) failures.push(`Rust source suppresses stale diagnostics: ${[...new Set(suppressions)].join(", ")}`);
const oneOff=workflowEntries.filter(name=>name.startsWith("one-off-"));if(oneOff.length) failures.push(`one-off workflows committed: ${oneOff.join(", ")}`);
if(!practiceSelection.includes("start_card_practice")||!practiceSelection.includes("card_ids")) failures.push("filtered cards cannot start a temporary session");
if(!app.includes('label="Search/Filter"')||!app.includes("onPracticeCards")) failures.push("Search/Filter is not registered end to end");
for(const fragment of ["Search / Filter","availabilityFilter","reviewedFilter","Select visible","Select all (","Actions for selected cards","Practice selected","PracticeResultsModal","BulkTagModal","DeleteCardsModal"])
  if(!cardsScreen.includes(fragment)) failures.push(`bounded Search/Filter screen missing: ${fragment}`);
for(const fragment of ["<TagInput","findDuplicateCards","Save duplicate anyway","deckId: selectedDeckId",'aria-label="Deck"'])
  if(!editor.includes(fragment)) failures.push(`card editor missing: ${fragment}`);
if(!bulkEntry.includes("analyzeBulkCards")||bulkEntry.includes("skipDuplicates")) failures.push("bulk entry must analyze duplicates without policy checkbox");
for(const fragment of [".cards-screen",".card-search-layout",".selection-action-bar"])
  if(!cardStyles.includes(fragment)) failures.push(`card organization styles missing: ${fragment}`);
for(const fragment of ["CURRENT_PACKAGE_VERSION","fresh-local-schedules","insert_new_schedule"])
  if(!studydeckTransfer.includes(fragment)) failures.push(`studydeck support missing: ${fragment}`);
if(failures.length){console.error("Stage 2 verification failed:");for(const failure of failures)console.error(`  - ${failure}`);process.exit(1)}
console.log("OK   Search/Filter uses bounded availability projections, exact generated-card identities, strict inputs, and no stale IPC surfaces.");
