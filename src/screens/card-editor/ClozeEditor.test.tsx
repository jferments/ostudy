import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import type { ClozeMode } from "../../ipc/cards";
import { ClozeEditor } from "./ClozeEditor";

function EditorHarness({ disabled = false }: { disabled?: boolean }) {
  const [source, setSource] = useState("The heart pumps blood.");
  const [mode, setMode] = useState<ClozeMode>("one_at_a_time");
  return <ClozeEditor source={source} mode={mode} disabled={disabled} onSourceChange={setSource} onModeChange={setMode} />;
}
describe("cloze editor shared controls", () => {
  it("preserves selection marking through the textarea facade", async () => {
    render(<EditorHarness />);
    const text = screen.getByRole("textbox", { name: "Complete statement" }) as HTMLTextAreaElement;
    text.setSelectionRange(4, 9);
    fireEvent.click(screen.getByRole("button", { name: "Mark selection as blank" }));
    await waitFor(() => expect(text.value).toContain("{{c1::heart}}"));
    fireEvent.click(screen.getByRole("button", { name: "Remove all blank markers" }));
    expect(text.value).toBe("The heart pumps blood.");
  });
  it("updates blank behavior with accessible radio selection", async () => {
    render(<EditorHarness />);
    const all = screen.getByRole("radio", { name: /Hide all blanks together/u });
    fireEvent.click(all);
    await waitFor(() => expect(all.getAttribute("aria-checked")).toBe("true"));
    expect(screen.getByText("1 card · all blanks")).toBeTruthy();
  });
  it("preserves the disabled editor boundary", () => {
    render(<EditorHarness disabled />);
    expect((screen.getByRole("textbox", { name: "Complete statement" }) as HTMLTextAreaElement).disabled).toBe(true);
    const all = screen.getByRole("radio", { name: /Hide all blanks together/u });
    fireEvent.click(all);
    expect(all.getAttribute("aria-checked")).toBe("false");
  });
});
