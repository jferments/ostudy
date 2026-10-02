#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [recipes, buttonSource, viewCss, folderCss, deckCss, feedbackCss, rootSource, deckSource, folderSource, reorderSource, tauriConfigText] = await Promise.all([
  readFile(join(root, "src/screens/flashcards/deck-library/libraryPresentation.ts"), "utf8"),
  readFile(join(root, "src/ui/vendor/shadcn/button.tsx"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/DeckLibraryView.module.css"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/FolderSection.module.css"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/DeckItem.module.css"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/LibraryDragFeedback.module.css"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/DeckLibraryRoot.tsx"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/DeckItem.tsx"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/FolderSection.tsx"), "utf8"),
  readFile(join(root, "src/screens/flashcards/deck-library/useDeckLibraryReorder.ts"), "utf8"),
  readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
]);
const css = `${viewCss}\n${folderCss}\n${deckCss}\n${feedbackCss}`;
const movementSource = `${deckSource}\n${folderSource}\n${reorderSource}`;
const failures = [];

// Verify the live Tailwind recipes; interaction-specific CSS still owns drag feedback.
expectRecipe("root", ["tw:@container/deck-library"], "deck-library container");
expectRecipe("toolbar", ["tw:flex", "tw:justify-end"], "deck toolbar");
expectRecipe("toolbarActions", ["tw:flex", "tw:w-auto", "tw:justify-end", "tw:[&>button]:flex-none", "tw:[&>button]:whitespace-nowrap"], "intrinsic toolbar actions at every width");
expectContains(rootSource, "Import deck", "labeled import action");
expectAbsent(rootSource, 'label="Import deck from file"', "retired icon-only import action");
expectRecipe("sectionActions", ["tw:flex", "tw:justify-end", "tw:@max-[620px]/deck-library:w-full", "tw:@max-[620px]/deck-library:justify-stretch"], "content-width compact folder controls");
expectContains(rootSource, 'libraryPresentation as styles', "live library recipe consumer");
expectContains(folderSource, '<IconButton', "shared folder reorder control");
expectContains(folderSource, 'icon="move"', "keyboard reorder affordance");
expectContains(buttonSource, 'icon: "tw:size-8 tw:p-0"', "compact fine-pointer icon size");
expectContains(buttonSource, "tw:coarse:min-h-touch", "coarse-pointer minimum action height");
expectContains(buttonSource, "tw:coarse:min-w-touch", "coarse-pointer minimum action width");
expectContains(deckSource, 'data-library-row={archived ? undefined : "deck"}', "active deck pointer row identity");
expectContains(folderSource, 'data-library-folder-surface="true"', "complete folder pointer destination");
expectContains(deckSource, "reorder.beginPointerDrag", "deck row pointer session initiation");
expectContains(folderSource, "reorder.beginPointerDrag", "folder surface pointer session initiation");
expectContains(css, '.preview[data-status="valid"]', "valid drag feedback");
expectContains(css, '.preview[data-status="noop"]', "no-op drag feedback");
expectContains(css, '.preview[data-status="invalid"]', "invalid drag feedback");
expectAbsent(css, ".density", "removed deck-density selector");
expectAbsent(css, ".dragHandle", "obsolete drag-handle selector");
for (const nativeDragFragment of [
  "DragEvent",
  "dataTransfer",
  "onDragStart",
  "onDragOver",
  "onDrop",
  "draggable=",
  "html-drag",
  "application/x-ostudy",
]) {
  expectAbsent(
    movementSource,
    nativeDragFragment,
    `native HTML movement fragment ${nativeDragFragment}`,
  );
}

const tauriConfig = JSON.parse(tauriConfigText);
const mainWindow = tauriConfig?.app?.windows?.find((window) => window?.label === "main");
if (!mainWindow || typeof mainWindow.width !== "number" || mainWindow.width < 1280) {
  failures.push("The default desktop main window must be at least 1280 CSS pixels wide.");
}

if (failures.length > 0) {
  console.error("Deck library layout contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Deck layout, intrinsic compact toolbar actions, labeled import affordance, row pointer surfaces, semantic drag feedback, keyboard reorder controls, and coarse-pointer targets are consistent.",
);

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectAbsent(source, forbidden, label) {
  if (source.includes(forbidden)) failures.push(`Unexpected ${label}: ${forbidden}`);
}

function expectRecipe(name, utilities, label) {
  const marker = `${name}: "`;
  const start = recipes.indexOf(marker);
  if (start < 0) { failures.push(`Missing ${label} recipe ${name}`); return; }
  const body = recipes.slice(start + marker.length).split('"', 1)[0].split(/\s+/u);
  for (const utility of utilities) if (!body.includes(utility)) failures.push(`Missing ${label}: ${utility}`);
}
