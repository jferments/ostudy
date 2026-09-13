import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { IconButton } from "../icons/IconButton";
import { Badge } from "./Badge";
import { Button } from "./Button";
import { ButtonGroup } from "./ButtonGroup";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "./Card";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "./Item";
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

  it("preserves explicit IconButton sizes instead of letting the vendor button shrink them", () => {
    const { container } = render(
      <IconButton icon="folder-open" iconSize={24} label="Import deck from file" tooltip={false} />,
    );
    const icon = container.querySelector("svg") as SVGElement;
    expect(icon.getAttribute("width")).toBe("24");
    expect(icon.getAttribute("height")).toBe("24");
    expect(icon.classList.contains("tw:size-6")).toBe(true);
  });

  it("preserves semantic application button intents while the vendor layer uses stock variants", () => {
    for (const variant of ["primary", "secondary", "ghost", "danger", "dangerSecondary", "text", "back", "dangerGhost"] as const) {
      const view = render(<Button variant={variant}>{variant}</Button>);
      expect(screen.getByRole("button", { name: variant }).className).not.toContain("undefined");
      view.unmount();
    }
  });

  it("exposes stock card composition through the application boundary", () => {
    render(
      <Card as="article" size="sm">
        <CardHeader>
          <CardTitle>Card title</CardTitle>
          <CardDescription>Card description</CardDescription>
          <CardAction><Button size="small">Open</Button></CardAction>
        </CardHeader>
        <CardContent>Body</CardContent>
      </Card>,
    );
    expect(screen.getByText("Card title").getAttribute("data-slot")).toBe("card-title");
    expect(screen.getByText("Card description").getAttribute("data-slot")).toBe("card-description");
    expect(screen.getByRole("button", { name: "Open" })).toBeTruthy();
  });

  it("provides stock item and button-group composition without exposing vendor imports", () => {
    render(
      <>
        <Item as="article" variant="outline">
          <ItemContent><ItemTitle>Document</ItemTitle><ItemDescription>Ready to study</ItemDescription></ItemContent>
          <ItemActions><Button size="small">Open document</Button></ItemActions>
        </Item>
        <ButtonGroup aria-label="Formatting"><Button size="small">Bold</Button><Button size="small">Italic</Button></ButtonGroup>
      </>,
    );
    expect(screen.getByText("Document").getAttribute("data-slot")).toBe("item-title");
    expect(screen.getByRole("group", { name: "Formatting" })).toBeTruthy();
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
