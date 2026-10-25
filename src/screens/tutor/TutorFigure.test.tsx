import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { StudyMarkdown } from "../../components/StudyMarkdown";
import { renderDocumentPageTile } from "../../ipc/documentAnnotations";
import { getTutorFigure } from "../../ipc/tutor";
import { TutorFigure } from "./TutorFigure";
import { withoutFigures } from "./presentation";

vi.mock("../../ipc/tutor", () => ({ getTutorFigure: vi.fn() }));
vi.mock("../../ipc/documentAnnotations", () => ({ renderDocumentPageTile: vi.fn() }));

const documentId = "d".repeat(32);
const turnId = "t".repeat(32);
const answer = `The loop dips into the medulla.\n\n![The nephron](figure:${documentId}/p00003-01)\n\nSo urine concentrates.`;

beforeEach(() => {
  vi.resetAllMocks();
  URL.createObjectURL = vi.fn(() => "blob:figure");
  URL.revokeObjectURL = vi.fn();
  vi.mocked(getTutorFigure).mockResolvedValue({
    documentId, figureId: "p00003-01", title: "Nephron", pageNumber: 3, pageWidth: 600, pageHeight: 800,
    bounds: { left: 100, top: 100, width: 200, height: 150 },
  });
  vi.mocked(renderDocumentPageTile).mockResolvedValue(new ArrayBuffer(8));
});

function Answer({ text }: { text: string }) {
  return (
    <StudyMarkdown
      text={text}
      figure={(reference, title) => <TutorFigure turnId={turnId} reference={reference} title={title} />}
    />
  );
}

describe("Figures in Tutor answers", () => {
  it("draws a figure the answer names from its document's page", async () => {
    render(<Answer text={answer} />);
    const figure = await screen.findByRole("figure", { name: "The nephron" });
    expect(figure.querySelector("img")?.getAttribute("src")).toBe("blob:figure");
    expect(figure.textContent).toContain("page 3");
    expect(getTutorFigure).toHaveBeenCalledWith(turnId, documentId, "p00003-01");
    expect(vi.mocked(renderDocumentPageTile).mock.calls[0]?.[0]).toBe(documentId);
    expect(screen.getByText("So urine concentrates.")).toBeTruthy();
  });

  it("shows nothing for a figure that is no longer available", async () => {
    vi.mocked(getTutorFigure).mockRejectedValue(new Error("This figure is no longer available."));
    render(<Answer text={answer} />);
    await waitFor(() => expect(getTutorFigure).toHaveBeenCalled());
    expect(screen.queryByRole("figure")).toBeNull();
    expect(screen.queryByText("The nephron")).toBeNull();
  });

  it("never fetches other images, and shows a figure's words where it is not drawn", () => {
    render(<StudyMarkdown text={`![photo](https://example.com/a.png) ${answer}`} />);
    expect(screen.getByText(/photo/)).toBeTruthy();
    expect(screen.getByText(/The nephron/)).toBeTruthy();
    expect(getTutorFigure).not.toHaveBeenCalled();
    expect(document.querySelector("img")).toBeNull();
  });

  it("copies an answer without its figures", () => {
    expect(withoutFigures(answer)).toBe("The loop dips into the medulla.\n\nSo urine concentrates.");
  });
});
