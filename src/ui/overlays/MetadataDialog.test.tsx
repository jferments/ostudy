import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { OverlayProvider } from "./OverlayProvider";
import { InfoButton, MetadataItem, MetadataList } from "./MetadataDialog";

const originalMatchMedia = window.matchMedia;
afterEach(() => {
  window.matchMedia = originalMatchMedia;
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.motion;
});

function renderInfo(mode: "wide" | "compact" = "wide") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query.includes("760") ? mode === "compact" : query.includes("1180") ? mode === "compact" : false,
    media: query,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn(),
  }));
  render(
    <LayoutModeProvider><OverlayProvider>
      <InfoButton label="View details" title="Operational details" description="Local information.">
        <MetadataList><MetadataItem label="Version" value="0.1.0" /></MetadataList>
      </InfoButton>
    </OverlayProvider></LayoutModeProvider>,
  );
}

describe("MetadataDialog and InfoButton", () => {
  it("labels the dialog, focuses its close control, and restores focus after Escape", async () => {
    renderInfo();
    const opener = screen.getByRole("button", { name: "View details" });
    opener.focus();
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog", { name: "Operational details" });
    expect(dialog.getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByText("0.1.0")).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close dialog" })));
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(opener));
  });

  it("uses compact sheet presentation and backdrop dismissal", async () => {
    renderInfo("compact");
    const opener = screen.getByRole("button", { name: "View details" });
    fireEvent.click(opener);
    const dialog = await screen.findByRole("dialog", { name: "Operational details" });
    expect(dialog.getAttribute("data-presentation")).toBe("sheet");
    fireEvent.mouseDown(dialog);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it.each(["light", "dark", "console", "synthwave"])("preserves metadata semantics in the %s theme", (theme) => {
    document.documentElement.dataset.theme = theme;
    renderInfo();
    fireEvent.click(screen.getByRole("button", { name: "View details" }));
    expect(screen.getByText("Version").tagName).toBe("DT");
  });

  it("preserves semantics with reduced motion", () => {
    document.documentElement.dataset.motion = "reduced";
    renderInfo();
    fireEvent.click(screen.getByRole("button", { name: "View details" }));
    expect(screen.getByRole("dialog", { name: "Operational details" })).toBeTruthy();
  });
});
