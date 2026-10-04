import { useState } from "react";

import type { Asset } from "../../ipc/assets";
import type { OcclusionQuestion } from "../../ipc/questions";
import { OcclusionWorkspace } from "../../editor/occlusion/OcclusionWorkspace";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { OcclusionQuestionInspector } from "./OcclusionQuestionInspector";
import {
  mutateOcclusionRegion, occlusionStudyQuestions, removeOcclusionRegion, selectOcclusionRegion,
  setOcclusionMode, undoOcclusionGeometry, type OcclusionDraftUpdate, type OcclusionEditorDraft,
} from "./occlusionDraft";
import { hasCustomOcclusionContent } from "./occlusionQuestionDefaults";
import { useOcclusionImport, useOcclusionSource } from "./useOcclusionImage";

interface ImageOcclusionEditorProps {
  draft: OcclusionEditorDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onUpdate: (update: OcclusionDraftUpdate) => void;
  flushDraft: () => Promise<void>;
}

export function ImageOcclusionEditor(props: ImageOcclusionEditorProps) {
  const { draft, assets, assetDraftId, onUpdate, onAssetImported } = props;
  const importing = useOcclusionImport(props);
  const source = useOcclusionSource(draft.imageAssetId);
  const confirm = useConfirm();
  const [addToCurrent, setAddToCurrent] = useState(false);
  const [showDetails, setShowDetails] = useState(false);
  const [newRegionMode, setNewRegionMode] = useState<OcclusionQuestion["mode"]>("hide_target");
  const question = draft.questions.find((item) => item.id === draft.selectedQuestionId);
  const studyQuestions = occlusionStudyQuestions(draft);
  const modes = new Set(studyQuestions.map((item) => item.mode));
  const mode = modes.size > 1 ? "mixed" : studyQuestions[0]?.mode ?? newRegionMode;
  const incomplete = studyQuestions.filter((item) => item.maskIds.length === 0);
  const disabled = props.disabled || importing.busy || !!draft.pendingImport;

  async function chooseImage(): Promise<void> {
    if (props.disabled || importing.busy || draft.pendingImport) return;
    if (draft.imageAssetId && (draft.masks.length > 0 || draft.questions.length > 0)) {
      const approved = await confirm({
        title: "Replace the image and start new questions?",
        description: "The replacement starts without the old regions or questions. The saved note is unchanged until Save. Canceling or failing image selection leaves your current draft intact.",
        confirmLabel: "Choose replacement", destructive: true,
      });
      if (!approved) return;
    }
    importing.start();
  }
  async function deleteRegion(id: string): Promise<void> {
    if (disabled) return;
    const custom = draft.questions.filter((item) => item.maskIds.includes(id) && hasCustomOcclusionContent(item));
    if (custom.length > 0) {
      const approved = await confirm({
        title: "Remove this region?",
        description: `${custom.length} custom ${custom.length === 1 ? "question uses" : "questions use"} this region. Their text stays in the draft, but any custom question left without a region must be retargeted before saving. Undo region edit restores the associations.`,
        confirmLabel: "Remove region", destructive: true,
      });
      if (!approved) return;
    }
    onUpdate((current) => removeOcclusionRegion(current, id));
  }

  return <Surface className="tw:grid tw:min-w-0 tw:gap-5 tw:p-4 tw:md:p-5">
    <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
      <div className="tw:grid tw:gap-1">
        <h2>Image occlusion</h2>
        <p className="tw:text-small tw:text-muted-foreground">{draft.masks.length} regions · {studyQuestions.length} study cards</p>
      </div>
      <Button disabled={disabled || !assetDraftId} onClick={() => void chooseImage()}>
        {draft.imageAssetId ? "Replace image" : "Choose image"}
      </Button>
    </div>
    <p className="tw:text-small tw:text-muted-foreground">Draw rectangles over labels or regions. Each rectangle creates a study card automatically; no question or answer text is required. Reveal shows the uncovered image. Use Preview card above to try it.</p>
    {importing.busy ? <InlineProgress>Preparing the image…</InlineProgress> : null}
    {importing.error ? <InlineNotice kind="error">{importing.error}</InlineNotice> : null}
    {draft.pendingImport && !importing.busy ? <div className="tw:flex tw:flex-wrap tw:gap-2">
      <Button disabled={props.disabled} onClick={importing.retry}>Resume image import</Button>
      <Button variant="secondary" disabled={props.disabled} onClick={importing.cancel}>Cancel image import</Button>
    </div> : null}
    {source.error ? <InlineNotice kind="error">{source.error} <Button onClick={source.retry}>Retry image</Button></InlineNotice> : null}
    {draft.imageAssetId && !source.size && !source.error ? <InlineProgress>Opening the managed image…</InlineProgress> : null}
    {source.url && source.size ? <>
      <div className="tw:flex tw:flex-wrap tw:items-end tw:gap-4">
        <FormField label="Question-side masks">
          <NativeSelect value={mode} disabled={disabled} onChange={(event) => {
            const value = event.currentTarget.value;
            if (value !== "hide_target" && value !== "hide_all") return;
            setNewRegionMode(value);
            onUpdate((current) => setOcclusionMode(current, value));
          }}>
            {mode === "mixed" ? <option value="mixed" disabled>Mixed settings from existing questions</option> : null}
            <option value="hide_target">Hide one region</option>
            <option value="hide_all">Hide all regions</option>
          </NativeSelect>
        </FormField>
        <Button aria-expanded={showDetails} disabled={disabled} onClick={() => setShowDetails((value) => !value)}>
          {showDetails ? "Hide optional question details" : "Optional question details"}
        </Button>
      </div>
      <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
        <Checkbox checked={addToCurrent} disabled={disabled || !question} onCheckedChange={setAddToCurrent} />
        <span>Group new rectangles with the selected card</span>
      </label>
      {addToCurrent ? <p className="tw:text-small tw:text-muted-foreground">Grouped regions are recalled together on one card. “Hide one region” hides that card’s whole group; “Hide all regions” also hides the other cards’ regions.</p> : null}
      {studyQuestions.length >= 64 && !addToCurrent ? <InlineNotice>One image supports up to 64 study cards. Group additional regions with an existing card, or start another image.</InlineNotice> : null}
      {incomplete.length > 0 ? <InlineNotice kind="warning">
        {incomplete.length} custom question{incomplete.length === 1 ? " needs" : "s need"} a target region before saving. Your written content is preserved.
        <Button onClick={() => {
          setShowDetails(true);
          const id = incomplete[0]?.id;
          if (id) onUpdate((current) => ({ ...current, selectedQuestionId: id }));
        }}>Repair question targets</Button>
      </InlineNotice> : null}
      <div className={showDetails
        ? "tw:grid tw:min-w-0 tw:gap-6 tw:xl:grid-cols-[minmax(0,3fr)_minmax(18rem,2fr)]"
        : "tw:grid tw:min-w-0 tw:gap-6"}>
        <OcclusionWorkspace imageKey={draft.imageAssetId!} imageUrl={source.url} imageSize={source.size}
          initialTool="rectangle" masks={draft.masks} selectedMaskId={draft.selectedMaskId} disabled={disabled}
          canUndo={draft.history.undo.length > 0} canRedo={draft.history.redo.length > 0}
          onSelect={(id) => onUpdate((current) => selectOcclusionRegion(current, id))}
          onMutation={(mutation) => onUpdate((current) => mutateOcclusionRegion(current, mutation, addToCurrent,
            mode === "mixed" ? undefined : mode))}
          onDelete={(id) => void deleteRegion(id)}
          onUndo={() => onUpdate((current) => undoOcclusionGeometry(current, "undo"))}
          onRedo={() => onUpdate((current) => undoOcclusionGeometry(current, "redo"))} />
        {showDetails ? <OcclusionQuestionInspector draft={draft} assets={assets} assetDraftId={assetDraftId} disabled={disabled}
          onUpdate={onUpdate} onAssetImported={onAssetImported} /> : null}
      </div>
    </> : null}
  </Surface>;
}
