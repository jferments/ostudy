import { Chart as ChartJS } from "chart.js";
import { describe, expect, it } from "vitest";

// Importing the component performs the explicit tree-shaken Chart.js
// registration used by the production analytics screen.
import "./ProgressChart";

describe("ProgressChart Chart.js registration", () => {
  it("registers both controllers required by the mixed progress chart", () => {
    // Chart.js throws the exact runtime error reported by the Analytics error
    // boundary when a requested controller has not been registered.
    expect(() => ChartJS.registry.getController("bar")).not.toThrow();
    expect(() => ChartJS.registry.getController("line")).not.toThrow();
  });
});
