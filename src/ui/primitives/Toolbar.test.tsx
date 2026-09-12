import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createRef, type FormEvent } from "react";
import { describe, expect, it, vi } from "vitest";
import { Toolbar, ToolbarAction, ToolbarGroup } from "./Toolbar";
import { ColorField } from "./ColorField";

describe("editor command controls", () => {
  it("exposes pressed state and native button refs without submitting a surrounding form", () => {
    const run = vi.fn();
    const submit = vi.fn((event: FormEvent) => event.preventDefault());
    const ref = createRef<HTMLButtonElement>();
    render(<form onSubmit={submit}><Toolbar label="Formatting"><ToolbarGroup label="Marks"><ToolbarAction ref={ref} label="Bold" pressed onClick={run}>B</ToolbarAction></ToolbarGroup></Toolbar></form>);
    const button = screen.getByRole("button", { name: "Bold" });
    expect(ref.current).toBe(button);
    expect(button.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(button);
    expect(run).toHaveBeenCalledTimes(1);
    expect(submit).not.toHaveBeenCalled();
  });
  it("delegates roving command focus to the toolbar implementation", async () => {
    render(<Toolbar label="Formatting"><ToolbarAction label="Bold">B</ToolbarAction><ToolbarAction label="Italic">I</ToolbarAction></Toolbar>);
    const bold = screen.getByRole("button", { name: "Bold" });
    act(() => bold.focus());
    fireEvent.keyDown(bold, { key: "ArrowRight" });
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Italic" })));
  });
  it("does not execute disabled commands", () => {
    const run = vi.fn();
    render(<Toolbar label="Formatting" disabled><ToolbarAction label="Bold" onClick={run}>B</ToolbarAction></Toolbar>);
    fireEvent.click(screen.getByRole("button", { name: "Bold" }));
    expect(run).not.toHaveBeenCalled();
  });
  it("keeps authored color values and native color picker semantics", () => {
    const ref = createRef<HTMLInputElement>();
    const change = vi.fn();
    render(<label>Text color<ColorField ref={ref} defaultValue="#224466" onChange={change} /></label>);
    expect(ref.current?.type).toBe("color");
    fireEvent.change(screen.getByLabelText("Text color"), { target: { value: "#446688" } });
    expect(ref.current?.value).toBe("#446688");
    expect(change).toHaveBeenCalledOnce();
  });
});
