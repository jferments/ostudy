import { describe, expect, it, vi } from "vitest";

import {
  RICH_DOCUMENT_SCHEMA_VERSION,
  editorToolbarGroups,
  type EditorAdapter,
  type EditorCapabilities,
  type EditorCommandRequest,
} from "./adapter";

const capabilities: EditorCapabilities = {
  schemaVersion: RICH_DOCUMENT_SCHEMA_VERSION,
  marks: {
    bold: true,
    italic: true,
    underline: true,
    strike: true,
    inlineCode: true,
    subscript: true,
    superscript: true,
  },
  blocks: {
    headings: true,
    bulletList: true,
    numberedList: true,
    blockquote: true,
    codeBlock: true,
    horizontalRule: true,
  },
  tables: {
    insert: true,
    rows: true,
    columns: true,
    headerRow: true,
    mergeSplit: false,
  },
  checklists: {
    ordinary: true,
    answerKey: true,
    stableItemIds: true,
  },
  history: { undo: true, redo: true },
};

describe("editor-neutral adapter contract", () => {
  it("generates common and advanced toolbar groups only from capabilities", () => {
    const groups = editorToolbarGroups(capabilities);

    expect(groups.map((group) => group.id)).toEqual([
      "text",
      "lists",
      "insert",
      "table",
      "history",
    ]);
    expect(groups.find((group) => group.id === "text")?.advanced).toBe(false);
    expect(groups.find((group) => group.id === "history")?.advanced).toBe(true);
    expect(
      groups.flatMap((group) => group.items).map((entry) => entry.command),
    ).toContain("toggle_subscript");
    expect(
      groups.flatMap((group) => group.items).map((entry) => entry.command),
    ).toContain("insert_answer_checklist");
  });

  it("can be implemented without importing a concrete editor library", () => {
    const execute = vi.fn((_request: EditorCommandRequest) => true);
    const adapter: EditorAdapter = {
      capabilities,
      selection: () => ({
        blockStyle: "paragraph",
        textAlignment: null,
        marks: new Set(),
        inTable: false,
        inTaskList: false,
        taskChecked: null,
      }),
      document: () => ({ type: "doc", attrs: { schemaVersion: 2 }, content: [] }),
      plainText: () => "",
      attributes: () => ({}),
      isActive: () => false,
      execute,
      canExecute: () => true,
      setDocument: vi.fn(),
      setEditable: vi.fn(),
      focus: vi.fn(),
    };

    adapter.execute({ command: "toggle_superscript" });
    expect(execute).toHaveBeenCalledWith({ command: "toggle_superscript" });
    expect(adapter.capabilities.schemaVersion).toBe(2);
  });

  it("does not advertise unsupported merge and split operations", () => {
    expect(capabilities.tables.mergeSplit).toBe(false);
    expect(
      editorToolbarGroups(capabilities)
        .flatMap((group) => group.items)
        .some((item) => /merge|split/iu.test(item.label)),
    ).toBe(false);
  });
});
