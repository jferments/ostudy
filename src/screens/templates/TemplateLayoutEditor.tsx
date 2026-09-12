import { RichTextEditor } from "../../editor/RichTextEditor";
import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";

interface TemplateLayoutEditorProps {
  label: string;
  document: RichDocument;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onChange: (document: RichDocument) => void;
  onAssetImported: (asset: Asset) => void;
}

/** Template rich-content state stays in the editor, independent of its Settings chrome. */
export function TemplateLayoutEditor({
  label, document, assets, assetDraftId, disabled, onChange, onAssetImported,
}: TemplateLayoutEditorProps) {
  return (
    <section className="tw:grid tw:min-w-0 tw:gap-4">
      <RichTextEditor
        label={label}
        description="Type a field name in double braces, such as {{Subject}}. Simple one-paragraph values stay inline; lists, images, headings, block quotes, and multi-paragraph values become blocks automatically."
        document={document}
        assets={assets}
        assetDraftId={assetDraftId}
        disabled={disabled}
        onAssetImported={onAssetImported}
        onChange={(nextDocument) => onChange(nextDocument)}
        minHeight={220}
      />
    </section>
  );
}
