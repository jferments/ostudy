import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Badge } from "./Badge";
import { Button } from "./Button";
import { Skeleton } from "./Skeleton";

describe("shadcn-backed oStudy primitives", () => {
  it("keeps native button, ref, form and callback behavior", () => {
    const ref = createRef<HTMLButtonElement>();
    const submit = vi.fn((event) => event.preventDefault());
    const click = vi.fn();
    render(<form onSubmit={submit}><Button ref={ref} onClick={click}>Keep studying</Button><Button type="submit">Save</Button></form>);
    const action = screen.getByRole("button", { name: "Keep studying" });
    expect(ref.current).toBe(action);
    expect(action.tagName).toBe("BUTTON");
    fireEvent.click(action);
    expect(click).toHaveBeenCalledTimes(1);
    expect(submit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(submit).toHaveBeenCalledTimes(1);
  });

  it("retains the action name and blocks repeat writes while loading", () => {
    const click = vi.fn();
    const view = render(<Button loading onClick={click}>Create deck</Button>);
    const button = screen.getByRole("button", { name: "Create deck" }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    fireEvent.click(button);
    expect(click).not.toHaveBeenCalled();
    expect(screen.getByText("Working…").getAttribute("aria-hidden")).toBe("true");
    view.rerender(<Button disabled onClick={click}>Create deck</Button>);
    expect(button.disabled).toBe(true);
    expect(button.hasAttribute("aria-busy")).toBe(false);
    view.rerender(<Button onClick={click}>Create deck</Button>);
    fireEvent.click(button);
    expect(click).toHaveBeenCalledTimes(1);
  });

  it("preserves explicit ARIA names and caller-owned presentation", () => {
    render(<Button loading size="icon" aria-label="Delete conversation" className="caller-owned" data-testid="delete">×</Button>);
    expect(screen.getByRole("button", { name: "Delete conversation" }).classList.contains("caller-owned")).toBe(true);
  });

  it("renders all existing semantic button variants without a vendor API at call sites", () => {
    for (const variant of ["primary", "secondary", "ghost", "danger", "dangerSecondary", "text", "back", "dangerGhost"] as const) {
      const view = render(<Button variant={variant}>{variant}</Button>);
      expect(screen.getByRole("button", { name: variant }).className).not.toContain("undefined");
      view.unmount();
    }
  });

  it("preserves badges as text and skeletons as decorative spans", () => {
    const { container } = render(<><Badge tone="success">Saved</Badge><Skeleton width="45%" height="2rem" radius="1rem" /></>);
    expect(screen.getByText("Saved").tagName).toBe("SPAN");
    const skeleton = container.querySelector('[data-slot="skeleton"]') as HTMLElement;
    expect(skeleton.tagName).toBe("SPAN");
    expect(skeleton.getAttribute("aria-hidden")).toBe("true");
    expect(skeleton.style.width).toBe("45%");
    expect(skeleton.style.height).toBe("2rem");
  });
});
