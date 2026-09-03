import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";

export interface RichTextEditorProps {
  label: string;
  description?: string;
  document: RichDocument;
  assets: Asset[];
  assetDraftId?: string | null;
  onChange: (document: RichDocument, plainText: string) => void;
  onAssetImported: (asset: Asset) => void;
  minHeight?: number;
  disabled?: boolean;
}

export type RichTextPresentation = "ordinary" | "question" | "answer";

export interface RichTextViewProps {
  document: RichDocument;
  assets: Asset[];
  presentation?: RichTextPresentation;
  onLinkError?: (message: string) => void;
}
