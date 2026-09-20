import { useMemo, useState } from "react";

import type { Asset } from "../../ipc/assets";
import type { CardDraftPreview } from "../../ipc/cards";
import { Badge } from "../../ui/primitives/Badge";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { RecallCardSurface } from "../practice/RecallCardSurface";

export interface RecallPreviewPresentation extends CardDraftPreview {
  assets: Asset[];
}

interface RecallDraftPreviewProps {
  presentations: RecallPreviewPresentation[];
  tags: string[];
  loading: boolean;
  disabled: boolean;
  onError: (message: string) => void;
}

export function RecallDraftPreview({
  presentations,
  tags,
  loading,
  disabled,
  onError,
}: RecallDraftPreviewProps) {
  const [selectedKey, setSelectedKey] = useState("");
  const [revealed, setRevealed] = useState(false);
  const selected = useMemo(
    () => presentations.find((item) => item.presentationKey === selectedKey) ?? presentations[0] ?? null,
    [presentations, selectedKey],
  );

  if (loading) return <InlineProgress>Validating and preparing the Practice preview…</InlineProgress>;
  if (!selected) {
    return <InlineNotice kind="warning" title="Preview is not ready">
      Complete the required card or template fields, then try Preview again.
    </InlineNotice>;
  }

  return <section className="tw:grid tw:min-w-0 tw:gap-5">
    {presentations.length > 1 ? <FormField label="Presentation">
      <NativeSelect value={selected.presentationKey} onChange={(event) => {
        setSelectedKey(event.currentTarget.value);
        setRevealed(false);
      }}>
        {presentations.map((item) => <option key={item.presentationKey} value={item.presentationKey}>
          {item.presentationLabel}
        </option>)}
      </NativeSelect>
    </FormField> : null}
    {tags.length > 0 ? <div className="tw:flex tw:flex-wrap tw:gap-2">
      {tags.map((tag) => <Badge key={tag}>{tag}</Badge>)}
    </div> : null}
    <RecallCardSurface presentation={selected} presentationKey={selected.presentationKey}
      revealed={revealed} onReveal={() => setRevealed(true)} onLinkError={onError} />
    <div className="tw:flex tw:justify-center">
      <Button variant="primary" size="large" disabled={disabled}
        onClick={() => setRevealed((value) => !value)}>
        {revealed ? "Show question" : "Reveal answer"}
      </Button>
    </div>
  </section>;
}
