import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { RichTextEditor } from "./RichTextEditor";
import { extractRichDocumentText } from "./richText";

interface StudyRichEditorProps {
  value: RichDocument;
  assets: Asset[];
  disabled?: boolean;
  label: string;
  onChange: (document: RichDocument, text: string) => void;
  onRequestImage?: () => Promise<Asset | null>;
}

/** Compact study-artifact editor using the editor-neutral public surface. */
export function StudyRichEditor({
  value,
  assets,
  disabled = false,
  label,
  onChange,
  onRequestImage,
}: StudyRichEditorProps) {
  return (
    <RichTextEditor
      label={label}
      document={value}
      assets={assets}
      disabled={disabled}
      minHeight={128}
      onChange={onChange}
      onRequestImage={onRequestImage}
    />
  );
}

export function emptyRichDocument(): RichDocument {
  return {
    type: "doc",
    attrs: { schemaVersion: 2 },
    content: [{ type: "paragraph" }],
  };
}

export function plainTextDocument(value: string): RichDocument {
  return {
    type: "doc",
    attrs: { schemaVersion: 2 },
    content: value.split("\n").map((line) => ({
      type: "paragraph",
      content: line ? [{ type: "text", text: line }] : undefined,
    })),
  };
}

export function extractText(value: RichDocument): string {
  return extractRichDocumentText(value);
}
