import { RichTextEditor } from "../../editor/RichTextEditor";
import type { Asset, RichDocument } from "../../ipc";

interface TemplateLayoutEditorProps {
  label: string;
  document: RichDocument;
  assets: Asset[];
  disabled: boolean;
  onChange: (document: RichDocument) => void;
  onAssetImported: (asset: Asset) => void;
}

export function TemplateLayoutEditor({
  label,
  document,
  assets,
  disabled,
  onChange,
  onAssetImported,
}: TemplateLayoutEditorProps) {
  return (
    <section className="template-layout-editor">
      <RichTextEditor
        label={label}
        description={
          "Type a field name in double braces, such as {{Subject}}. Simple one-paragraph values stay inline; lists, images, headings, block quotes, and multi-paragraph values become blocks automatically."
        }
        document={document}
        assets={assets}
        disabled={disabled}
        onAssetImported={onAssetImported}
        onChange={(nextDocument) => onChange(nextDocument)}
        minHeight={220}
      />
    </section>
  );
}
