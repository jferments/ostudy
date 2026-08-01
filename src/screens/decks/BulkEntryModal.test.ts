import { describe, expect, it } from "vitest";

import { parseBulkCards } from "./BulkEntryModal";

describe("parseBulkCards", () => {
  it("parses tab and double-colon rows with optional tags", () => {
    const parsed = parseBulkCards(
      "Question one\tAnswer one\talpha, beta\nQuestion two :: Answer two :: gamma",
    );

    expect(parsed.errors).toEqual([]);
    expect(parsed.rows).toEqual([
      {
        frontText: "Question one",
        backText: "Answer one",
        tags: ["alpha", "beta"],
      },
      {
        frontText: "Question two",
        backText: "Answer two",
        tags: ["gamma"],
      },
    ]);
  });

  it("reports malformed rows instead of silently importing them", () => {
    const parsed = parseBulkCards("No separator here\nQuestion only ::   ");
    expect(parsed.rows).toEqual([]);
    expect(parsed.errors).toHaveLength(2);
  });
});
