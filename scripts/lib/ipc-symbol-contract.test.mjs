import assert from "node:assert/strict";

// Tauri 2 generates its exported helper macros from Rust function identifiers.
// Wire aliases are separately unique among registered (not all defined) commands.
export function duplicateFunctionNames(definitionsByFunction) {
  return [...definitionsByFunction].filter(([, definitions]) => definitions.length > 1);
}

assert.deepEqual(duplicateFunctionNames(new Map([
  ["domain_read", [{ commandName: "read" }]],
  ["leased_read", [{ commandName: "read" }]],
])), []);
assert.deepEqual(duplicateFunctionNames(new Map([
  ["same_function", [{ commandName: "first" }, { commandName: "second" }]],
])).map(([name]) => name), ["same_function"]);
assert.deepEqual(duplicateFunctionNames(new Map([
  ["same_function", [{ commandName: "read" }, { commandName: "read" }]],
])).map(([name]) => name), ["same_function"]);
