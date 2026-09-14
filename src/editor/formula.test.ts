import { describe, expect, it } from "vitest";

import { formulaSourceError, MAX_FORMULA_SOURCE_BYTES } from "./formula";

describe("formula source admission", () => {
  it("bounds UTF-8 bytes rather than UTF-16 code units", () => {
    expect(formulaSourceError("x".repeat(MAX_FORMULA_SOURCE_BYTES))).toBeNull();
    expect(formulaSourceError("x".repeat(MAX_FORMULA_SOURCE_BYTES + 1))).toMatch(/UTF-8/u);
    expect(formulaSourceError("α".repeat(MAX_FORMULA_SOURCE_BYTES / 2))).toBeNull();
    expect(formulaSourceError("α".repeat(MAX_FORMULA_SOURCE_BYTES / 2 + 1))).toMatch(/UTF-8/u);
    expect(formulaSourceError("😀".repeat(MAX_FORMULA_SOURCE_BYTES / 4))).toBeNull();
    expect(formulaSourceError("😀".repeat(MAX_FORMULA_SOURCE_BYTES / 4 + 1))).toMatch(/UTF-8/u);
  });

  it("rejects empty, control and malformed Unicode input without normalizing source", () => {
    for (const source of [null, 4, "", " \t\n"]) expect(formulaSourceError(source)).not.toBeNull();
    for (const source of ["x\0y", "x\u007fy", "x\u0085y", "x\u009fy"]) {
      expect(formulaSourceError(source)).toMatch(/control/u);
    }
    expect(formulaSourceError("\ud800")).toMatch(/Unicode/u);
    expect(formulaSourceError("\udc00")).toMatch(/Unicode/u);
    expect(formulaSourceError(" x\t+\ny\r ")).toBeNull();
    // Admission is deliberately not a TeX interpreter or typesetting parser.
    expect(formulaSourceError("\\unknown{x}")).toBeNull();
  });
});
