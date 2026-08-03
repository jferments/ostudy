import { Chart as ChartJS } from "chart.js";
import { describe, expect, it } from "vitest";

// Importing the component performs the explicit tree-shaken Chart.js
// registration used by the production analytics screen.
import "./ProgressChart";

describe("ProgressChart Chart.js registration", () => {
  it("registers both controllers required by the mixed progress chart", () => {
    expect(ChartJS.registry.getController("bar").id).toBe("bar");
    expect(ChartJS.registry.getController("line").id).toBe("line");
  });
});
