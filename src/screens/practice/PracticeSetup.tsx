import { useEffect, useRef } from "react";

import type { DeckSummary } from "../../ipc/decks";
import type { PracticeWorkloadOverview } from "../../ipc/practice";
import { formatConfidence, type RecentDeckSummary } from "./confidence";
import {
  resolvePracticeSetupDeckIds,
  savePracticeSetupDeckIds,
} from "./practiceSetupScope";
import { Button } from "../../ui/primitives/Button";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { Panel } from "../../ui/primitives/Panel";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "./Practice.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


export type PracticeWorkloadStatus = "idle" | "empty" | "loading" | "ready" | "error";

interface PracticeSetupProps {
  decks: DeckSummary[];
  selectedDeckIds: Set<string>;
  shuffle: boolean;
  isStarting: boolean;
  workloadStatus: PracticeWorkloadStatus;
  workload: PracticeWorkloadOverview | null;
  workloadError: string | null;
  recentDecks: RecentDeckSummary[];
  onSelectedDeckIdsChange: (deckIds: Set<string>) => void;
  onShuffleChange: (shuffle: boolean) => void;
  onStart: (deckIds: string[], shuffle: boolean, mode: "scheduled" | "extra") => void;
}

const reviewedDateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export function PracticeSetup({
  decks,
  selectedDeckIds,
  shuffle,
  isStarting,
  workloadStatus,
  workload,
  workloadError,
  recentDecks,
  onSelectedDeckIdsChange,
  onShuffleChange,
  onStart,
}: PracticeSetupProps) {
  const scopeInitialized = useRef(false);
  const decksWithCards = decks.filter(
    (deck) => deck.archivedAt === null && deck.generatedCardCount > 0,
  );
  const workloadReady = workloadStatus === "ready" && workload !== null;
  const scheduledCount = workloadReady
    ? workload.scheduledReviews + workload.scheduledNew
    : 0;

  useEffect(() => {
    if (scopeInitialized.current || decks.length === 0) return;
    scopeInitialized.current = true;
    if (selectedDeckIds.size > 0) return;
    const restored = resolvePracticeSetupDeckIds(decks);
    if (restored.length > 0) onSelectedDeckIdsChange(new Set(restored));
  }, [decks, onSelectedDeckIdsChange, selectedDeckIds.size]);

  function commitDeckSelection(next: Set<string>): void {
    savePracticeSetupDeckIds(next);
    onSelectedDeckIdsChange(next);
  }

  if (decksWithCards.length === 0) {
    return (
      <EmptyState
        title="No active flashcards yet"
        description="Add a card to an active deck or unarchive a deck before starting a session."
      />
    );
  }

  return (
    <>
      <Panel >
        <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row")}>
          <div>
            <h2>Select decks to study</h2>
            <span>{selectedDeckIds.size} selected</span>
          </div>
          <div className={ownedClassNames(ownedStyleMaps, "inline-actions")}>
            <Button
              type="button"
              variant="text"
              onClick={() => commitDeckSelection(new Set(decksWithCards.map((deck) => deck.id)))}
            >
              Select all
            </Button>
            {selectedDeckIds.size > 0 ? (
              <Button
                type="button"
                variant="text"
                onClick={() => commitDeckSelection(new Set())}
              >
                Clear
              </Button>
            ) : null}
          </div>
        </div>

        <div className={ownedClassNames(ownedStyleMaps, "practice-deck-list")}>
          {decksWithCards.map((deck) => {
            const checked = selectedDeckIds.has(deck.id);
            return (
              <label
                key={deck.id}
                className={ownedClassNames(ownedStyleMaps, ["practice-deck-choice", checked ? "selected" : ""]
                  .filter(Boolean)
                  .join(" "))}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={(event) => {
                    const next = new Set(selectedDeckIds);
                    if (event.currentTarget.checked) next.add(deck.id);
                    else next.delete(deck.id);
                    commitDeckSelection(next);
                  }}
                />
                <span>
                  <strong>{deck.name}</strong>
                  <small>
                    {deck.studyableDueCount > 0
                      ? `${deck.studyableDueCount} ready now · ${deck.generatedCardCount} generated total`
                      : `Nothing ready now · ${deck.generatedCardCount} generated total`}
                  </small>
                </span>
              </label>
            );
          })}
        </div>

        {workloadReady && selectedDeckIds.size > 0 ? (
          <div className={ownedClassNames(ownedStyleMaps, "session-plan-copy")}>
            <strong>{scheduledCount} cards in the scheduled session</strong>
            <span>
              {workload.scheduledReviews} reviews · {workload.scheduledNew} new
            </span>
            {workload.omittedDueReviews > 0 ? (
              <small>
                {workload.omittedDueReviews} ready reviews remain outside today’s limit.
              </small>
            ) : null}
            {workload.omittedDueNew > 0 ? (
              <small>
                {workload.omittedDueNew} ready new cards are paused by the workload policy.
              </small>
            ) : null}
          </div>
        ) : null}

        <div className={ownedClassNames(ownedStyleMaps, "practice-setup-actions")}>
          <label className={ownedClassNames(ownedStyleMaps, "compact-shuffle-control")}>
            <input
              type="checkbox"
              checked={shuffle}
              onChange={(event) => onShuffleChange(event.currentTarget.checked)}
            />
            <span>Shuffle within priority groups</span>
          </label>
          <div className={ownedClassNames(ownedStyleMaps, "practice-due-inline")}>
            <strong>{workloadReady ? scheduledCount : "—"}</strong>
            <span>scheduled</span>
          </div>
          {workloadReady && scheduledCount > 0 ? (
            <Button
              type="button"
              variant="primary" className={ownedClassNames(ownedStyleMaps, "practice-start-button")}
              disabled={selectedDeckIds.size === 0 || isStarting}
              onClick={() => onStart([...selectedDeckIds], shuffle, "scheduled")}
            >
              {isStarting ? "Starting…" : "Start scheduled practice"}
            </Button>
          ) : workloadReady && workload.extraAvailable > 0 && selectedDeckIds.size > 0 ? (
            <Button
              type="button"
              variant="secondary" className={ownedClassNames(ownedStyleMaps, "practice-start-button")}
              disabled={isStarting}
              onClick={() => onStart([...selectedDeckIds], shuffle, "extra")}
            >
              {isStarting ? "Starting…" : "Study this selection anyway"}
            </Button>
          ) : (
            <Button type="button" variant="primary" className={ownedClassNames(ownedStyleMaps, "practice-start-button")} disabled>
              Start scheduled practice
            </Button>
          )}
        </div>
      </Panel>

      <Panel  aria-label="Practice workload" aria-live="polite">
        <div>
          <span>Today</span>
          <strong>{workloadReady ? workload.reviewsRemaining : "—"}</strong>
          <small>reviews remaining</small>
        </div>
        <div>
          <span>New</span>
          <strong>{workloadReady ? workload.newRemaining : "—"}</strong>
          <small>cards remaining</small>
        </div>
        <div>
          <span>Ready now</span>
          <strong>
            {workloadReady ? workload.studyableDueReviews + workload.studyableDueNew : "—"}
          </strong>
          <small>studyable cards</small>
        </div>
        <div>
          <span>Priority</span>
          <strong>{workloadReady ? workload.needsMorePractice : "—"}</strong>
          <small>need more practice</small>
        </div>
        <WorkloadStatus status={workloadStatus} error={workloadError} />
      </Panel>

      <section className={ownedClassNames(ownedStyleMaps, "recent-practice-section")} aria-labelledby="recent-practice-heading">
        <div className={ownedClassNames(ownedStyleMaps, "recent-practice-heading-row")}>
          <h2 id="recent-practice-heading">Recently practiced</h2>
        </div>
        {recentDecks.length === 0 ? (
          <p className={ownedClassNames(ownedStyleMaps, "recent-practice-empty")}>
            Complete a practice session and its confidence result will appear here.
          </p>
        ) : (
          <div className={ownedClassNames(ownedStyleMaps, "recent-practice-list")}>
            {recentDecks.map((deck) => (
              <article key={deck.deckName} className={ownedClassNames(ownedStyleMaps, "recent-practice-row")}>
                <div>
                  <strong>{deck.deckName}</strong>
                  <span>
                    {deck.reviewCount} recent reviews ·{" "}
                    {reviewedDateFormatter.format(new Date(deck.lastReviewedAt))}
                  </span>
                </div>
                <div className={ownedClassNames(ownedStyleMaps, "recent-confidence-copy")}>
                  <strong>{formatConfidence(deck.averageConfidence)}</strong>
                  <span>average confidence</span>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </>
  );
}

function WorkloadStatus({
  status,
  error,
}: {
  status: PracticeWorkloadStatus;
  error: string | null;
}) {
  if (status === "loading") {
    return <span className={ownedClassNames(ownedStyleMaps, "workload-loading")}>Updating…</span>;
  }
  if (status === "empty") {
    return <span className={ownedClassNames(ownedStyleMaps, "workload-loading")}>Select at least one deck to plan a session.</span>;
  }
  if (status === "error") {
    return (
      <span className={ownedClassNames(ownedStyleMaps, "workload-loading")}>
        Workload unavailable{error ? `: ${error}` : "."}
      </span>
    );
  }
  if (status === "idle") {
    return <span className={ownedClassNames(ownedStyleMaps, "workload-loading")}>Preparing workload…</span>;
  }
  return null;
}
