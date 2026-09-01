import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { PracticeSetupController } from "../../../controllers/practiceSetupController";
import { getErrorMessage } from "../../../ipc/common";
import type { DeckSummary } from "../../../ipc/decks";
import type { PracticeWorkloadOverview, SelectedPracticeRequest } from "../../../ipc/practice";
import type { TagSummary } from "../../../ipc/tags";
import { PerformanceProfiler } from "../../../testing/PerformanceProfiler";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { Sheet } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import {
  MAX_PRACTICE_TAGS,
  canAddPracticeTag,
  canonicalPracticeWorkloadQuery,
  createPracticeSetupRequest,
  practiceModeAvailability,
  reconcilePracticeTagIds,
  validatePracticeDeckScope,
  type PracticeDeckScope,
  type SetupResource,
} from "./practiceSetupModel";
import styles from "./PracticeSetupSheet.module.css";

interface PracticeSetupSheetProps {
  scope: PracticeDeckScope;
  currentDecks: readonly DeckSummary[];
  initialShuffle: boolean;
  onClose(): void;
  onStart(request: SelectedPracticeRequest): void | Promise<void>;
}

export function PracticeSetupSheet({
  scope,
  currentDecks,
  initialShuffle,
  onClose,
  onStart,
}: PracticeSetupSheetProps) {
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [shuffle, setShuffle] = useState(initialShuffle);
  const [mode, setMode] = useState<"scheduled" | "extra">("scheduled");
  const [tags, setTags] = useState<SetupResource<TagSummary[]> | null>(null);
  const [workload, setWorkload] =
    useState<SetupResource<PracticeWorkloadOverview> | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);
  const startingRef = useRef(false);
  const controllerRef = useRef<PracticeSetupController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new PracticeSetupController();
  const controller = controllerRef.current;
  const scopeValidation = validatePracticeDeckScope(scope, currentDecks);
  const deckQuery = useMemo(
    () => canonicalPracticeWorkloadQuery(scope.deckIds, []),
    [scope.deckIds],
  );
  const workloadQuery = useMemo(
    () => canonicalPracticeWorkloadQuery(scope.deckIds, selectedTagIds),
    [scope.deckIds, selectedTagIds],
  );
  const { layoutMode } = useLayoutMode();

  const loadTags = useCallback(async (): Promise<void> => {
    if (startingRef.current) return;
    setTags({ key: deckQuery.key, status: "loading" });
    try {
      const loaded = await controller.tags(deckQuery.deckIds);
      if (!loaded) return;
      setSelectedTagIds((current) => reconcilePracticeTagIds(current, loaded));
      setTags({ key: deckQuery.key, status: "ready", value: loaded });
    } catch (error: unknown) {
      setTags({ key: deckQuery.key, status: "error", message: getErrorMessage(error) });
    }
  }, [controller, deckQuery]);

  const loadWorkload = useCallback(async (): Promise<void> => {
    if (startingRef.current) return;
    setWorkload({ key: workloadQuery.key, status: "loading" });
    try {
      const loaded = await controller.workload(
        workloadQuery.deckIds,
        workloadQuery.tagIds,
      );
      if (!loaded) return;
      setWorkload({ key: workloadQuery.key, status: "ready", value: loaded });
    } catch (error: unknown) {
      setWorkload({
        key: workloadQuery.key,
        status: "error",
        message: getErrorMessage(error),
      });
    }
  }, [controller, workloadQuery]);

  useEffect(() => () => controller.invalidate(), [controller]);

  useEffect(() => {
    if (!scopeValidation.valid) {
      controller.invalidate();
      setTags(null);
      setWorkload(null);
      return;
    }
    void loadTags();
  }, [controller, loadTags, scopeValidation.valid]);

  useEffect(() => {
    if (!scopeValidation.valid) {
      controller.invalidate();
      return;
    }
    void loadWorkload();
  }, [controller, loadWorkload, scopeValidation.valid]);

  const currentTags = tags?.key === deckQuery.key ? tags : null;
  const currentWorkload = workload?.key === workloadQuery.key ? workload : null;
  const availability = useMemo(
    () =>
      currentWorkload?.status === "ready"
        ? practiceModeAvailability(currentWorkload.value)
        : null,
    [currentWorkload],
  );
  const request =
    scopeValidation.valid && currentWorkload?.status === "ready"
      ? createPracticeSetupRequest(
          scope,
          workloadQuery.tagIds,
          shuffle,
          mode,
          currentWorkload.value,
        )
      : null;

  async function start(): Promise<void> {
    if (!request || startingRef.current) return;
    startingRef.current = true;
    setIsStarting(true);
    setStartError(null);
    try {
      await onStart(request);
    } catch (error: unknown) {
      setStartError(getErrorMessage(error));
    } finally {
      startingRef.current = false;
      setIsStarting(false);
    }
  }

  return (
    <PerformanceProfiler id="practice-setup">
      <Sheet
        title="Practice selected decks"
        description="Review the selected decks and optional tags before starting scheduled or extra study."
        width="medium"
        onClose={onClose}
        busy={isStarting}
        dismissible={!isStarting}
      >
        <div className={styles.content} data-layout-mode={layoutMode}>
          <section className={styles.scope} aria-labelledby="practice-scope-heading">
            <div className={styles.sectionHeading}>
              <h2 id="practice-scope-heading">Selected decks</h2>
              <span>{scope.decks.length} {scope.decks.length === 1 ? "deck" : "decks"}</span>
            </div>
            <ul>
              {scope.decks.map((deck) => (
                <li key={deck.id}>
                  <strong>{deck.name}</strong>
                  <span>{deck.generatedCardCount} generated cards</span>
                </li>
              ))}
            </ul>
          </section>

          {!scopeValidation.valid ? (
            <InlineNotice kind="error">{scopeValidation.message}</InlineNotice>
          ) : null}

          {scopeValidation.valid ? (
          <>
          <section className={styles.tags} aria-labelledby="practice-tags-heading">
            <div className={styles.sectionHeading}>
              <div>
                <h2 id="practice-tags-heading">Refine by tags</h2>
                <p>Optional · matching cards must have at least one selected tag.</p>
              </div>
              <span>{selectedTagIds.length}/{MAX_PRACTICE_TAGS}</span>
            </div>
            {!currentTags || currentTags.status === "loading" ? <p aria-live="polite">Loading tags…</p> : null}
            {currentTags?.status === "error" ? (
              <InlineNotice kind="error">
                Tags are unavailable: {currentTags.message}{" "}
                <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadTags()}>
                  Retry tags
                </Button>
              </InlineNotice>
            ) : null}
            {currentTags?.status === "ready" && currentTags.value.length === 0 ? (
              <p>None of the selected decks use tags yet.</p>
            ) : null}
            {currentTags?.status === "ready" && currentTags.value.length > 0 ? (
              <div className={styles.tagGrid}>
                {currentTags.value.map((tag) => {
                  const checked = selectedTagIds.includes(tag.id);
                  return (
                    <label key={tag.id}>
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={!canAddPracticeTag(selectedTagIds, tag.id) || isStarting}
                        onChange={(event) => {
                          const isChecked = event.currentTarget.checked;
                          setSelectedTagIds((current) => {
                            const next = new Set(current);
                            if (isChecked) next.add(tag.id);
                            else next.delete(tag.id);
                            return reconcilePracticeTagIds(next, currentTags.value);
                          });
                        }}
                      />
                      <span>
                        <strong>{tag.name}</strong>
                        <small>{tag.generatedCardCount} generated · {tag.studyableDueCount} ready</small>
                      </span>
                    </label>
                  );
                })}
              </div>
            ) : null}
          </section>

          <section className={styles.workload} aria-labelledby="practice-workload-heading" aria-live="polite">
            <div className={styles.sectionHeading}>
              <h2 id="practice-workload-heading">Cards available to study</h2>
              {!currentWorkload || currentWorkload.status === "loading" ? <span>Updating…</span> : null}
            </div>
            {currentWorkload?.status === "error" ? (
              <InlineNotice kind="error">
                Study counts are unavailable: {currentWorkload.message}{" "}
                <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadWorkload()}>
                  Retry study counts
                </Button>
              </InlineNotice>
            ) : null}
            {currentWorkload?.status === "ready" ? <WorkloadMetrics workload={currentWorkload.value} /> : null}
          </section>

          <fieldset className={styles.modes} disabled={isStarting || !scopeValidation.valid}>
            <legend>Study mode</legend>
            <label data-unavailable={availability ? !availability.scheduled.available : true}>
              <input
                type="radio"
                name="practice-mode"
                value="scheduled"
                checked={mode === "scheduled"}
                disabled={!availability?.scheduled.available}
                onChange={() => setMode("scheduled")}
              />
              <span>
                <strong>Scheduled study</strong>
                <small>{availability?.scheduled.explanation ?? "Checking available cards…"}</small>
              </span>
            </label>
            <label data-unavailable={availability ? !availability.extra.available : true}>
              <input
                type="radio"
                name="practice-mode"
                value="extra"
                checked={mode === "extra"}
                disabled={!availability?.extra.available}
                onChange={() => setMode("extra")}
              />
              <span>
                <strong>Extra study</strong>
                <small>{availability?.extra.explanation ?? "Checking available cards…"}</small>
              </span>
            </label>
          </fieldset>

          <label className={styles.shuffle}>
            <input
              type="checkbox"
              checked={shuffle}
              disabled={isStarting}
              onChange={(event) => setShuffle(event.currentTarget.checked)}
            />
            <span>Shuffle cards while keeping study priorities</span>
          </label>

          {request ? (
            <p className={styles.requestSummary}>
              Starting exactly {request.deckIds.length} {request.deckIds.length === 1 ? "deck" : "decks"}, {request.tagIds.length} {request.tagIds.length === 1 ? "tag" : "tags"}, {request.mode} mode, shuffle {request.shuffle ? "on" : "off"}.
            </p>
          ) : null}
          {startError ? <InlineNotice kind="error">{startError}</InlineNotice> : null}
          </>
          ) : null}
          <div className={styles.actions}>
            <Button disabled={isStarting} onClick={onClose}>Cancel</Button>
            <Button
              variant="primary"
              loading={isStarting}
              disabled={!request || isStarting}
              onClick={() => void start()}
            >
              Start practice
            </Button>
          </div>
        </div>
      </Sheet>
    </PerformanceProfiler>
  );
}

function WorkloadMetrics({ workload }: { workload: PracticeWorkloadOverview }) {
  return (
    <dl className={styles.metrics}>
      <div><dt>Due reviews</dt><dd>{workload.studyableDueReviews}</dd></div>
      <div><dt>Due new</dt><dd>{workload.studyableDueNew}</dd></div>
      <div><dt>Scheduled reviews</dt><dd>{workload.scheduledReviews}</dd></div>
      <div><dt>Scheduled new</dt><dd>{workload.scheduledNew}</dd></div>
      <div><dt>Omitted reviews</dt><dd>{workload.omittedDueReviews}</dd></div>
      <div><dt>Omitted new</dt><dd>{workload.omittedDueNew}</dd></div>
      <div><dt>Extra available</dt><dd>{workload.extraAvailable}</dd></div>
    </dl>
  );
}
