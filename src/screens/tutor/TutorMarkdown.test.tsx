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

  it("preserves literal character references in inline and fenced code", () => {
    const { container } = render(<TutorMarkdown text={'`&amp; <tag>`\n\n```text\n&amp; <tag>\n```'} />);
    const code = container.querySelectorAll("code");
    expect(code).toHaveLength(2);
    for (const item of code) expect(item.textContent).toBe("&amp; <tag>");
    expect(container.querySelector("tag")).toBeNull();
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


// Exercise the shared renderer through the unchanged Tutor entry point, with
// real KaTeX rather than a renderer stub.
describe("Tutor mathematical Markdown", () => {
  it("typesets saved formulas, preserves surrounding formatting and uses MathML", async () => {
    const { container } = render(<TutorMarkdown text={String.raw`**Before** \(\frac{a}{b}\) after.

\[x^2+y^2=z^2\]

| Symbol | Value |
| --- | --- |
| x | \(\sqrt{2}\) |`} />);
    expect(screen.getByText("Before").tagName).toBe("STRONG");
    await waitFor(() => expect(Array.from(container.querySelectorAll("math annotation"), (node) => node.textContent))
      .toEqual([String.raw`\frac{a}{b}`, "x^2+y^2=z^2", String.raw`\sqrt{2}`]));
    expect(container.querySelectorAll(".katex-display")).toHaveLength(1);
    expect(container.querySelector("p div")).toBeNull();
    expect(openExternalUrl).not.toHaveBeenCalled();
  });

  it("keeps code and prices literal even when neighboring text contains math", async () => {
    const { container } = render(<TutorMarkdown text={'`\\(code\\)` and $5 or $10.\n\n```tex\n$$code$$\n```\n\n\\(x\\)'} />);
    await waitFor(() => expect(container.querySelectorAll("math")).toHaveLength(1));
    expect(container.querySelector("code")?.textContent).toBe(String.raw`\(code\)`);
    expect(container.querySelector("pre code")?.textContent).toBe("$$code$$");
    expect(container.textContent).toContain("$5 or $10.");
  });

  it("updates unfinished and completed replies without retaining stale typesetting", async () => {
    const { container, rerender } = render(<TutorMarkdown text={String.raw`Before \(x_`} />);
    expect(container.textContent).toBe(String.raw`Before \(x_`);
    expect(container.querySelector("math")).toBeNull();
    rerender(<TutorMarkdown text={String.raw`Before \(x_1\) after.`} />);
    await waitFor(() => expect(container.querySelector("math annotation")?.textContent).toBe("x_1"));
    rerender(<TutorMarkdown text={String.raw`Before \(\unknown{x}\) after.`} />);
    expect(container.querySelector("math")).toBeNull();
    await waitFor(() => expect(container.querySelector("[data-formula-error]")).not.toBeNull());
    rerender(<TutorMarkdown text="Replacement reply" />);
    expect(container.querySelector("[data-study-math]")).toBeNull();
    expect(container.textContent).toBe("Replacement reply");
  });

  it("does not turn TeX into network resources, active HTML or cross-message macros", async () => {
    const { container, rerender } = render(<TutorMarkdown text={String.raw`\(\href{https://example.org/leak}{click}\)

\(\includegraphics{https://example.org/image}\)

\(<img src=x onerror=alert(1)>\)

\(\gdef\secret{42}\)`} />);
    await waitFor(() => expect(container.querySelectorAll("[data-formula-error]")).toHaveLength(4));
    expect(container.querySelector("a,img,iframe,script,style,math")).toBeNull();
    expect(openExternalUrl).not.toHaveBeenCalled();
    rerender(<TutorMarkdown text={String.raw`\(\secret\)`} />);
    expect(container.querySelector("math")).toBeNull();
    expect(container.textContent).toBe(String.raw`\(\secret\)`);
  });

  it("preserves Markdown and renders all equations beyond the old 64-formula cliff", async () => {
    const text = "**Dense** " + String.raw`\(x\) `.repeat(65);
    const { container } = render(<TutorMarkdown text={text} />);
    expect(container.querySelector("strong")?.textContent).toBe("Dense");
    await waitFor(() => expect(container.querySelectorAll("math")).toHaveLength(65));
    expect(container.querySelectorAll("math annotation")).toHaveLength(65);
  });
});
