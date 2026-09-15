import { cleanup, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Asset } from "../ipc/assets";
import type { RichDocument } from "../ipc/common";
import { RichFieldList, type RichEditorField, type RichFieldListProps } from "./RichFieldList";
import type { RichTextEditorProps } from "./RichTextEditor";

const editorState = vi.hoisted(() => ({
  nextInstance: 0,
  fields: new Map<string, RichTextEditorProps>(),
}));

vi.mock("./RichTextEditor", () => ({
  RichTextEditor: function TestEditor(props: RichTextEditorProps) {
    const [instance] = useState(() => ++editorState.nextInstance);
    editorState.fields.set(props.label, props);
    return <div data-testid={props.label} data-instance={instance}>{props.label}</div>;
  },
}));

const asset: Asset = {
  id: "a".repeat(32), mimeType: "audio/mpeg", originalFilename: "local.mp3",
  byteSize: 100, createdAt: 1,
};
const document: RichDocument = {
  type: "doc", attrs: { schemaVersion: 3 }, content: [
    { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "x^2" } }] },
    { type: "audio", attrs: { assetId: asset.id, src: `study-asset://${asset.id}` } },
  ],
};
const fields: RichEditorField[] = [
  { id: "left-id", label: "Term", description: "First field", document },
  { id: "right-id", label: "Definition", document, minHeight: 180 },
];

function props(): RichFieldListProps {
  return {
    fields, assets: [asset], assetDraftId: "d".repeat(32), disabled: false,
    onAssetImported: vi.fn(), onFieldChange: vi.fn(),
  };
}

function field(label: string): RichTextEditorProps {
  const value = editorState.fields.get(label);
  if (!value) throw new Error(`Missing test editor: ${label}`);
  return value;
}

beforeEach(() => { editorState.fields.clear(); editorState.nextInstance = 0; });
afterEach(cleanup);

describe("RichFieldList", () => {
  it("shares managed resources and emits an identity-addressed delta without changing rich content", () => {
    const input = props();
    render(<RichFieldList {...input} />);
    expect(field("Term").document).toBe(document);
    expect(field("Definition").document).toBe(document);
    expect(field("Term").assets).toBe(input.assets);
    expect(field("Definition").assetDraftId).toBe(input.assetDraftId);
    expect(field("Definition").minHeight).toBe(180);
    expect(field("Term").description).toBe("First field");
    field("Definition").onChange(document, "x squared; local audio");
    expect(input.onFieldChange).toHaveBeenCalledWith(
      "right-id", document, "x squared; local audio",
    );
    field("Term").onAssetImported?.(asset);
    expect(input.onAssetImported).toHaveBeenCalledWith(asset);
  });

  it("retains editor instances when rows move and dispatches by ID, not position", () => {
    const input = props();
    const { rerender } = render(<RichFieldList {...input} />);
    const term = screen.getByTestId("Term");
    const definition = screen.getByTestId("Definition");
    rerender(<RichFieldList {...input} fields={[...fields].reverse()} />);
    expect(screen.getByTestId("Term")).toBe(term);
    expect(screen.getByTestId("Definition")).toBe(definition);
    expect(definition.compareDocumentPosition(term) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    field("Term").onChange(document, "edited term");
    expect(input.onFieldChange).toHaveBeenCalledWith("left-id", document, "edited term");
  });

  it("does not reset a field's session merely because its label changes", () => {
    const input = props();
    const { rerender } = render(<RichFieldList {...input} />);
    const original = screen.getByTestId("Term");
    const renamed = fields.map((item) => item.id === "left-id" ? { ...item, label: "Prompt" } : item);
    rerender(<RichFieldList {...input} fields={renamed} />);
    expect(screen.getByTestId("Prompt")).toBe(original);
    field("Prompt").onChange(document, "new prompt");
    expect(input.onFieldChange).toHaveBeenCalledWith("left-id", document, "new prompt");
  });

  it("removes only the deleted field and gives a replacement ID a new instance", () => {
    const input = props();
    const { rerender } = render(<RichFieldList {...input} />);
    const originalTerm = screen.getByTestId("Term");
    const originalDefinition = screen.getByTestId("Definition");
    rerender(<RichFieldList {...input} fields={fields.filter((item) => item.id === "right-id")} />);
    expect(screen.queryByTestId("Term")).toBeNull();
    expect(screen.getByTestId("Definition")).toBe(originalDefinition);
    rerender(<RichFieldList {...input} fields={fields.map((item) =>
      item.id === "left-id" ? { ...item, id: "replacement-id" } : item,
    )} />);
    expect(screen.getByTestId("Term")).not.toBe(originalTerm);
    expect(screen.getByTestId("Definition")).toBe(originalDefinition);
  });

  it("propagates write locks without remounting fields", () => {
    const input = props();
    const { rerender } = render(<RichFieldList {...input} />);
    const term = screen.getByTestId("Term");
    rerender(<RichFieldList {...input} disabled />);
    expect(field("Term").disabled).toBe(true);
    expect(field("Definition").disabled).toBe(true);
    expect(screen.getByTestId("Term")).toBe(term);
  });

  it("resets all field sessions only when the caller starts a new authoring session", () => {
    const input = props();
    const { rerender } = render(<RichFieldList key="session-1" {...input} />);
    const term = screen.getByTestId("Term");
    const definition = screen.getByTestId("Definition");
    rerender(<RichFieldList key="session-2" {...input} />);
    expect(screen.getByTestId("Term")).not.toBe(term);
    expect(screen.getByTestId("Definition")).not.toBe(definition);
  });
});
