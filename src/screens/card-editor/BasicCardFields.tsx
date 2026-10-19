import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { RichFieldList, type RichEditorField } from "../../editor/RichFieldList";
import { Checkbox } from "../../ui/primitives/FormControls";

type BasicFieldId = "front" | "back";

interface BasicCardFieldsProps {
  frontDoc: RichDocument;
  backDoc: RichDocument;
  reverseEnabled: boolean;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onFieldChange: (fieldId: BasicFieldId, document: RichDocument, plainText: string) => void;
  onReverseChange: (enabled: boolean) => void;
}

/** Basic owns front/back and reversal; the enclosing editor owns saving/drafts. */
export function BasicCardFields({
  frontDoc,
  backDoc,
  reverseEnabled,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onFieldChange,
  onReverseChange,
}: BasicCardFieldsProps) {
  const fields: RichEditorField<BasicFieldId>[] = [
    { id: "front", label: "Question", document: frontDoc },
    { id: "back", label: "Answer", document: backDoc, minHeight: 220 },
  ];

  return <>
    <RichFieldList fields={fields} assets={assets} assetDraftId={assetDraftId}
      disabled={disabled} onAssetImported={onAssetImported} onFieldChange={onFieldChange} />
    <label
      title="Study both directions with separate schedules"
      className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:px-4 tw:text-label"
    >
      <Checkbox checked={reverseEnabled} disabled={disabled} onCheckedChange={onReverseChange} />
      <span className="tw:font-medium">Create reverse card</span>
    </label>
  </>;
}
