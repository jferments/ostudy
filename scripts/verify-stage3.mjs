import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const readJoined = async (...paths) =>
  (await Promise.all(paths.map((parts) => read(...parts)))).join("\n");

const [
  migration6, migration7, database, rustCommands, authoringEntry, authoringTypes,
  authoringGenerated, authoringTemplates, authoringAutomaticLayout,
  authoringNormalization, practiceSelection, historicalAttribution,
  templateProjections, ipc, app, editor, extensions, clozeEditor, templateEntry,
  templateLayoutEditor, templatesScreen, cardsScreen, cardStyles, richStyles,
  stage3Transfer, architecture, dataModel,
] = await Promise.all([
  read("src-tauri", "migrations", "0006_card_types_and_templates.sql"),
  read("src-tauri", "migrations", "0007_rich_templates_single_cloze.sql"),
  read("src-tauri", "src", "database.rs"), read("src-tauri", "src", "lib.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "types_commands.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "generated_cards.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "templates.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "auto_template_layout.rs"),
  read("src-tauri", "src", "authoring", "stage3_v2", "normalization.rs"),
  read("src-tauri", "src", "practice_selection.rs"),
  read("src-tauri", "src", "historical_attribution.rs"),
  read("src-tauri", "src", "template_projections.rs"), read("src", "ipc.ts"),
  read("src", "App.tsx"), read("src", "screens", "CardEditorScreen.tsx"),
  read("src", "editor", "extensions.ts"),
  read("src", "screens", "card-editor", "ClozeEditor.tsx"),
  read("src", "screens", "card-editor", "TemplateEntryForm.tsx"),
  read("src", "screens", "templates", "TemplateLayoutEditor.tsx"),
  read("src", "screens", "TemplatesScreen.tsx"),
  read("src", "screens", "CardsScreen.tsx"), read("src", "card-types.css"),
  read("src", "rich-templates.css"),
  readJoined(["src-tauri", "src", "studydeck.rs"],
    ["src-tauri", "src", "studydeck", "export.rs"],
    ["src-tauri", "src", "studydeck", "import.rs"],
    ["src-tauri", "src", "studydeck", "read.rs"]),
  read("docs", "ARCHITECTURE.md"), read("docs", "CARD-FORMAT-AND-DATA-MODEL.md"),
]);
const authoring=[authoringEntry,authoringTypes,authoringGenerated,authoringTemplates,authoringAutomaticLayout,authoringNormalization].join("\n");
const failures=[];
for(const [version,migration] of [[6,migration6],[7,migration7]]){const checksum=createHash("sha256").update(migration).digest("hex");if(!database.includes(`checksum: "${checksum}"`))failures.push(`database.rs checksum mismatch for migration 000${version}: ${checksum}`)}
const schemaVersion=Number(database.match(/LATEST_SCHEMA_VERSION: i64 = (\d+)/u)?.[1]);if(!Number.isInteger(schemaVersion)||schemaVersion<7)failures.push("database schema version is older than completed Stage 3");
for(const fragment of ["ALTER TABLE notes ADD COLUMN reverse_enabled","ALTER TABLE notes ADD COLUMN cloze_source","CREATE TABLE card_templates"]){if(!migration6.includes(fragment))failures.push(`migration 0006 missing: ${fragment}`)}
for(const fragment of ["presentation_variant","front_doc TEXT","back_doc TEXT","CREATE TABLE card_template_assets"]){if(!migration7.includes(fragment))failures.push(`migration 0007 missing: ${fragment}`)}
if(/template_id\s+TEXT[^\n]*REFERENCES[^\n]*notes/iu.test(migration6+migration7))failures.push("generated notes incorrectly depend on reusable templates");
for(const command of ["get_card_type_info","create_generated_card","update_generated_card","list_card_template_summaries","get_card_template","create_card_template","update_card_template","delete_card_template","preview_template_cards","generate_cards_from_template"]){if(!rustCommands.includes(command))failures.push(`Rust command not registered: ${command}`);if(!ipc.includes(`"${command}"`))failures.push(`frontend command missing: ${command}`)}
for(const fragment of ["apply_stage3_data_repairs","normalize_cloze_card_rows","cards: vec![GeneratedCardSpec",'direction: "forward"',"presentation_variant_for_projected","render_stored_card_presentation_for_variant","templateFieldInline","templateFieldBlock","card_template_assets","template_revision","insert_organized_card"]){if(!authoring.includes(fragment))failures.push(`revised Stage 3 domain behavior missing: ${fragment}`)}
for(const fragment of ["normalize_typed_template_layout","TemplateValuePlacement","materialize_automatic_template_document","materialize_template_text_block","split_by_block_value","template_document_is_inline","normalize_explicit_template_placeholders"]){if(!authoring.includes(fragment))failures.push(`automatic template layout missing: ${fragment}`)}
for(const forbidden of ["eval(","new Function","Command::new","std::process","reqwest"]){if(authoring.includes(forbidden))failures.push(`template implementation contains executable behavior: ${forbidden}`)}
if(authoringGenerated.includes('direction: "cloze"'))failures.push("active cloze creation still generates sibling card identities");
for(const fragment of ["presentation_variant_for_projected","prepare_attributions","practice_session_card_snapshots","get_practice_card_window"]){if(!(practiceSelection+historicalAttribution).includes(fragment))failures.push(`practice snapshot behavior missing: ${fragment}`)}
if(/c\.direction\s*=\s*'forward'/u.test(practiceSelection))failures.push("practice still filters out reverse cards");
if(!templateProjections.includes("CardTemplateSummaryDto")||!templateProjections.includes("list_card_template_summaries"))failures.push("template library is not a bounded summary projection");
for(const fragment of ["frontDoc: RichDocument","backDoc: RichDocument","doc: RichDocument","templateRevision","questionDoc","answerDoc"]){if(!ipc.includes(fragment))failures.push(`rich template IPC missing: ${fragment}`)}
for(const fragment of ["templateFieldInline","templateFieldBlock"]){if(!extensions.includes(fragment))failures.push(`template compatibility extension missing: ${fragment}`)}
for(const fragment of ["{{Subject}}","Simple one-paragraph values stay inline","<RichTextEditor"]){if(!templateLayoutEditor.includes(fragment))failures.push(`automatic template layout editor missing: ${fragment}`)}
for(const forbidden of ["Insert inline","Insert as block","Insert field"]){if(templateLayoutEditor.includes(forbidden))failures.push(`manual template placement UI remains: ${forbidden}`)}
if(!templateEntry.includes("<RichTextEditor")||!templateEntry.includes("<RichTextView"))failures.push("template entry does not use rich editors and rich previews");
for(const fragment of ["Field description","Question layout","Answer layout","TemplateLayoutEditor","renderer keeps simple values inline"]){if(!templatesScreen.includes(fragment))failures.push(`Templates screen missing: ${fragment}`)}
for(const forbidden of ["Input guidance","Include the therapeutic class","Templates are safe authoring tools, not card containers","Insert short fields inline"]){if(templatesScreen.includes(forbidden))failures.push(`Templates screen retains obsolete copy: ${forbidden}`)}
for(const fragment of ["templateRevision","TemplateEntryForm","generateCardsFromTemplate"]){if(!editor.includes(fragment))failures.push(`card editor missing rich template behavior: ${fragment}`)}
for(const fragment of ["1 card ·","Hide one blank per review","rotates through the marked blanks"]){if(!clozeEditor.includes(fragment))failures.push(`single-card cloze copy missing: ${fragment}`)}
if(clozeEditor.includes("ACE inhibitors"))failures.push("cloze editor retains pharmacology-specific example copy");
for(const fragment of ["selectedCards.has(card.cardId)","setPracticeCards(selected)","card.cardLabel"]){if(!cardsScreen.includes(fragment))failures.push(`Search/Filter card identity behavior missing: ${fragment}`)}
for(const fragment of [".cloze-editor",".template-rule-card",".card-type-badge"]){if(!cardStyles.includes(fragment))failures.push(`Stage 3 base styles missing: ${fragment}`)}
if(!richStyles.includes(".template-field-placeholder"))failures.push("rich template placeholder styles are missing");if(richStyles.includes(".template-field-inserter"))failures.push("manual template field inserter styles remain");
for(const fragment of ["CURRENT_PACKAGE_VERSION","fresh-local-schedules","insert_new_schedule"]){if(!stage3Transfer.includes(fragment))failures.push(`studydeck support missing: ${fragment}`)}if(stage3Transfer.includes("CREATE TABLE card_templates"))failures.push("reusable authoring templates must not be exported with decks");
for(const fragment of ['label="Templates"',"<TemplatesScreen","hasUnsavedTemplate"]){if(!app.includes(fragment))failures.push(`Templates navigation missing: ${fragment}`)}
for(const [name,document,fragments] of [["architecture",architecture,["template","cloze"]],["data model",dataModel,["template","cloze"]]])for(const fragment of fragments)if(!document.toLowerCase().includes(fragment))failures.push(`${name} documentation missing: ${fragment}`);
if(failures.length){console.error("Stage 3 verification failed:");for(const failure of failures)console.error(`  - ${failure}`);process.exit(1)}
console.log("OK   Stage 3 rich templates use bounded summaries, cloze uses one schedule, and immutable practice snapshots preserve rendered presentations.");
