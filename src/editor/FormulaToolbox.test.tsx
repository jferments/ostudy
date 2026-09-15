import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { FormulaInput } from "../math/mathlive";
import { FormulaToolbox } from "./FormulaToolbox";

function input(): FormulaInput {
  return {
    focus: vi.fn(),
    applyTemplate: vi.fn(() => true),
    showKeyboard: vi.fn(() => true),
    toggleKeyboard: vi.fn(() => true),
    hideKeyboard: vi.fn(() => false),
    dispose: vi.fn(),
  };
}

describe("FormulaToolbox", () => {
  it("uses MathLive selection-aware templates for common structures", () => {
    const port = input();
    render(<FormulaToolbox input={port} disabled={false} />);

    expect(screen.getByText(/Select an existing expression/u)).toBeTruthy();
    fireEvent.pointerDown(screen.getByRole("button", { name: "Fraction" }));
    fireEvent.click(screen.getByRole("button", { name: "Fraction" }));
    expect(port.applyTemplate).toHaveBeenCalledWith(String.raw`\frac{#@}{#?}`);

    fireEvent.click(screen.getByRole("button", { name: "Square" }));
    expect(port.applyTemplate).toHaveBeenLastCalledWith("{#@}^{2}");

    fireEvent.click(screen.getByRole("button", { name: "Square root" }));
    expect(port.applyTemplate).toHaveBeenLastCalledWith(String.raw`\sqrt{#0}`);
  });

  it("switches between graphical symbol groups without exposing formula syntax", () => {
    const port = input();
    render(<FormulaToolbox input={port} disabled={false} />);

    fireEvent.click(screen.getByRole("tab", { name: "Relations" }));
    expect(screen.getByRole("tab", { name: "Relations" }).getAttribute("aria-selected")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Less than or equal" }));
    expect(port.applyTemplate).toHaveBeenLastCalledWith(String.raw`\le`);

    fireEvent.click(screen.getByRole("tab", { name: "Greek" }));
    fireEvent.click(screen.getByRole("button", { name: "Theta" }));
    expect(port.applyTemplate).toHaveBeenLastCalledWith(String.raw`\theta`);

    fireEvent.click(screen.getByRole("tab", { name: "Calculus" }));
    fireEvent.click(screen.getByRole("button", { name: "Definite integral" }));
    expect(port.applyTemplate).toHaveBeenLastCalledWith(String.raw`\int_{#?}^{#?} #0\,d#?`);
  });

  it("provides task-oriented help and the pinned MathLive shortcuts on keyboard platforms", () => {
    render(<FormulaToolbox input={input()} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Shortcuts & help" }));

    expect(screen.getByText(/Highlight an expression/u)).toBeTruthy();
    expect(screen.getByText("/")).toBeTruthy();
    expect(screen.getByText("Alt+V")).toBeTruthy();
    expect(screen.getByText("Alt+Shift+V")).toBeTruthy();
    expect(screen.getByText("Alt+P")).toBeTruthy();
    expect(screen.getByText("Tab")).toBeTruthy();
    expect(screen.getByText(/LaTeX-style commands/u)).toBeTruthy();
  });

  it("hides desktop shortcut help when the platform does not expose a hardware-keyboard workflow", () => {
    render(<FormulaToolbox input={input()} disabled={false} showKeyboardHelp={false} />);
    expect(screen.queryByRole("button", { name: "Shortcuts & help" })).toBeNull();
    expect(screen.queryByText("Alt+V")).toBeNull();
  });

  it("keeps graphical insertion unavailable until the MathLive port is ready", () => {
    const view = render(<FormulaToolbox input={null} disabled />);
    expect((screen.getByRole("button", { name: "Fraction" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("tab", { name: "Relations" }) as HTMLButtonElement).disabled).toBe(true);
    view.unmount();
  });
});
