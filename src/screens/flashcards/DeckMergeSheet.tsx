import { useMemo, useState } from "react";

import type { DeckSummary } from "../../ipc/decks";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

interface DeckMergeSheetProps {
  source: DeckSummary;
  decks: readonly DeckSummary[];
  busy: boolean;
  onClose(): void;
  onMerge(destinationDeckId: string): Promise<void>;
}

/**
 * One open sheet owns one native operation identity in its caller. Once the
 * first request starts, destination choice is frozen so an ambiguous response
 * can only be retried with the exact same request.
 */
export function DeckMergeSheet({
  source,
  decks,
  busy,
  onClose,
  onMerge,
}: DeckMergeSheetProps) {
  const destinations = useMemo(
    () => decks.filter((deck) => deck.id !== source.id && deck.archivedAt === null),
    [decks, source.id],
  );
  const [destinationDeckId, setDestinationDeckId] = useState(destinations[0]?.id ?? "");
  const [attempted, setAttempted] = useState(false);
  const destination = destinations.find((deck) => deck.id === destinationDeckId) ?? null;

  return (
    <Sheet
      title={"Merge “" + source.name + "”"}
      description="Move this deck’s flashcards into another active deck without resetting their study history."
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className="tw:grid tw:gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (!destinationDeckId) return;
          setAttempted(true);
          void onMerge(destinationDeckId);
        }}
      >
        <label className="tw:grid tw:gap-2">
          <span>Destination deck</span>
          <NativeSelect
            data-autofocus="true"
            value={destinationDeckId}
            disabled={busy || attempted}
            onChange={(event) => setDestinationDeckId(event.currentTarget.value)}
          >
            {destinations.length === 0 ? <option value="">No other active deck</option> : null}
            {destinations.map((deck) => (
              <option key={deck.id} value={deck.id}>
                {deck.name} · {deck.generatedCardCount} cards
              </option>
            ))}
          </NativeSelect>
        </label>

        <InlineNotice kind="warning">
          {source.generatedCardCount} {source.generatedCardCount === 1 ? "card" : "cards"} will move
          {destination ? " into “" + destination.name + "”" : ""}. Card identities, schedules,
          review history, tags, media, and source citations stay attached. “{source.name}” is
          archived after the merge. Existing cards are not deduplicated.
        </InlineNotice>

        {attempted && !busy ? (
          <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">
            If the previous response was interrupted, Retry merge reuses the same operation
            identity and destination instead of moving cards twice.
          </p>
        ) : null}

        <div className="tw:flex tw:justify-end tw:gap-2">
          <Button type="button" disabled={busy} onClick={onClose}>Cancel</Button>
          <Button
            type="submit"
            variant="primary"
            loading={busy}
            disabled={busy || destinationDeckId === ""}
          >
            {attempted && !busy ? "Retry merge" : "Merge decks"}
          </Button>
        </div>
      </form>
    </Sheet>
  );
}
