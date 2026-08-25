import { describe, expect, it } from "vitest";

import { ownedClassNames } from "./classOwnership";

describe("ownedClassNames", () => {
  it("resolves every matching owner and preserves explicit module classes", () => {
    const shared = { panel: "shared_panel", selected: "shared_selected" };
    const feature = { panel: "feature_panel", row: "feature_row" };

    expect(
      ownedClassNames([shared, feature], "panel row", "module_hash", false, null),
    ).toBe("shared_panel feature_panel feature_row module_hash");
  });

  it("deduplicates classes emitted by overlapping owners", () => {
    const first = { selected: "selected_hash", row: "row_hash" };
    const second = { selected: "selected_hash" };

    expect(ownedClassNames([first, second], "selected row selected")).toBe(
      "selected_hash row_hash",
    );
  });
});
