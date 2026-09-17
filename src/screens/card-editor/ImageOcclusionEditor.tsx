import { useState } from "react";

import type { Asset } from "../../ipc/assets";
import { OcclusionWorkspace } from "../../editor/occlusion/OcclusionWorkspace";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { OcclusionQuestionInspector } from "./OcclusionQuestionInspector";
import {
  mutateOcclusionRegion, removeOcclusionRegion, undoOcclusionGeometry,
  type OcclusionDraftUpdate, type OcclusionEditorDraft,
} from "./occlusionDraft";
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
  const question = draft.questions.find((item) => item.id === draft.selectedQuestionId);
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
    const count = draft.questions.filter((item) => item.maskIds.includes(id)).length;
    if (count > 0) {
      const approved = await confirm({
        title: "Remove this region?",
        description: `${count} ${count === 1 ? "question uses" : "questions use"} this region. Their text stays in the draft, but any question left without a region must be retargeted before saving. Undo region edit restores the associations.`,
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
        <p className="tw:text-sm tw:text-muted-foreground">{draft.masks.length} regions · {draft.questions.length} study cards</p>
      </div>
      <Button disabled={disabled || !assetDraftId} onClick={() => void chooseImage()}>
        {draft.imageAssetId ? "Replace image" : "Choose image"}
      </Button>
    </div>
    <p className="tw:text-sm tw:text-muted-foreground">Use a still PNG, JPEG or WebP image. Hide labels or regions, then write what you need to recall. Each question gets its own study schedule. Use Preview card above to see the masked study presentation.</p>
    {importing.busy ? <InlineProgress>Preparing the image…</InlineProgress> : null}
    {importing.error ? <InlineNotice kind="error">{importing.error}</InlineNotice> : null}
    {draft.pendingImport && !importing.busy ? <div className="tw:flex tw:flex-wrap tw:gap-2">
      <Button disabled={props.disabled} onClick={importing.retry}>Resume image import</Button>
      <Button variant="secondary" disabled={props.disabled} onClick={importing.cancel}>Cancel image import</Button>
    </div> : null}
    {source.error ? <InlineNotice kind="error">{source.error} <Button onClick={source.retry}>Retry image</Button></InlineNotice> : null}
    {draft.imageAssetId && !source.size && !source.error ? <InlineProgress>Opening the managed image…</InlineProgress> : null}
    {source.url && source.size ? <>
      <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
        <Checkbox checked={addToCurrent} disabled={disabled || !question} onCheckedChange={setAddToCurrent} />
        <span>Add new regions to the current question instead of creating a new question</span>
      </label>
      <div className="tw:grid tw:min-w-0 tw:gap-6 tw:xl:grid-cols-[minmax(0,3fr)_minmax(18rem,2fr)]">
        <OcclusionWorkspace imageKey={draft.imageAssetId!} imageUrl={source.url} imageSize={source.size}
          masks={draft.masks} selectedMaskId={draft.selectedMaskId} disabled={disabled}
          canUndo={draft.history.undo.length > 0} canRedo={draft.history.redo.length > 0}
          onSelect={(id) => onUpdate((current) => ({ ...current, selectedMaskId: id }))}
          onMutation={(mutation) => onUpdate((current) => mutateOcclusionRegion(current, mutation, addToCurrent))}
          onDelete={(id) => void deleteRegion(id)}
          onUndo={() => onUpdate((current) => undoOcclusionGeometry(current, "undo"))}
          onRedo={() => onUpdate((current) => undoOcclusionGeometry(current, "redo"))} />
        <OcclusionQuestionInspector draft={draft} assets={assets} assetDraftId={assetDraftId} disabled={disabled}
          onUpdate={onUpdate} onAssetImported={onAssetImported} />
      </div>
    </> : null}
  </Surface>;
}
