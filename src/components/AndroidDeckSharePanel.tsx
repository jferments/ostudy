import { useEffect, useState } from "react";

import type { ShowToast } from "./Toast";
import { createOperationId, getErrorMessage } from "../ipc/common";
import { prepareDeckExport, type DeckSummary } from "../ipc/decks";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { shareExport } from "../platform/files";
import { Button } from "../ui/primitives/Button";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import ownershipStyles from "./AndroidDeckSharePanel.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface AndroidDeckSharePanelProps {
  decks: DeckSummary[];
  showToast: ShowToast;
}

/**
 * Android's Sharesheet is intentionally a separate action from Save/Export.
 * The ordinary DecksScreen remains platform-neutral; this compact capability
 * panel appears only when the native runtime advertises system sharing.
 */
export function AndroidDeckSharePanel({ decks, showToast }: AndroidDeckSharePanelProps) {
  const capabilities = usePlatformCapabilities();
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const [deckId, setDeckId] = useState(activeDecks[0]?.id ?? "");
  const [sharing, setSharing] = useState(false);
  const canShare = capabilities.native?.services.systemShare.state === "available";

  useEffect(() => {
    if (deckId && activeDecks.some((deck) => deck.id === deckId)) return;
    setDeckId(activeDecks[0]?.id ?? "");
  }, [activeDecks, deckId]);

  if (!canShare || activeDecks.length === 0) return null;

  async function shareSelectedDeck(): Promise<void> {
    if (!deckId || sharing) return;
    const deck = activeDecks.find((candidate) => candidate.id === deckId);
    if (!deck) return;
    setSharing(true);
    try {
      const prepared = await prepareDeckExport(deck.id, createOperationId());
      const launched = await shareExport(prepared.preparedExport.id);
      if (!launched.launched) return;
      showToast(
        "info",
        `Opened system sharing for “${deck.name}” with ${prepared.cardCount} ${
          prepared.cardCount === 1 ? "card" : "cards"
        }.`,
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setSharing(false);
    }
  }

  return (
    <section className={ownedClassNames(ownedStyleMaps, "deck-share-panel")} aria-labelledby="android-deck-share-heading">
      <div>
        <span className={ownedClassNames(ownedStyleMaps, "eyebrow")}>ANDROID SHARING</span>
        <h2 id="android-deck-share-heading">Share a deck</h2>
        <p>Send a portable .studydeck file through Android's system Sharesheet.</p>
      </div>
      <div className={ownedClassNames(ownedStyleMaps, "header-actions")}>
        <label>
          <span className="sr-only">Deck to share</span>
          <select
            aria-label="Deck to share"
            value={deckId}
            disabled={sharing}
            onChange={(event) => setDeckId(event.currentTarget.value)}
          >
            {activeDecks.map((deck) => (
              <option key={deck.id} value={deck.id}>
                {deck.name}
              </option>
            ))}
          </select>
        </label>
        <Button
          type="button"
          variant="secondary"
          disabled={!deckId || sharing}
          onClick={() => void shareSelectedDeck()}
        >
          {sharing ? "Preparing…" : "Share deck"}
        </Button>
      </div>
    </section>
  );
}
