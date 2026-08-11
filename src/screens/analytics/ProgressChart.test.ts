import { Chart as ChartJS } from "chart.js";
import { describe, expect, it } from "vitest";

import "./ProgressChart";

describe("ProgressChart Chart.js registration", () => {
  it("registers the line controller used by the progress chart", () => {
    expect(() => ChartJS.registry.getController("line")).not.toThrow();
  });
});
