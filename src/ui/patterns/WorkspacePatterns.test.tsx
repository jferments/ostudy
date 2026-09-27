import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import { PageScaffold } from "./WorkspacePatterns";

describe("PageScaffold", () => {
  it("owns the outer page gutter but suppresses a second gutter for nested feature scaffolds", () => {
    const { container } = render(
      <PageScaffold>
        <PageScaffold width="wide"><span>Nested content</span></PageScaffold>
      </PageScaffold>,
    );
    const scaffolds = container.querySelectorAll('[data-page-scaffold="true"]');
    expect(scaffolds).toHaveLength(2);
    expect(required(scaffolds[0], "page scaffold 1").className).toContain("tw:px-page");
    expect(required(scaffolds[0], "page scaffold 1").className).not.toContain("tw:p-0");
    expect(required(scaffolds[1], "page scaffold 2").className).toContain("tw:p-0");
    expect(required(scaffolds[1], "page scaffold 2").className).not.toContain("tw:px-page");
    expect(screen.getByText("Nested content")).toBeTruthy();
  });

  it("allows an explicitly inset nested scaffold when a workflow needs its own inner page", () => {
    const { container } = render(
      <PageScaffold>
        <PageScaffold inset><span>Inset nested content</span></PageScaffold>
      </PageScaffold>,
    );
    const scaffolds = container.querySelectorAll('[data-page-scaffold="true"]');
    expect(required(scaffolds[1], "page scaffold 2").className).toContain("tw:px-page");
  });
});
