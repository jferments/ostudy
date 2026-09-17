import { fireEvent, render, screen } from "@testing-library/react";
import { useEffect, useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { RichTextEditorProps } from "./RichTextEditor";
import { RichFieldList } from "./RichFieldList";
import { Textarea } from "../ui/primitives/FormControls";

const mocks = vi.hoisted(() => ({ mounted: vi.fn(), unmounted: vi.fn() }));
vi.mock("./RichTextEditor", () => ({ RichTextEditor: (props: RichTextEditorProps) => {
  useEffect(() => { mocks.mounted(); return () => { mocks.unmounted(); }; }, []);
  return <Textarea aria-label={props.label} value={JSON.stringify(props.document)}
    onChange={() => props.onChange({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "Saved explanation" }] }] }, "Saved explanation")} />;
} }));
const empty = { type: "doc", content: [{ type: "paragraph" }] };
function Harness() {
  const [document, setDocument] = useState(empty);
  return <RichFieldList fields={[{ id: "reason", label: "Explanation", document, optional: true }]}
    assets={[]} assetDraftId={null} disabled={false} onAssetImported={vi.fn()} onFieldChange={(_id, next) => setDocument(next as typeof empty)} />;
}

describe("optional rich fields", () => {
  it("starts closed, then preserves the editor instance and content on collapse/reopen", () => {
    mocks.mounted.mockClear(); mocks.unmounted.mockClear();
    render(<Harness />);
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add explanation" }));
    const input = screen.getByRole("textbox", { name: "Explanation" });
    fireEvent.change(input, { target: { value: "anything" } });
    expect(input).toHaveProperty("value", expect.stringContaining("Saved explanation"));
    fireEvent.click(screen.getByRole("button", { name: "Hide explanation" }));
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(mocks.unmounted).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Edit explanation" }));
    expect(screen.getByRole("textbox")).toBe(input);
    expect(mocks.mounted).toHaveBeenCalledTimes(1);
  });
});
