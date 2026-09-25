import { describe, expect, it } from "vitest";
import { sameStudyScope, scopeMembers, selectedScope, studyScopeKey, type MaterialScope } from "./knowledge";

const document: MaterialScope = { kind: "document", id: "a".repeat(32) };
const deck: MaterialScope = { kind: "deck", id: "b".repeat(32) };
const collection: MaterialScope = { kind: "collection", id: "c".repeat(32) };
describe("canonical selected study scope", () => {
  it("uses legacy general and single-source identities without broadening selection", () => {
    expect(selectedScope([])).toEqual({ kind: "general", id: null });
    expect(selectedScope([document, document])).toEqual(document);
    expect(scopeMembers({ kind: "general", id: null })).toEqual([]);
    expect(scopeMembers(document)).toEqual([document]);
    expect(scopeMembers(collection)).toEqual([collection]);
  });
  it("matches the native canonical order regardless of checkbox order", () => {
    const a = selectedScope([document, deck, collection]);
    const b = selectedScope([collection, deck, document, deck]);
    expect(JSON.stringify(a)).toBe(JSON.stringify(b));
    expect(scopeMembers(a)).toEqual([collection, deck, document]);
    expect(sameStudyScope(a, b)).toBe(true);
    expect(studyScopeKey(a)).not.toBe(studyScopeKey(document));
    expect(sameStudyScope(a, { kind: "library", id: null })).toBe(false);
  });
  it("bounds the native authorization payload", () => {
    const members = Array.from({ length: 17 }, (_, i): MaterialScope => ({ kind: "document", id: i.toString(16).padStart(32, "0") }));
    expect(() => selectedScope(members)).toThrow("at most 16");
    expect(JSON.stringify(selectedScope(members.slice(0, 16))).length).toBeLessThanOrEqual(1024);
  });
});
