import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { IconButton } from "./IconButton";

describe("IconButton", () => {
  afterEach(() => {
    delete document.documentElement.dataset.motion;
    delete document.documentElement.dataset.theme;
  });

  it("requires a nonblank accessible label", () => {
    expect(() => render(<IconButton icon="search" label="   " />)).toThrow(
      "IconButton requires a nonblank accessible label.",
    );
  });

  it("uses its label as the accessible name and tooltip fallback", () => {
    render(<IconButton icon="search" label="Search flashcards" />);
    const button = screen.getByRole("button", { name: "Search flashcards" });
    expect(button.getAttribute("type")).toBe("button");
    expect(button.hasAttribute("title")).toBe(false);

    fireEvent.focus(button);
    expect(screen.getByRole("tooltip").textContent).toBe("Search flashcards");
  });

  it("can suppress visible tooltip copy without removing the accessible name", () => {
    render(<IconButton icon="close" label="Close study guide" tooltip={false} />);
    const button = screen.getByRole("button", { name: "Close study guide" });

    fireEvent.focus(button);
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(button.getAttribute("aria-label")).toBe("Close study guide");
  });

  it("supports custom tooltip copy, forwarding refs, and activation", () => {
    const ref = createRef<HTMLButtonElement>();
    const onClick = vi.fn();
    render(
      <IconButton
        ref={ref}
        icon="edit"
        label="Edit deck name and description"
        tooltip="Edit deck"
        onClick={onClick}
      />,
    );

    expect(ref.current).not.toBeNull();
    fireEvent.focus(ref.current as HTMLButtonElement);
    expect(screen.getByRole("tooltip").textContent).toBe("Edit deck");
    fireEvent.click(ref.current as HTMLButtonElement);
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("keeps its accessible name and geometry content while loading", () => {
    render(<IconButton icon="delete" label="Delete deck" tone="danger" loading />);
    const button = screen.getByRole("button", { name: "Delete deck" });

    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button.getAttribute("data-tone")).toBe("danger");
    expect(button.querySelector("svg")).not.toBeNull();
  });

  it("preserves an explicit disabled state without depending on a tooltip", () => {
    render(<IconButton icon="schedule" label="Schedule review" disabled />);
    const button = screen.getByRole("button", { name: "Schedule review" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(button.hasAttribute("aria-busy")).toBe(false);
  });

  it.each(["light", "dark", "console", "synthwave"])(
    "keeps the same semantic control contract in the %s theme",
    (theme) => {
      document.documentElement.dataset.theme = theme;
      render(<IconButton icon="information" label="Information" />);
      expect(screen.getByRole("button", { name: "Information" })).toBeTruthy();
    },
  );

  it.each(["reduced", "none"])(
    "keeps loading semantics for the nonanimated %s-motion fallback",
    (motion) => {
      document.documentElement.dataset.motion = motion;
      render(<IconButton icon="restore" label="Restore" loading />);
      expect(screen.getByRole("button", { name: "Restore" }).getAttribute("aria-busy")).toBe(
        "true",
      );
    },
  );
});
