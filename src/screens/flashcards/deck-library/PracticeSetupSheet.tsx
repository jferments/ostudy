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
import { Checkbox, RadioGroup } from "../../../ui/primitives/FormControls";
import { Panel } from "../../../ui/primitives/Panel";
import { Badge } from "../../../ui/primitives/Badge";

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
      <Sheet title="Practice selected decks" description="Review the selected decks and optional tags before starting scheduled or extra study."
        width="medium" onClose={onClose} busy={isStarting} dismissible={!isStarting}
        actions={<>
          <Button disabled={isStarting} onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={isStarting} disabled={!request || isStarting} onClick={() => void start()}>Start practice</Button>
        </>}>
        <div className="tw:grid tw:min-w-0 tw:gap-6" data-layout-mode={layoutMode}>
          <Panel title="Selected decks" actions={<Badge>{scope.decks.length} {scope.decks.length === 1 ? "deck" : "decks"}</Badge>}>
            <ul className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
              {scope.decks.map((deck) => <li key={deck.id} className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
                <strong className="tw:min-w-0 tw:break-words tw:font-medium">{deck.name}</strong>
                <span className="tw:text-caption tw:text-muted-foreground">{deck.generatedCardCount} generated cards</span>
              </li>)}
            </ul>
          </Panel>
          {!scopeValidation.valid ? <InlineNotice kind="error">{scopeValidation.message}</InlineNotice> : null}
          {scopeValidation.valid ? <>
            <Panel title="Refine by tags" description="Optional · matching cards must have at least one selected tag."
              actions={<Badge>{selectedTagIds.length}/{MAX_PRACTICE_TAGS}</Badge>}>
              {!currentTags || currentTags.status === "loading" ? <InlineProgress>Loading tags…</InlineProgress> : null}
              {currentTags?.status === "error" ? <InlineNotice kind="error">Tags are unavailable: {currentTags.message}{" "}
                <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadTags()}>Retry tags</Button>
              </InlineNotice> : null}
              {currentTags?.status === "ready" && currentTags.value.length === 0 ? <p className="tw:m-0 tw:text-label tw:text-muted-foreground">None of the selected decks use tags yet.</p> : null}
              {currentTags?.status === "ready" && currentTags.value.length > 0 ? <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2">
                {currentTags.value.map((tag) => <label key={tag.id} className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-3 tw:has-[[data-checked]]:border-primary tw:has-[[data-checked]]:bg-accent">
                  <Checkbox checked={selectedTagIds.includes(tag.id)} disabled={!canAddPracticeTag(selectedTagIds, tag.id) || isStarting}
                    onCheckedChange={(checked) => setSelectedTagIds((current) => {
                      const next = new Set(current);
                      if (checked) next.add(tag.id); else next.delete(tag.id);
                      return reconcilePracticeTagIds(next, currentTags.value);
                    })} />
                  <span className="tw:grid tw:min-w-0 tw:gap-1"><strong className="tw:break-words tw:font-medium">{tag.name}</strong>
                    <small className="tw:text-muted-foreground">{tag.generatedCardCount} generated · {tag.studyableDueCount} ready</small></span>
                </label>)}
              </div> : null}
            </Panel>
            <Panel title="Cards available to study" aria-live="polite">
              {!currentWorkload || currentWorkload.status === "loading" ? <InlineProgress>Updating…</InlineProgress> : null}
              {currentWorkload?.status === "error" ? <InlineNotice kind="error">Study counts are unavailable: {currentWorkload.message}{" "}
                <Button size="small" variant="text" disabled={isStarting} onClick={() => void loadWorkload()}>Retry study counts</Button>
              </InlineNotice> : null}
              {currentWorkload?.status === "ready" ? <WorkloadMetrics workload={currentWorkload.value} /> : null}
            </Panel>
            <Panel title="Study mode">
              <RadioGroup label="Study mode" name="practice-mode" value={mode} onValueChange={setMode} disabled={isStarting}
                options={([
                  { value: "scheduled", title: "Scheduled study", availability: availability?.scheduled },
                  { value: "extra", title: "Extra study", availability: availability?.extra },
                ] as const).map((option) => ({ value: option.value, disabled: !option.availability?.available,
                  label: <span className="tw:grid tw:gap-1"><strong className="tw:font-medium">{option.title}</strong>
                    <small className="tw:font-normal tw:leading-relaxed tw:text-muted-foreground">{option.availability?.explanation ?? "Checking available cards…"}</small></span>,
                }))} />
              <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:text-label">
                <Checkbox checked={shuffle} disabled={isStarting} onCheckedChange={setShuffle} />
                <span>Shuffle cards while keeping study priorities</span>
              </label>
            </Panel>
            {request ? <p className="tw:m-0 tw:text-caption tw:text-muted-foreground">
              Starting exactly {request.deckIds.length} {request.deckIds.length === 1 ? "deck" : "decks"}, {request.tagIds.length} {request.tagIds.length === 1 ? "tag" : "tags"}, {request.mode} mode, shuffle {request.shuffle ? "on" : "off"}.
            </p> : null}
            {startError ? <InlineNotice kind="error">{startError}</InlineNotice> : null}
          </> : null}
        </div>
      </Sheet>
    </PerformanceProfiler>
  );
}

function WorkloadMetrics({ workload }: { workload: PracticeWorkloadOverview }) {
  const metrics = [
    ["Due reviews", workload.studyableDueReviews], ["Due new", workload.studyableDueNew],
    ["Scheduled reviews", workload.scheduledReviews], ["Scheduled new", workload.scheduledNew],
    ["Omitted reviews", workload.omittedDueReviews], ["Omitted new", workload.omittedDueNew],
    ["Extra available", workload.extraAvailable],
  ] as const;
  return <dl className="tw:m-0 tw:grid tw:grid-cols-2 tw:gap-3 tw:sm:grid-cols-3">
    {metrics.map(([label, count]) => <div key={label} className="tw:rounded-lg tw:bg-muted tw:p-3">
      <dt className="tw:text-caption tw:text-muted-foreground">{label}</dt><dd className="tw:m-0 tw:mt-1 tw:text-section tw:font-semibold tw:tabular-nums">{count}</dd>
    </div>)}
  </dl>;
}
