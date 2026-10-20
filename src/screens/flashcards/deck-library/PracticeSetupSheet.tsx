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
import { InlineNotice, InlineProgress } from "../../../ui/primitives/InlineNotice";
import {
  canAddPracticeTag,
  canonicalPracticeWorkloadQuery,
  createPracticeSetupRequest,
  practiceModeAvailability,
  reconcilePracticeTagIds,
  validatePracticeDeckScope,
  type PracticeDeckScope,
  type SetupResource,
} from "./practiceSetupModel";
import { Switch } from "../../../ui/primitives/FormControls";
import { Chip, ChipButton } from "../../../ui/primitives/Chip";
import { SegmentedControl } from "../../../ui/primitives/SegmentedControl";
import { Icon } from "../../../ui/icons/Icon";

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

  const scheduledCount = availability?.scheduled.count;
  const extraCount = availability?.extra.count;
  const readyTags = currentTags?.status === "ready" ? currentTags.value : null;

  return (
    <PerformanceProfiler id="practice-setup">
      <Sheet title="Practice selected decks"
        width="medium" onClose={onClose} busy={isStarting} dismissible={!isStarting}
        actions={<>
          <Button disabled={isStarting} onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={isStarting} disabled={!request || isStarting} onClick={() => void start()}>Start practice</Button>
        </>}>
        <div className="tw:grid tw:min-w-0 tw:gap-6" data-layout-mode={layoutMode}>
          <section className="tw:grid tw:gap-2" aria-labelledby="practice-setup-decks">
            <h3 id="practice-setup-decks" className={SECTION_LABEL}>{scope.decks.length === 1 ? "Deck" : "Decks"}</h3>
            <ul className="tw:m-0 tw:flex tw:list-none tw:flex-wrap tw:gap-2 tw:p-0">
              {scope.decks.map((deck) => <li key={deck.id} className="tw:min-w-0"><Chip selected className="tw:max-w-full tw:break-words">{deck.name}</Chip></li>)}
            </ul>
          </section>
          {!scopeValidation.valid ? <InlineNotice kind="error">{scopeValidation.message}</InlineNotice> : null}
          {scopeValidation.valid ? <>
            {!currentTags || currentTags.status === "loading" ? <InlineProgress>Loading tags…</InlineProgress> : null}
            {currentTags?.status === "error" ? <InlineNotice kind="error">Tags are unavailable: {currentTags.message}{" "}
              <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadTags()}>Retry tags</Button>
            </InlineNotice> : null}
            {readyTags && readyTags.length > 0 ? <section className="tw:grid tw:gap-2" aria-labelledby="practice-setup-tags">
              <h3 id="practice-setup-tags" className={SECTION_LABEL}>Tags</h3>
              <div className="tw:flex tw:flex-wrap tw:gap-2" role="group" aria-labelledby="practice-setup-tags">
                {readyTags.map((tag) => {
                  const selected = selectedTagIds.includes(tag.id);
                  return <ChipButton key={tag.id} selected={selected}
                    disabled={(!selected && !canAddPracticeTag(selectedTagIds, tag.id)) || isStarting}
                    onClick={() => setSelectedTagIds((current) => {
                      const next = new Set(current);
                      if (selected) next.delete(tag.id); else next.add(tag.id);
                      return reconcilePracticeTagIds(next, readyTags);
                    })}>
                    {selected ? <Icon name="check" size={14} /> : null}{tag.name}
                  </ChipButton>;
                })}
              </div>
            </section> : null}
            <section className="tw:grid tw:gap-2" aria-labelledby="practice-setup-cards" aria-live="polite">
              <h3 id="practice-setup-cards" className={SECTION_LABEL}>Cards</h3>
              {currentWorkload?.status === "error" ? <InlineNotice kind="error">Study counts are unavailable: {currentWorkload.message}{" "}
                <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadWorkload()}>Retry study counts</Button>
              </InlineNotice> : null}
              <SegmentedControl label="Cards" value={mode} onChange={setMode}
                disabled={isStarting || currentWorkload?.status !== "ready"}
                options={[
                  { value: "scheduled", label: scheduledCount === undefined ? "Due" : `Due · ${scheduledCount.toLocaleString()}`, disabled: !availability?.scheduled.available },
                  { value: "extra", label: extraCount === undefined ? "Extra" : `Extra · ${extraCount.toLocaleString()}`, disabled: !availability?.extra.available },
                ]} />
            </section>
            <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:justify-between tw:gap-4 tw:text-body tw:font-medium">
              <span>Shuffle</span>
              <Switch checked={shuffle} disabled={isStarting} onCheckedChange={setShuffle} />
            </label>
            {startError ? <InlineNotice kind="error">{startError}</InlineNotice> : null}
          </> : null}
        </div>
      </Sheet>
    </PerformanceProfiler>
  );
}

const SECTION_LABEL = "tw:m-0 tw:text-label tw:font-semibold tw:text-muted-foreground";
