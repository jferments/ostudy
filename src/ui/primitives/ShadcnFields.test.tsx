import { createRef, useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { FormField } from "./FormField";
import { NativeSelect } from "./NativeSelect";
import { TextField } from "./TextField";

function ControlledField() {
  const [value, setValue] = useState("");
  return <TextField aria-label="Model name" value={value} onChange={(event) => setValue(event.currentTarget.value)} />;
}

describe("shadcn-backed native field contracts", () => {
  it("preserves controlled typing and clearing through native change events", () => {
    render(<ControlledField />);
    const field = screen.getByRole("textbox", { name: "Model name" }) as HTMLInputElement;
    fireEvent.change(field, { target: { value: "local-model" } });
    expect(field.value).toBe("local-model");
    fireEvent.change(field, { target: { value: "" } });
    expect(field.value).toBe("");
    expect(field.tagName).toBe("INPUT");
  });

  it("preserves FormField label, descriptions, required state and validation", () => {
    const ref = createRef<HTMLInputElement>();
    render(<><p id="existing-help">Connection settings</p><FormField label="Server URL" description="Use the configured endpoint." error="Enter a valid URL." required><TextField ref={ref} type="url" aria-describedby="existing-help" /></FormField></>);
    const field = screen.getByRole("textbox", { name: /^Server URL/u }) as HTMLInputElement;
    expect(ref.current).toBe(field);
    expect(field.required).toBe(true);
    expect(field.getAttribute("aria-invalid")).toBe("true");
    const descriptions = field.getAttribute("aria-describedby")?.split(" ") ?? [];
    expect(descriptions).toContain("existing-help");
    expect(descriptions.map((id) => document.getElementById(id)?.textContent)).toEqual([
      "Connection settings", "Use the configured endpoint.", "Enter a valid URL.",
    ]);
    field.focus();
    expect(document.activeElement).toBe(field);
  });

  it("keeps password inputs native and does not lose the entered value on visibility changes", () => {
    const ref = createRef<HTMLInputElement>();
    const view = render(<TextField ref={ref} aria-label="API key" name="key" type="password" defaultValue="test-only-key" autoComplete="off" />);
    const field = screen.getByLabelText("API key") as HTMLInputElement;
    expect(ref.current).toBe(field);
    expect(field.type).toBe("password");
    view.rerender(<TextField ref={ref} aria-label="API key" name="key" type="text" defaultValue="test-only-key" autoComplete="off" />);
    expect(ref.current).toBe(field);
    expect(field.type).toBe("text");
    expect(field.value).toBe("test-only-key");
    expect(field.autocomplete).toBe("off");
  });

  it("keeps native select values, refs, optgroups and form submission", () => {
    const ref = createRef<HTMLSelectElement>();
    const change = vi.fn();
    render(<form aria-label="Provider settings"><label>Provider<NativeSelect ref={ref} name="provider" defaultValue="local" onChange={change}><optgroup label="Available"><option value="local">Local</option><option value="remote">Remote</option></optgroup></NativeSelect></label></form>);
    const select = screen.getByRole("combobox", { name: "Provider" }) as HTMLSelectElement;
    expect(ref.current).toBe(select);
    expect(select.tagName).toBe("SELECT");
    fireEvent.change(select, { target: { value: "remote" } });
    expect(change).toHaveBeenCalledTimes(1);
    expect(select.value).toBe("remote");
    expect(new FormData(screen.getByRole("form", { name: "Provider settings" }) as HTMLFormElement).get("provider")).toBe("remote");
    expect(select.querySelector("optgroup")?.label).toBe("Available");
  });

  it("preserves disabled and numeric-size listbox semantics without a false picker affordance", () => {
    const { container } = render(<NativeSelect aria-label="Disabled provider" disabled size={4}><option>Local</option></NativeSelect>);
    const field = screen.getByRole("listbox", { name: "Disabled provider" }) as HTMLSelectElement;
    expect(field.disabled).toBe(true);
    expect(field.size).toBe(4);
    expect(container.querySelector('[data-slot="native-select-icon"]')).toBeNull();
  });
});
