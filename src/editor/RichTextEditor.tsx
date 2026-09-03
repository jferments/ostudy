import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { TiptapRichTextEditor } from "./tiptap/TiptapRichTextEditor";

export interface RichTextEditorProps {
  label: string;
  description?: string;
  document: RichDocument;
  assets: Asset[];
  assetDraftId?: string | null;
  onChange: (document: RichDocument, plainText: string) => void;
  onAssetImported?: (asset: Asset) => void;
  onRequestImage?: () => Promise<Asset | null>;
  minHeight?: number;
  disabled?: boolean;
}

/** Editor-neutral feature surface backed by a private concrete-editor adapter. */
export function RichTextEditor(props: RichTextEditorProps) {
  return <TiptapRichTextEditor {...props} />;
}
