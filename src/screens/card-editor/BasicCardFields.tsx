import type { Asset } from "../../ipc/assets";
import type { RichDocument } from "../../ipc/common";
import { RichFieldList, type RichEditorField } from "../../editor/RichFieldList";
import { Checkbox } from "../../ui/primitives/FormControls";
import { Surface } from "../../ui/primitives/Surface";

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
    <Surface className="tw:p-5">
      <label className="tw:flex tw:items-center tw:justify-between tw:gap-4 tw:text-label">
        <span>
          <strong>Create reverse card</strong>
          <small className="tw:mt-1 tw:block tw:text-muted-foreground">
            Study both directions with separate schedules and review histories.
          </small>
        </span>
        <Checkbox checked={reverseEnabled} disabled={disabled} onCheckedChange={onReverseChange} />
      </label>
    </Surface>
  </>;
}
