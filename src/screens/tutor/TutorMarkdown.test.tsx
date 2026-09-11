import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { openExternalUrl } from "../../platform/externalLinks";
import { TutorMarkdown } from "./TutorMarkdown";

vi.mock("../../platform/externalLinks", () => ({ openExternalUrl: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(openExternalUrl).mockResolvedValue(undefined); });

describe("Tutor Markdown", () => {
  it("renders the reported bold headings, nested lists, tables, quotes and fenced code", () => {
    const { container } = render(<TutorMarkdown text={'# Review\n\n**Question 1 — Heart failure / RAAS**\n\n*Consider* the following.\n\n1. First\n   - Nested\n2. Second\n\n| Term | Meaning |\n| --- | --- |\n| A | B |\n\n> Keep qualifications.\n\n```python\nx = "<script>"\n```\n\nUse `code`, ~~not this~~, and A &amp; B.'} />);
    expect(screen.getByRole("heading", { name: "Review" })).toBeTruthy();
    expect(screen.getByText("Question 1 — Heart failure / RAAS").tagName).toBe("STRONG");
    expect(screen.getByText("Consider").tagName).toBe("EM");
    expect(container.querySelector("ol ul li")?.textContent).toContain("Nested");
    expect(screen.getByRole("table")).toBeTruthy();
    expect(container.querySelector("blockquote")?.textContent).toContain("Keep qualifications.");
    expect(container.querySelector("pre code")?.textContent).toBe('x = "<script>"');
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("del")?.textContent).toBe("not this");
    expect(container.textContent).toContain("A & B");
  });

  it("never creates active HTML, resource-fetching images or unsafe links", () => {
    const { container } = render(<TutorMarkdown text={'<script>alert(1)</script>\n\n<img src="https://example.org/tracker" onerror="alert(1)">\n\n![Diagram](https://example.org/image)\n\n[Bad](javascript:alert%281%29) [File](file:///etc/passwd) [Data](data:text/html,x) [Encoded](javascript&#58;alert%281%29)'} />);
    for (const selector of ["script", "img", "iframe", "object", "embed", "a"]) expect(container.querySelector(selector)).toBeNull();
    expect(container.textContent).toContain("Diagram");
    expect(container.textContent).toContain("Bad");
    expect(openExternalUrl).not.toHaveBeenCalled();
  });

  it("opens allowed links only on explicit user activation through the platform adapter", async () => {
    render(<TutorMarkdown text="[Documentation](https://example.org/docs?a=1&amp;b=2)" />);
    const link = screen.getByRole("link", { name: "Documentation" });
    expect(link.getAttribute("href")).toBe("https://example.org/docs?a=1&b=2");
    expect(openExternalUrl).not.toHaveBeenCalled();
    fireEvent.click(link);
    await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith("https://example.org/docs?a=1&b=2"));
  });

  it("reports a failed link open without exposing adapter errors", async () => {
    vi.mocked(openExternalUrl).mockRejectedValue(new Error("Private internal detail"));
    render(<TutorMarkdown text="[Documentation](https://example.org/)" />);
    fireEvent.click(screen.getByRole("link"));
    expect((await screen.findByRole("alert")).textContent).toBe("The link could not be opened.");
    expect(screen.queryByText("Private internal detail")).toBeNull();
  });

  it("preserves literal code and large historic messages instead of truncating them", () => {
    const text = "**" + "x".repeat(25_000) + "**";
    const { container } = render(<TutorMarkdown text={text} />);
    expect(container.textContent).toBe(text);
    expect(container.querySelector("strong")).toBeNull();
  });
});
