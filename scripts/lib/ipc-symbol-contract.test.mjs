import { describe, expect, it } from "vitest";
import { duplicateFunctionNames } from "./ipc-symbol-contract.mjs";

describe("IPC symbol contract helper", () => {
  it("allows distinct Rust function names to share an unregistered wire alias", () => {
    expect(duplicateFunctionNames(new Map([
      ["domain_read", [{ commandName: "read" }]],
      ["leased_read", [{ commandName: "read" }]],
    ]))).toEqual([]);
  });

  it("rejects one Rust function name with multiple command definitions", () => {
    expect(duplicateFunctionNames(new Map([
      ["same_function", [{ commandName: "first" }, { commandName: "second" }]],
    ])).map(([name]) => name)).toEqual(["same_function"]);
  });

  it("rejects duplicate definitions even when their wire names also match", () => {
    expect(duplicateFunctionNames(new Map([
      ["same_function", [{ commandName: "read" }, { commandName: "read" }]],
    ])).map(([name]) => name)).toEqual(["same_function"]);
  });
});
