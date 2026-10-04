import { isDocumentEmpty } from "../../editor/richText";
import type { RichDocument } from "../../ipc/common";
import type { OcclusionQuestion } from "../../ipc/questions";

function paragraph(text: string): RichDocument {
  return { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] };
}

const prompt = "Identify the highlighted region.";
const answer = "Check your recall against the uncovered image.";

/** Image pixels are the answer for rectangle-first cards. Keep native rich-text
 * contracts intact without requiring the author to transcribe every hidden label. */
export function withOcclusionDefaults(question: OcclusionQuestion): OcclusionQuestion {
  return {
    ...question,
    prompt: isDocumentEmpty("", question.prompt) ? paragraph(prompt) : question.prompt,
    answer: isDocumentEmpty("", question.answer) ? paragraph(answer) : question.answer,
  };
}

/** Only empty/default prose may be omitted when a region is undone or removed.
 * Any authored text, rich formatting, media or explanation remains recoverable
 * and still requires a valid target before it can be saved. */
export function hasCustomOcclusionContent(question: OcclusionQuestion): boolean {
  return (!isDocumentEmpty("", question.prompt) && JSON.stringify(question.prompt) !== JSON.stringify(paragraph(prompt)))
    || (!isDocumentEmpty("", question.answer) && JSON.stringify(question.answer) !== JSON.stringify(paragraph(answer)))
    || (!!question.explanation && !isDocumentEmpty("", question.explanation));
}
