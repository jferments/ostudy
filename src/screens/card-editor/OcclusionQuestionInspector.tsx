import { RichFieldList } from "../../editor/RichFieldList";
import { EMPTY_RICH_DOCUMENT } from "../../editor/richText";
import type { Asset } from "../../ipc/assets";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { FormField } from "../../ui/primitives/FormField";
import { useConfirm } from "../../ui/overlays/overlayContext";
import {
  addOcclusionQuestion, occlusionStudyQuestions, optionalOcclusionField, removeOcclusionQuestion, updateOcclusionQuestion,
  type OcclusionDraftUpdate, type OcclusionEditorDraft,
} from "./occlusionDraft";
import { hasCustomOcclusionContent } from "./occlusionQuestionDefaults";

interface InspectorProps {
  draft: OcclusionEditorDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onUpdate: (update: OcclusionDraftUpdate) => void;
  onAssetImported: (asset: Asset) => void;
}

export function OcclusionQuestionInspector({ draft, assets, assetDraftId, disabled, onUpdate, onAssetImported }: InspectorProps) {
  const confirm = useConfirm();
  const question = draft.questions.find((candidate) => candidate.id === draft.selectedQuestionId);
  async function remove(): Promise<void> {
    if (!question || disabled) return;
    const id = question.id;
    const approved = await confirm({
      title: "Remove this question?",
      description: "Its regions stay available for other questions. Saving the note retires this question's scheduled card; other questions keep their schedules.",
      confirmLabel: "Remove question", destructive: true,
    });
    if (approved) onUpdate((current) => removeOcclusionQuestion(current, id));
  }
  return <section className="tw:grid tw:min-w-0 tw:content-start tw:gap-4" aria-label="Image question inspector">
    <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
      <h3>Optional question details</h3>
      <Button disabled={disabled || draft.masks.length === 0 || occlusionStudyQuestions(draft).length >= 64}
        onClick={() => onUpdate(addOcclusionQuestion)}>Add question</Button>
    </div>
    <p className="tw:text-small tw:text-muted-foreground">The uncovered image is the answer. These fields are optional; leaving them empty uses a simple recall prompt and image-based answer. Custom questions keep their own study schedules.</p>
    <FormField label="Current question">
      <NativeSelect value={question?.id ?? ""} disabled={disabled || draft.questions.length === 0}
        onChange={(event) => {
          const id = event.currentTarget.value;
          onUpdate((current) => ({ ...current, selectedQuestionId: id }));
        }}>
        {!question ? <option value="">Choose a question…</option> : null}
        {draft.questions.map((item, index) => <option key={item.id} value={item.id}>Question {index + 1}{item.maskIds.length === 0 ? " (no region)" : ""}</option>)}
      </NativeSelect>
    </FormField>
    {question ? <>
      <RichFieldList key={question.id} assets={assets} assetDraftId={assetDraftId} disabled={disabled}
        onAssetImported={onAssetImported}
        fields={[
          { id: "prompt", label: "Question (optional)", document: question.prompt, minHeight: 90 },
          { id: "answer", label: "Answer notes (optional)", document: question.answer, minHeight: 100 },
          {
            id: "explanation",
            label: "Explanation",
            document: question.explanation ?? EMPTY_RICH_DOCUMENT,
            minHeight: 80,
            optional: true,
            optionalLabel: "explanation",
          },
        ]}
        onFieldChange={(field, document) => onUpdate((current) => updateOcclusionQuestion(current, question.id,
          field === "explanation" ? { explanation: optionalOcclusionField(document) }
            : field === "answer" ? { answer: document } : { prompt: document }))} />
      <fieldset className="tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:p-0" disabled={disabled}>
        <legend className="tw:mb-2 tw:font-semibold">Regions this question asks about</legend>
        {draft.masks.map((mask, index) => <label key={mask.id} className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
          <Checkbox checked={question.maskIds.includes(mask.id)} disabled={disabled}
            onCheckedChange={(checked) => onUpdate((current) => {
              const target = current.questions.find((item) => item.id === question.id);
              if (!target) return current;
              const ids = target.maskIds.filter((id) => id !== mask.id);
              if (checked && current.masks.some((item) => item.id === mask.id)) ids.push(mask.id);
              return updateOcclusionQuestion(current, target.id, { maskIds: ids });
            })} />
          <span>Region {index + 1}</span>
        </label>)}
      </fieldset>
      {question.maskIds.length === 0 ? <InlineNotice kind="warning">{hasCustomOcclusionContent(question)
        ? "Select at least one region. Your custom question and answer remain in the draft until you repair its target."
        : "This question has no region and will not be saved. Undo the region edit or select a region to restore it."}</InlineNotice> : null}
      <RadioGroup label="Which regions are hidden?" value={question.mode} disabled={disabled}
        options={[
          { value: "hide_all" as const, label: "Hide all regions on the question side" },
          { value: "hide_target" as const, label: "Hide only this question's targets" },
        ]}
        onValueChange={(mode) => onUpdate((current) => updateOcclusionQuestion(current, question.id, { mode }))} />
      <p className="tw:text-small tw:text-muted-foreground">Reveal removes every mask and number badge, including overlapping regions.</p>
      <Button variant="danger" disabled={disabled} onClick={() => void remove()}>Remove question</Button>
    </> : <p className="tw:text-small tw:text-muted-foreground">Drawing a region creates a card. Add another question to reuse an existing region.</p>}
  </section>;
}
