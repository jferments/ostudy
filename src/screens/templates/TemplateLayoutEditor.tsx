import { RichTextEditor } from "../../editor/RichTextEditor";
import {
  type Asset,
} from "../../ipc/assets";
import {
  type RichDocument,
} from "../../ipc/common";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "../TemplatesScreen.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface TemplateLayoutEditorProps {
  label: string;
  document: RichDocument;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onChange: (document: RichDocument) => void;
  onAssetImported: (asset: Asset) => void;
}

export function TemplateLayoutEditor({
  label,
  document,
  assets,
  assetDraftId,
  disabled,
  onChange,
  onAssetImported,
}: TemplateLayoutEditorProps) {
  return (
    <section className={ownedClassNames(ownedStyleMaps, "template-layout-editor")}>
      <RichTextEditor
        label={label}
        description={
          "Type a field name in double braces, such as {{Subject}}. Simple one-paragraph values stay inline; lists, images, headings, block quotes, and multi-paragraph values become blocks automatically."
        }
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
