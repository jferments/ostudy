import { useEffect, useMemo, useState } from "react";

import { RichTextView } from "../../editor/RichTextView";
import { OcclusionImageView } from "../../editor/occlusion/OcclusionImageView";
import type { Asset } from "../../ipc/assets";
import type { QuestionResponse } from "../../ipc/questionAttempts";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Surface } from "../../ui/primitives/Surface";
import { QuestionBody } from "../practice/QuestionBody";
import { emptyQuestionResponse } from "../practice/questionPresentation";
import { presentation } from "./presentation";
import { publicQuestionFromDraft, questionPreviewStructureKey } from "./questionPreview";
import type { QuestionEditorDraft } from "./questionDraft";
import type { OcclusionEditorDraft } from "./occlusionDraft";
import { useOcclusionSource } from "./useOcclusionImage";
import { useOcclusionPreview } from "./useOcclusionPreview";

interface QuestionDraftPreviewProps {
  draft: QuestionEditorDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onError: (message: string) => void;
}

export function QuestionDraftPreview(props: QuestionDraftPreviewProps) {
  return props.draft.kind === "image_occlusion"
    ? <OcclusionDraftPreview {...props} draft={props.draft} />
    : <ObjectiveDraftPreview {...props} draft={props.draft} />;
}

function ObjectiveDraftPreview({ draft, assets, disabled, onError }: QuestionDraftPreviewProps & {
  draft: Exclude<QuestionEditorDraft, { kind: "image_occlusion" }>;
}) {
  const question = useMemo(() => publicQuestionFromDraft(draft)!, [draft]);
  const structureKey = questionPreviewStructureKey(question);
  const [response, setResponse] = useState<QuestionResponse>(() => emptyQuestionResponse(question));
  useEffect(() => { setResponse(emptyQuestionResponse(question)); }, [structureKey]);
  return <Surface as="aside" className={presentation["editor-preview-panel"]}>
    <div className={presentation["preview-panel-heading"]}>
      <div><span className={presentation["eyebrow"]}>CARD PREVIEW</span><h2>Practice appearance</h2></div>
      <Button variant="secondary" size="small" disabled={disabled}
        onClick={() => setResponse(emptyQuestionResponse(question))}>Reset preview</Button>
    </div>
    <QuestionBody question={question} response={response} evaluation={null} assets={assets}
      disabled={disabled} onChange={setResponse} onError={onError} />
    <p className="tw:mt-3 tw:text-xs tw:text-muted-foreground">Preview interactions are local only. They do not check, schedule, or record a review.</p>
  </Surface>;
}

function OcclusionDraftPreview({ draft, assets, assetDraftId, disabled, onError }: QuestionDraftPreviewProps & {
  draft: OcclusionEditorDraft;
}) {
  const [revealed, setRevealed] = useState(false);
  const source = useOcclusionSource(draft.imageAssetId);
  const preview = useOcclusionPreview(draft, assetDraftId, true, revealed);
  const question = draft.questions.find((item) => item.id === draft.selectedQuestionId) ?? null;
  const size = draft.imageSize ?? source.size;
  const targets = draft.masks.filter((mask) => question?.maskIds.includes(mask.id)).map((mask) => mask.rect);
  useEffect(() => { setRevealed(false); }, [draft.selectedQuestionId, draft.imageAssetId]);
  return <Surface as="aside" className={presentation["editor-preview-panel"]}>
    <div className={presentation["preview-panel-heading"]}>
      <div><span className={presentation["eyebrow"]}>CARD PREVIEW</span><h2>{revealed ? "Answer" : "Question"}</h2></div>
      <Button variant="primary" size="small" disabled={disabled || !preview.ready}
        onClick={() => setRevealed((value) => !value)}>{revealed ? "Show question" : "Reveal answer"}</Button>
    </div>
    {question ? <div className="tw:grid tw:gap-4">
      <RichTextView document={question.prompt} assets={assets} onLinkError={onError} />
      {source.error ? <InlineNotice kind="error">{source.error}</InlineNotice> : null}
      {preview.error ? <InlineNotice kind="error">{preview.error} <Button onClick={preview.retry}>Retry preview</Button></InlineNotice> : null}
      {preview.ready && !preview.url && !preview.error ? <InlineProgress>Rendering the masked preview…</InlineProgress> : null}
      {preview.url && size ? <OcclusionImageView imageUrl={preview.url}
        imageKey={`${draft.imageAssetId}:${question.id}`} imageSize={size} targets={targets}
        label={revealed ? "Revealed image preview" : "Masked image preview"} /> : null}
      {revealed ? <section className="tw:grid tw:gap-2" aria-label="Preview answer">
        <h3>Answer</h3><RichTextView document={question.answer} assets={assets} onLinkError={onError} />
        {question.explanation ? <RichTextView document={question.explanation} assets={assets} onLinkError={onError} /> : null}
      </section> : null}
    </div> : <InlineNotice>Choose a question to preview.</InlineNotice>}
  </Surface>;
}
