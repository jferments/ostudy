#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");
function replaceExact(text, before, after, label) {
  if (!text.includes(before)) throw new Error(`${label}: expected source not found`);
  return text.replace(before, after);
}

let decks = await read("src/screens/DecksScreen.tsx");
decks = replaceExact(
  decks,
  `import { publishExport, selectExternalImport } from "../platform/files";\n`,
  `import { publishExport, selectExternalImport, shareExport } from "../platform/files";\nimport { usePlatformCapabilities } from "../platform/PlatformProvider";\n`,
  "import deck sharing support",
);
decks = replaceExact(
  decks,
  `  const confirm = useConfirm();\n  const compact = useMediaQuery("(max-width: 760px)");`,
  `  const confirm = useConfirm();\n  const capabilities = usePlatformCapabilities();\n  const canShare = capabilities.native?.services.systemShare.state === "available";\n  const compact = useMediaQuery("(max-width: 760px)");`,
  "read system share capability",
);
const shareHandler = `
  async function handleShare(deck: DeckSummary): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const prepared = await controller.prepareExport(deck.id, createOperationId());
      const result = await shareExport(prepared.preparedExport.id);
      if (result.launched) {
        showToast(
          "info",
          `Opened Android sharing for ${prepared.cardCount} ${
            prepared.cardCount === 1 ? "card" : "cards"
          }.`,
        );
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

`;
decks = replaceExact(
  decks,
  `  async function showImportedDeck(`,
  `${shareHandler}  async function showImportedDeck(`,
  "add deck share handler",
);
const exportButton = `                      <button
                        type="button"
                        disabled={isTransferring}
                        onClick={() => void handleExport(deck)}
                      >
                        Export
                      </button>`;
const exportShareButtons = `${exportButton}
                      {canShare ? (
                        <button
                          type="button"
                          disabled={isTransferring}
                          onClick={() => void handleShare(deck)}
                        >
                          Share
                        </button>
                      ) : null}`;
const libraryMatches = decks.split(exportButton).length - 1;
if (libraryMatches !== 2) {
  throw new Error(`deck library export buttons: expected 2 matches, found ${libraryMatches}`);
}
decks = decks.split(exportButton).join(exportShareButtons);
const detailButton = `              <button
                type="button"
                className="secondary-button"
                disabled={isTransferring}
                onClick={() => void handleExport(selectedDeck)}
              >
                Export
              </button>`;
decks = replaceExact(
  decks,
  detailButton,
  `${detailButton}
              {canShare ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={isTransferring}
                  onClick={() => void handleShare(selectedDeck)}
                >
                  Share
                </button>
              ) : null}`,
  "add detail share button",
);
await write("src/screens/DecksScreen.tsx", decks);

let storage = await read("src/screens/settings/StorageBackupSettings.tsx");
storage = replaceExact(
  storage,
  `import { publishExport, selectExternalImport } from "../../platform/files";\n`,
  `import { publishExport, selectExternalImport, shareExport } from "../../platform/files";\nimport { usePlatformCapabilities } from "../../platform/PlatformProvider";\n`,
  "import backup sharing support",
);
storage = replaceExact(
  storage,
  `  const confirm = useConfirm();\n`,
  `  const confirm = useConfirm();\n  const capabilities = usePlatformCapabilities();\n  const canShare = capabilities.native?.services.systemShare.state === "available";\n`,
  "read backup share capability",
);
const backupShare = `
  async function shareLibraryBackup(): Promise<void> {
    if (isTransferringLibrary) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const prepared = await prepareLibraryBackup(createOperationId());
      const result = await shareExport(prepared.preparedExport.id);
      if (result.launched) {
        showToast(
          "info",
          `Opened Android sharing for a backup containing ${prepared.deckCount} ${
            prepared.deckCount === 1 ? "deck" : "decks"
          }.`,
        );
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

`;
storage = replaceExact(
  storage,
  `  async function restoreLibrary(): Promise<void> {`,
  `${backupShare}  async function restoreLibrary(): Promise<void> {`,
  "add backup share handler",
);
const backupButton = `          <button
            type="button"
            className="secondary-button"
            disabled={isTransferringLibrary}
            onClick={() => void backupLibrary()}
          >
            {isTransferringLibrary ? "Working…" : "Back up library"}
          </button>`;
storage = replaceExact(
  storage,
  backupButton,
  `${backupButton}
          {canShare ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isTransferringLibrary}
              onClick={() => void shareLibraryBackup()}
            >
              Share backup
            </button>
          ) : null}`,
  "add backup share button",
);
await write("src/screens/settings/StorageBackupSettings.tsx", storage);

console.log("Applied Android save/share actions to deck and backup workflows.");
