import { useEffect, useMemo, useState } from "react";

import { RichTextView } from "../../editor/RichTextView";
import { OcclusionImageView } from "../../editor/occlusion/OcclusionImageView";
import type { Asset } from "../../ipc/assets";
import { getErrorMessage } from "../../ipc/common";
import type { QuestionResponse } from "../../ipc/questionAttempts";
import { previewQuestionDraft, type QuestionDraftPreview as NativeQuestionDraftPreview } from "../../ipc/questions";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { QuestionBody } from "../practice/QuestionBody";
import { emptyQuestionResponse, responseReady } from "../practice/questionPresentation";
import { presentation } from "./presentation";
import {
  definitionFromDraft,
  questionDraftReady,
  type QuestionEditorDraft,
} from "./questionDraft";
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
  const ready = questionDraftReady(draft);
  const definitionJson = useMemo(
    () => ready ? JSON.stringify(definitionFromDraft(draft)) : null,
    [draft, ready],
  );
  const [compiled, setCompiled] = useState<NativeQuestionDraftPreview | null>(null);
  const [response, setResponse] = useState<QuestionResponse | null>(null);
  const [isCompiling, setIsCompiling] = useState(false);
  const [isChecking, setIsChecking] = useState(false);

  useEffect(() => {
    setCompiled(null);
    setResponse(null);
    setIsChecking(false);
    if (!definitionJson) return;
    let live = true;
    setIsCompiling(true);
    void previewQuestionDraft({ definitionJson, response: null }).then((result) => {
      if (!live) return;
      setCompiled(result);
      setResponse(emptyQuestionResponse(result.question));
    }).catch((error: unknown) => {
      if (live) onError(getErrorMessage(error));
    }).finally(() => {
      if (live) setIsCompiling(false);
    });
    return () => { live = false; };
  }, [definitionJson, onError]);

  async function checkAnswer(): Promise<void> {
    if (!definitionJson || !compiled || !response || disabled || isChecking || !responseReady(response)) return;
    setIsChecking(true);
    try {
      const result = await previewQuestionDraft({ definitionJson, response });
      setCompiled(result);
    } catch (error: unknown) {
      onError(getErrorMessage(error));
    } finally {
      setIsChecking(false);
    }
  }

  function reset(): void {
    if (!compiled) return;
    setCompiled({ ...compiled, evaluation: null });
    setResponse(emptyQuestionResponse(compiled.question));
  }

  if (!ready) {
    return <InlineNotice kind="warning" title="Complete this question before previewing">
      Fill the required prompt/items and answer key in the editor, then open Preview again.
    </InlineNotice>;
  }
  if (isCompiling || !compiled || !response) {
    return <InlineProgress>Validating and preparing the Practice preview…</InlineProgress>;
  }

  const checked = compiled.evaluation !== null;
  return <section className="tw:grid tw:min-w-0 tw:gap-5">
    <div className={presentation["preview-panel-heading"]}>
      <div><span className={presentation["eyebrow"]}>PRACTICE PREVIEW</span><h2>{checked ? "Feedback" : "Question"}</h2></div>
      <Button variant="secondary" size="small" disabled={disabled || isChecking} onClick={reset}>Reset preview</Button>
    </div>
    <QuestionBody question={compiled.question} response={response} evaluation={compiled.evaluation}
      assets={assets} disabled={disabled || isChecking || checked} onChange={setResponse} onError={onError} />
    {!checked ? <div className="tw:flex tw:justify-center">
      <Button variant="primary" size="large" disabled={disabled || isChecking || !responseReady(response)}
        onClick={() => void checkAnswer()}>{isChecking ? "Checking…" : "Check answer"}</Button>
    </div> : null}
    <p className="tw:text-xs tw:text-muted-foreground">
      Preview checking uses the native question evaluator but creates no attempt, review, schedule, or analytics event.
    </p>
  </section>;
}

function OcclusionDraftPreview({ draft, assets, assetDraftId, disabled, onError }: QuestionDraftPreviewProps & {
  draft: OcclusionEditorDraft;
}) {
  const [revealed, setRevealed] = useState(false);
  const [questionId, setQuestionId] = useState(draft.selectedQuestionId ?? draft.questions[0]?.id ?? "");
  const selectedQuestionId = draft.questions.some((item) => item.id === questionId)
    ? questionId
    : draft.selectedQuestionId ?? draft.questions[0]?.id ?? "";
  const previewDraft = { ...draft, selectedQuestionId: selectedQuestionId || null };
  const source = useOcclusionSource(draft.imageAssetId);
  const preview = useOcclusionPreview(previewDraft, assetDraftId, true, revealed);
  const question = draft.questions.find((item) => item.id === selectedQuestionId) ?? null;
  const size = draft.imageSize ?? source.size;
  const targets = revealed ? [] : draft.masks.filter((mask) => question?.maskIds.includes(mask.id)).map((mask) => mask.rect);
  useEffect(() => { setRevealed(false); }, [selectedQuestionId, draft.imageAssetId]);
  return <section className="tw:grid tw:min-w-0 tw:gap-5">
    <div className={presentation["preview-panel-heading"]}>
      <div><span className={presentation["eyebrow"]}>CARD PREVIEW</span><h2>{revealed ? "Answer" : "Question"}</h2></div>
      <Button variant="primary" size="small" disabled={disabled || !preview.ready}
        onClick={() => setRevealed((value) => !value)}>{revealed ? "Show question" : "Reveal answer"}</Button>
    </div>
    {draft.questions.length > 1 ? <FormField label="Presentation">
      <NativeSelect value={selectedQuestionId} onChange={(event) => {
        setQuestionId(event.currentTarget.value);
        setRevealed(false);
      }}>
        {draft.questions.map((item, index) => <option key={item.id} value={item.id}>Question {index + 1}</option>)}
      </NativeSelect>
    </FormField> : null}
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
  </section>;
}
