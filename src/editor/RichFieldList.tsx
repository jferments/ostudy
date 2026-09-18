import { useId, useState } from "react";
import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { Button } from "../ui/primitives/Button";
import { Icon } from "../ui/icons/Icon";
import { RichTextEditor } from "./RichTextEditor";
import { isDocumentEmpty } from "./richText";

/** Stable identity belongs to the authored field, never its label or position. */
export interface RichEditorField<Id extends string = string> {
  id: Id;
  label: string;
  description?: string;
  document: RichDocument;
  minHeight?: number;
  optional?: boolean;
  optionalLabel?: string;
}

export interface RichFieldListProps<Id extends string = string> {
  fields: readonly RichEditorField<Id>[];
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onFieldChange: (fieldId: Id, document: RichDocument, plainText: string) => void;
}

/** Fields emit deltas. Optional fields mount on first expansion and thereafter
 * remain mounted while hidden, preserving editor history and pending dialogs.
 * Collapsing never clears content or changes managed-asset ownership.
 */
export function RichFieldList<Id extends string>({ fields, ...shared }: RichFieldListProps<Id>) {
  return <>{fields.map((field) => <RichField key={field.id} field={field} {...shared} />)}</>;
}

function RichField<Id extends string>({ field, assets, assetDraftId, disabled, onAssetImported, onFieldChange }:
  Omit<RichFieldListProps<Id>, "fields"> & { field: RichEditorField<Id> }) {
  const id = useId();
  const [expanded, setExpanded] = useState(false);
  const [visited, setVisited] = useState(false);
  const editor = <RichTextEditor label={field.label} description={field.description}
    document={field.document} minHeight={field.minHeight} assets={assets} assetDraftId={assetDraftId}
    disabled={disabled} onAssetImported={onAssetImported}
    onChange={(document, plainText) => onFieldChange(field.id, document, plainText)} />;
  if (!field.optional) return editor;
  const action = expanded ? "Hide" : isDocumentEmpty("", field.document) ? "Add" : "Edit";
  return <section className="tw:grid tw:min-w-0 tw:gap-2" aria-label={field.label}>
    <div><Button variant="text" size="small" disabled={disabled} aria-expanded={expanded} aria-controls={id}
      onClick={() => { setVisited(true); setExpanded((value) => !value); }}>
      <Icon name={expanded ? "chevron-down" : "plus"} size={16} /> {action} {field.optionalLabel ?? "explanation"}
    </Button></div>
    <div id={id} hidden={!expanded}>{visited ? editor : null}</div>
  </section>;
}
