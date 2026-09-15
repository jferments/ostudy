import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { RichTextEditor } from "./RichTextEditor";

/** Stable identity belongs to the authored field, never its label or position. */
export interface RichEditorField<Id extends string = string> {
  id: Id;
  label: string;
  description?: string;
  document: RichDocument;
  minHeight?: number;
}

export interface RichFieldListProps<Id extends string = string> {
  fields: readonly RichEditorField<Id>[];
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onFieldChange: (fieldId: Id, document: RichDocument, plainText: string) => void;
}

/**
 * Repeated rich inputs share one resource boundary without owning note storage.
 * ID-keyed editors retain their selection/history when fields move or labels
 * change. The caller resets the list's React key only for a new authoring session.
 * Emit field deltas, not a captured copy of the entire form: two edits delivered
 * in one React batch must not overwrite each other's fields.
 */
export function RichFieldList<Id extends string>({
  fields,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onFieldChange,
}: RichFieldListProps<Id>) {
  return <>{fields.map((field) => (
    <RichTextEditor
      key={field.id}
      label={field.label}
      description={field.description}
      document={field.document}
      minHeight={field.minHeight}
      assets={assets}
      assetDraftId={assetDraftId}
      disabled={disabled}
      onAssetImported={onAssetImported}
      onChange={(document, plainText) => onFieldChange(field.id, document, plainText)}
    />
  ))}</>;
}
