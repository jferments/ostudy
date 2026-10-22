import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  findDocumentFigures, getDocumentFigures, readDocumentFigures, setDocumentFigureHidden, setDocumentFiguresUseImages,
} from "./documentFigures";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);

describe("document figure IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("reads and finds a document's figures by its id alone", async () => {
    await getDocumentFigures(documentId);
    expect(invoke).toHaveBeenCalledWith("get_document_figures", { input: { documentId } });
    await findDocumentFigures(documentId);
    expect(invoke).toHaveBeenCalledWith("find_document_figures", { input: { documentId } });
    await readDocumentFigures(documentId);
    expect(invoke).toHaveBeenCalledWith("read_document_figures", { input: { documentId } });
  });

  it("saves the learner's choices as whole states, so a repeated request is safe", async () => {
    await setDocumentFiguresUseImages(documentId, false);
    expect(invoke).toHaveBeenCalledWith("set_document_figures_use_images", {
      input: { documentId, useImages: false },
    });
    await setDocumentFigureHidden(documentId, "p00002-01", true);
    expect(invoke).toHaveBeenCalledWith("set_document_figure_hidden", {
      input: { documentId, figureId: "p00002-01", hidden: true },
    });
  });
});
