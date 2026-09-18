import { render, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StudyMarkdown } from "../components/StudyMarkdown";
import { TutorMarkdown } from "../screens/tutor/TutorMarkdown";

const quadratic = String.raw`The $\pm$ means **two solutions**, one using $+$ and one using $-$.

Start with $ax^2 + bx + c = 0$ and divide by $a$.

\[x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}\]

Move the constant and add $\left(\frac{b}{2a}\right)^2$.

| Discriminant | Solutions |
| --- | --- |
| $D > 0$ | two distinct real solutions |
| $D = 0$ | $x = -\frac{b}{2a}$ |
| $D < 0$ | no real solutions |

- Identify $a$, $b$, $c$ **with their signs**.
- The sum is $-b/a$ and the product is $c/a$.

Prices are $5 and $10; code stays literal: \`$x$\`.
`;
// Keep the code span literal without escaping its backticks in the raw fixture.
const fixture = quadratic.replaceAll("\\`", "`");
const sources = [String.raw`\pm`, "+", "-", "ax^2 + bx + c = 0", "a",
  String.raw`x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}`, String.raw`\left(\frac{b}{2a}\right)^2`,
  "D > 0", "D = 0", String.raw`x = -\frac{b}{2a}`, "D < 0", "a", "b", "c", "-b/a", "c/a"];

describe("reported mixed inline/display math", () => {
  it.each(["tutor", "guide"])("typesets the screenshot notation through the shared %s reader", async (surface) => {
    const { container } = render(surface === "tutor" ? <TutorMarkdown text={fixture} /> : <StudyMarkdown text={fixture} />);
    await waitFor(() => expect(Array.from(container.querySelectorAll("math annotation"), (node) => node.textContent)).toEqual(sources));
    expect(container.querySelectorAll("table math")).toHaveLength(4);
    expect(container.querySelector("strong")?.textContent).toBe("two solutions");
    expect(container.querySelector("code")?.textContent).toBe("$x$");
    expect(container.textContent).toContain("Prices are $5 and $10");
    expect(container.querySelectorAll(".katex-display")).toHaveLength(1);
    expect(container.querySelector("p div, script, img")).toBeNull();
  });

  it("rerenders saved/streamed single-dollar formulas without stale output or active content", async () => {
    const { container, rerender } = render(<TutorMarkdown text="Answer $x_1" />);
    expect(container.textContent).toBe("Answer $x_1");
    rerender(<TutorMarkdown text="Answer $x_1$" />);
    await waitFor(() => expect(container.querySelector("math annotation")?.textContent).toBe("x_1"));
    rerender(<TutorMarkdown text={String.raw`$\href{https://example.invalid}{x}$`} />);
    expect(container.querySelector("a,math,img,script")).toBeNull();
    await waitFor(() => expect(container.querySelector("[data-formula-error]")).not.toBeNull());
    rerender(<TutorMarkdown text="Price $10" />);
    expect(container.querySelector("[data-study-math]")).toBeNull();
    expect(container.textContent).toBe("Price $10");
  });
});
