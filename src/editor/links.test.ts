import { describe, expect, it } from "vitest";

import { isAllowedExternalLink, normalizeExternalLink } from "./links";

describe("external card links", () => {
  it("adds https only when the user omitted an explicit scheme", () => {
    expect(normalizeExternalLink("  example.org/path  ")).toBe(
      "https://example.org/path",
    );
    expect(normalizeExternalLink("mailto:nurse@example.org")).toBe(
      "mailto:nurse@example.org",
    );
    expect(normalizeExternalLink("javascript:alert(1)")).toBe(
      "javascript:alert(1)",
    );
    expect(normalizeExternalLink("   ")).toBe("");
  });

  it.each([
    "https://example.org",
    "http://localhost:3000/path?query=1#section",
    "mailto:nurse@example.org",
    "tel:+12065550123",
  ])("accepts the supported absolute address %s", (address) => {
    expect(isAllowedExternalLink(address)).toBe(true);
  });

  it.each([
    "",
    "example.org",
    "https://",
    "https:///missing-host",
    "javascript:alert(1)",
    "file:///tmp/card.txt",
    "data:text/html,unsafe",
    "mailto:",
    "tel:",
    " https://example.org",
    "https://example.org ",
    "https://exam ple.org",
    "https://example.org\nnext",
  ])("rejects the unsafe or incomplete address %s", (address) => {
    expect(isAllowedExternalLink(address)).toBe(false);
  });

  it("rejects addresses beyond the durable content limit", () => {
    expect(isAllowedExternalLink(`https://example.org/${"a".repeat(2_000)}`)).toBe(
      false,
    );
  });
});
