import { useMemo, useState } from "react";

import { RichTextEditor } from "../../editor/RichTextEditor";
import {
  TEMPLATE_FIELD_BLOCK,
  TEMPLATE_FIELD_INLINE,
} from "../../editor/extensions";
import { cloneDocument } from "../../editor/richText";
import type {
  Asset,
  CardTemplateField,
  RichDocument,
} from "../../ipc";

interface TemplateLayoutEditorProps {
  label: string;
  document: RichDocument;
  fields: CardTemplateField[];
  assets: Asset[];
  disabled: boolean;
  onChange: (document: RichDocument) => void;
  onAssetImported: (asset: Asset) => void;
}

export function TemplateLayoutEditor({
  label,
  document,
  fields,
  assets,
  disabled,
  onChange,
  onAssetImported,
}: TemplateLayoutEditorProps) {
  const availableFields = useMemo(
    () => fields.filter((field) => field.name.trim() !== ""),
    [fields],
  );
  const [selectedFieldId, setSelectedFieldId] = useState("");
  const [editorVersion, setEditorVersion] = useState(0);
  const selected =
    availableFields.find((field) => field.id === selectedFieldId) ??
    availableFields[0];

  function insertPlaceholder(kind: "inline" | "block"): void {
    if (!selected) return;
    const next = cloneDocument(document) as {
      type?: string;
      content?: Array<Record<string, unknown>>;
    };
    if (!Array.isArray(next.content)) next.content = [];
    const placeholder = {
      type: kind === "inline" ? TEMPLATE_FIELD_INLINE : TEMPLATE_FIELD_BLOCK,
      attrs: { fieldId: selected.id, label: selected.name },
    };
    if (kind === "block") {
      next.content.push(placeholder);
    } else {
      let target = [...next.content]
        .reverse()
        .find(
          (node) => node.type === "paragraph" || node.type === "heading",
        ) as { content?: Array<Record<string, unknown>> } | undefined;
      if (!target) {
        target = { type: "paragraph", content: [] } as {
          content?: Array<Record<string, unknown>>;
        };
        next.content.push(target as Record<string, unknown>);
      }
      if (!Array.isArray(target.content)) target.content = [];
      target.content.push(placeholder);
    }
    onChange(next as RichDocument);
    setEditorVersion((value) => value + 1);
  }

  return (
    <section className="template-layout-editor">
      <div className="template-field-inserter">
        <label>
          <span>Insert field</span>
          <select
            value={selected?.id ?? ""}
            disabled={disabled || availableFields.length === 0}
            onChange={(event) => setSelectedFieldId(event.currentTarget.value)}
          >
            {availableFields.length === 0 ? (
              <option value="">Add a field first</option>
            ) : null}
            {availableFields.map((field) => (
              <option key={field.id} value={field.id}>
                {field.name}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="secondary-button"
          disabled={disabled || !selected}
          onClick={() => insertPlaceholder("inline")}
        >
          Insert inline
        </button>
        <button
          type="button"
          className="secondary-button"
          disabled={disabled || !selected}
          onClick={() => insertPlaceholder("block")}
        >
          Insert as block
        </button>
        <small>
          Use inline for a short value inside a sentence. Use block for lists,
          images, headings, or multiple paragraphs.
        </small>
      </div>
      <RichTextEditor
        key={`${label}-${editorVersion}`}
        label={label}
        description="Design this side with the full editor, then insert fields where their content should appear."
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
