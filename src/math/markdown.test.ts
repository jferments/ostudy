import { Lexer, Marked } from "marked";
import { describe, expect, it } from "vitest";
import { isMarkdownFormula, lexStudyMarkdown, MAX_MARKDOWN_FORMULAS, type MarkdownFormula } from "./markdown";

function formulas(text: string, inline = false): MarkdownFormula[] {
  const result: MarkdownFormula[] = [];
  const tokens = lexStudyMarkdown(text, inline);
  expect(tokens).not.toBeNull();
  new Marked().walkTokens(tokens!, (token) => { if (isMarkdownFormula(token)) result.push(token); });
  return result;
}

describe("bounded mathematical Markdown tokens", () => {
  it("captures inline, display and multiline TeX before Markdown escaping", () => {
    const matrix = String.raw`\begin{pmatrix}a&b\\c&d\end{pmatrix}`;
    const text = String.raw`Before \(a_b * c\) after.` + `\n\n\\[\n${matrix}\n\\]\n\n$$x^2$$`;
    expect(formulas(text).map(({ source, block, complete }) => ({ source, block, complete }))).toEqual([
      { source: "a_b * c", block: false, complete: true },
      { source: `\n${matrix}\n`, block: true, complete: true },
      { source: "x^2", block: true, complete: true },
    ]);
  });

  it("preserves code, escaped delimiters and single-dollar currency as literals", () => {
    const text = '`\\(code\\)`\n\n```tex\n\\[code\\]\n$$code$$\n```\n\n    \\[indented\\]\n\n'
      + String.raw`Escaped \\(not math\\). $5 and $10, or $variable$, are text.`;
    expect(formulas(text)).toEqual([]);
  });

  it("does not tokenize raw HTML code or script content", () => {
    expect(formulas(String.raw`Before <code>\(literal\)</code> after.`)).toEqual([]);
    expect(formulas('<script>\\(literal\\)</script>\n\n<pre>$$literal$$</pre>')).toEqual([]);
  });

  it("preserves unfinished streamed formulas instead of consuming their backslashes", () => {
    const partial = String.raw`\(a_b + **unfinished`;
    expect(formulas(`Before ${partial}`)).toMatchObject([{ raw: partial, complete: false }]);
    expect(formulas(`${partial}\\)`)).toMatchObject([{ source: "a_b + **unfinished", complete: true }]);
  });

  it("keeps multiplication and subscripts inside surrounding Markdown emphasis", () => {
    expect(formulas(String.raw`*Before \(a*b\) after* and **\(a_b\)** and ~~\(x~y\)~~`)
      .map((token) => token.source)).toEqual(["a*b", "a_b", "x~y"]);
  });

  it("finds equations in lists, quotes and table cells without altering link destinations", () => {
    const text = '- First \\(x\\)\n\n> Then \\(y\\)\n\n| Term | Value |\n| --- | --- |\n| A | \\(z\\) |\n\n[Link](https://example.org/target "\\(title\\)")';
    expect(formulas(text).map((token) => token.source)).toEqual(["x", "y", "z"]);
  });

  it("keeps escaped closing delimiters in the mathematical source", () => {
    expect(formulas(String.raw`\(x\\) + y\)`)).toMatchObject([{ source: String.raw`x\\) + y` }]);
  });

  it("bounds count and total UTF-8 work, with independent budgets and no global registration", () => {
    const atLimit = String.raw`\(x\) `.repeat(MAX_MARKDOWN_FORMULAS);
    expect(formulas(atLimit)).toHaveLength(MAX_MARKDOWN_FORMULAS);
    expect(lexStudyMarkdown(atLimit + String.raw`\(y\)`)).toBeNull();
    expect(lexStudyMarkdown(`\\(${'α'.repeat(6000)}\\) `.repeat(3))).toBeNull();
    expect(lexStudyMarkdown("x".repeat(24_001))).toBeNull();
    expect(formulas(String.raw`\(x\)`)).toHaveLength(1);
    expect(JSON.stringify(Lexer.lex(String.raw`\(x\)`))).not.toContain("ostudyFormula");
  });

  it("supports inline-only labels without creating heading or paragraph blocks", () => {
    const tokens = lexStudyMarkdown(String.raw`# Label \(x\)`, true)!;
    expect(tokens.some((token) => ["heading", "paragraph"].includes(token.type))).toBe(false);
    expect(formulas(String.raw`# Label \(x\)`, true)).toHaveLength(1);
  });
});
