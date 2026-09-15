import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import type { AnswerKeySide } from "./richText";
import { TiptapRichTextView } from "./tiptap/TiptapRichTextView";

export interface RichTextViewProps {
  document: RichDocument;
  assets: Asset[];
  answerKeySide?: AnswerKeySide | null;
  /** Hidden card faces must not receive a runtime media URL or decoder resource. */
  mediaPlaybackActive?: boolean;
  className?: string;
  onLinkError?: (message: string) => void;
}

/** Editor-neutral read-only rich-content surface. */
export function RichTextView(props: RichTextViewProps) {
  return <TiptapRichTextView {...props} />;
}
