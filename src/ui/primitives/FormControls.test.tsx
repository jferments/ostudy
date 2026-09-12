import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Checkbox, Disclosure, RadioGroup, Slider, Switch, Textarea } from "./FormControls";

function ToggleHarness() {
  const [checked, setChecked] = useState(false);
  return <label>Include documents<Checkbox checked={checked} onCheckedChange={setChecked} name="documents" value="yes" /></label>;
}

describe("application form controls", () => {
  it("associates a checkbox with its label and toggles a controlled value", async () => {
    render(<ToggleHarness />);
    const control = screen.getByRole("checkbox", { name: "Include documents" });
    expect(control.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(control);
    await waitFor(() => expect(control.getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(control);
    await waitFor(() => expect(control.getAttribute("aria-checked")).toBe("false"));
  });
  it("preserves indeterminate state, native form values and input refs", () => {
    const inputRef = createRef<HTMLInputElement>();
    const { container } = render(<form><Checkbox aria-label="Select group" indeterminate inputRef={inputRef} /><Checkbox aria-label="Included" name="enabled" value="yes" defaultChecked /></form>);
    expect(screen.getByRole("checkbox", { name: "Select group" }).getAttribute("aria-checked")).toBe("mixed");
    expect(inputRef.current?.type).toBe("checkbox");
    expect(new FormData(container.querySelector("form")!).get("enabled")).toBe("yes");
  });
  it("does not mutate disabled toggles", () => {
    const changed = vi.fn();
    render(<><Checkbox aria-label="Locked checkbox" disabled onCheckedChange={changed} /><Switch aria-label="Locked switch" disabled onCheckedChange={changed} /></>);
    fireEvent.click(screen.getByRole("checkbox", { name: "Locked checkbox" }));
    fireEvent.click(screen.getByRole("switch", { name: "Locked switch" }));
    expect(changed).not.toHaveBeenCalled();
  });
  it("submits the selected radio value without coupling callers to a vendor event", async () => {
    const changed = vi.fn();
    render(<form><RadioGroup label="Credential lifetime" name="storage" value="session" onValueChange={changed} options={[{ value: "session", label: "Session only" }, { value: "secure", label: "Save securely" }]} /></form>);
    fireEvent.click(screen.getByRole("radio", { name: "Save securely" }));
    await waitFor(() => expect(changed).toHaveBeenCalledWith("secure"));
  });
  it("uses one labelled slider thumb and handles keyboard changes", async () => {
    const changed = vi.fn();
    render(<Slider label="Interface scale" value={100} min={80} max={140} step={5} onValueChange={changed} />);
    const thumb = screen.getByRole("slider", { name: "Interface scale" });
    expect(thumb.getAttribute("aria-valuenow")).toBe("100");
    fireEvent.keyDown(thumb, { key: "ArrowRight" });
    await waitFor(() => expect(changed).toHaveBeenCalledWith(105));
  });
  it("retains a disclosure's draft while its content is collapsed", async () => {
    const ref = createRef<HTMLTextAreaElement>();
    render(<Disclosure title="Advanced settings"><label>Draft<Textarea ref={ref} defaultValue="Keep this" /></label></Disclosure>);
    const trigger = screen.getByRole("button", { name: "Advanced settings" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(trigger);
    await waitFor(() => expect(trigger.getAttribute("aria-expanded")).toBe("true"));
    fireEvent.change(screen.getByRole("textbox", { name: "Draft" }), { target: { value: "Edited" } });
    fireEvent.click(trigger);
    fireEvent.click(trigger);
    expect(ref.current?.value).toBe("Edited");
  });
});
