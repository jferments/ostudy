import { useCallback, useEffect, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import {
  dismissSubjectFilingReview,
  getSubjectFilingReviews,
  type SubjectFilingEntityKind,
  type SubjectFilingReview,
} from "../../ipc/subjects";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Panel } from "../../ui/primitives/Panel";

type ReviewList =
  | { status: "loading" }
  | { status: "ready"; reviews: SubjectFilingReview[] }
  | { status: "error"; message: string };

const GROUPS: ReadonlyArray<{ kind: SubjectFilingEntityKind; title: string }> = [
  { kind: "subject", title: "Subjects" },
  { kind: "deck", title: "Decks" },
  { kind: "document", title: "Documents" },
  { kind: "note", title: "Cards" },
];

/** Why each entry is listed, in the learner's words. */
const REASONS: Readonly<Record<string, string>> = {
  same_name_not_merged:
    "A deck folder and a document collection had this name, so they became separate subjects.",
  bundle_of_several_collections:
    "This study deck covered several collections and is filed in one of their subjects.",
  moved_to_collection_subject:
    "This study deck moved from its folder's subject to its collection's subject.",
  documents_in_several_subjects:
    "This deck's documents are in different subjects. It is filed in one of them.",
  moved_to_document_subject:
    "This deck moved from its folder's subject to the subject of its documents.",
  in_several_collections:
    "This document was in several collections. It stayed in the first one it joined.",
  followed_generated_deck:
    "This document was in no collection, so it joined the subject of its deck's folder.",
  copy_edited: "This collection copy of a card was edited, so it stayed a card of its own.",
  original_missing: "This collection copy's original card is gone, so the copy stayed.",
  original_archived: "This collection copy's original is in an archived deck, so the copy stayed.",
  card_mismatch: "This collection copy did not match its original card, so it stayed.",
  ambiguous_origin: "This collection copy came from more than one card, so it stayed.",
  sync_filing_conflict:
    "Two devices filed this differently, or one deleted its subject. This device kept one place.",
  sync_edited_retired_copy:
    "Another device edited this copy after it was merged into its original here.",
};

interface CheckTheseSheetProps {
  onClose: () => void;
  /** A dismissal changed the list; Home rereads its count. */
  onChanged: () => void;
}

/**
 * The Check these list: places where the upgrade to subjects or a sync had to
 * choose. Each entry explains the choice and can be dismissed once it looks
 * right. The list stays on this device.
 */
export function CheckTheseSheet({ onClose, onChanged }: CheckTheseSheetProps) {
  const lane = useRequestLane();
  const [list, setList] = useState<ReviewList>({ status: "loading" });
  const [dismissing, setDismissing] = useState<string | null>(null);
  const [dismissError, setDismissError] = useState<string | null>(null);

  const load = useCallback(() => {
    const token = lane.begin();
    setList({ status: "loading" });
    void getSubjectFilingReviews().then(
      (reviews) => {
        if (lane.isCurrent(token)) setList({ status: "ready", reviews });
      },
      (error: unknown) => {
        if (lane.isCurrent(token)) setList({ status: "error", message: getErrorMessage(error) });
      },
    );
  }, [lane]);

  useEffect(() => load(), [load]);

  async function dismiss(review: SubjectFilingReview): Promise<void> {
    if (dismissing !== null) return;
    const key = reviewKey(review);
    setDismissing(key);
    setDismissError(null);
    try {
      const reviews = await dismissSubjectFilingReview({
        entityKind: review.entityKind,
        entityId: review.entityId,
      });
      // The dismissal's answer is newer than any read still in flight.
      lane.invalidate();
      setList({ status: "ready", reviews });
      onChanged();
    } catch (error: unknown) {
      setDismissError(getErrorMessage(error));
    } finally {
      setDismissing(null);
    }
  }

  return (
    <Sheet
      title="Check these"
      description="When subjects arrived, or when a sync met two choices, these had more than one sensible place. Each was given one. Dismiss an entry once it looks right."
      width="medium"
      onClose={onClose}
      actions={<Button onClick={onClose}>Done</Button>}
    >
      <div className="tw:grid tw:min-w-0 tw:gap-5" aria-busy={list.status === "loading" || undefined}>
        {list.status === "loading" ? <InlineProgress>Loading the list…</InlineProgress> : null}
        {list.status === "error" ? (
          <InlineNotice kind="error">
            The list is unavailable: {list.message}{" "}
            <Button size="small" variant="text" onClick={load}>
              Retry
            </Button>
          </InlineNotice>
        ) : null}
        {dismissError ? <InlineNotice kind="error">{dismissError}</InlineNotice> : null}
        {list.status === "ready" && list.reviews.length === 0 ? (
          <p className="tw:m-0 tw:text-body tw:text-muted-foreground">Nothing left to check.</p>
        ) : null}
        {list.status === "ready"
          ? GROUPS.map(({ kind, title }) => {
              const reviews = list.reviews.filter((review) => review.entityKind === kind);
              if (reviews.length === 0) return null;
              return (
                <Panel key={kind} title={title}>
                  <ul className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
                    {reviews.map((review) => (
                      <li
                        key={reviewKey(review)}
                        className="tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-x-3 tw:gap-y-1"
                      >
                        <span className="tw:grid tw:min-w-0 tw:flex-[1_1_16rem] tw:gap-0.5">
                          <strong className="tw:font-medium tw:break-words">{review.label}</strong>
                          <span className="tw:text-small tw:text-muted-foreground">
                            {reasonText(review)}
                          </span>
                        </span>
                        <Button
                          size="small"
                          aria-label={`Dismiss ${review.label}`}
                          loading={dismissing === reviewKey(review)}
                          disabled={dismissing !== null}
                          onClick={() => void dismiss(review)}
                        >
                          Dismiss
                        </Button>
                      </li>
                    ))}
                  </ul>
                </Panel>
              );
            })
          : null}
      </div>
    </Sheet>
  );
}

function reviewKey(review: SubjectFilingReview): string {
  return `${review.entityKind}:${review.entityId}`;
}

function reasonText(review: SubjectFilingReview): string {
  const text = REASONS[review.reason] ?? "This had more than one sensible place.";
  const dropped = review.detail.droppedRangeCount;
  if (review.reason === "in_several_collections" && typeof dropped === "number" && dropped > 0) {
    return `${text} ${dropped === 1 ? "1 page range" : `${dropped.toLocaleString()} page ranges`} set in the others ${dropped === 1 ? "was" : "were"} dropped.`;
  }
  return text;
}
