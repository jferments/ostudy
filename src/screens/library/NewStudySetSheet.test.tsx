import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { StagedExternalFile } from "../../ipc/externalFiles";
import * as sourceNotesIpc from "../../ipc/sourceNotes";
import * as subjectsIpc from "../../ipc/subjects";
import * as files from "../../platform/files";
import { makeSubject, makeSubjectLibrary } from "../../test/factories";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { takeStudySet } from "../documents/pendingStudySets";
import { NewStudySetSheet } from "./NewStudySetSheet";

const jobs = { importStagedDocument: vi.fn() };
vi.mock("../../controllers/DocumentJobsProvider", () => ({ useDocumentJobs: () => jobs }));
vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/sourceNotes", () => ({ SOURCE_NOTES_PROMPT_MAX_CHARS: 4000, writeSourceNotes: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  createSubject: vi.fn(),
  moveDocumentToSubject: vi.fn(),
}));
vi.mock("../../platform/files", () => ({ selectExternalImport: vi.fn() }));

const physiology = makeSubject({ id: "p".repeat(32), name: "Physiology" });
const created = makeSubject({ id: "n".repeat(32), name: "Krebs cycle" });
const before = makeSubjectLibrary({ libraryRevision: 3, documentFilingRevision: 5, subjects: [physiology] });
const after = makeSubjectLibrary({ libraryRevision: 4, documentFilingRevision: 5, subjects: [physiology, created] });

function staged(id: string, displayName: string): StagedExternalFile {
  return { id, displayName, purpose: "document", detectedKind: "pdf", mediaType: "application/pdf", byteSize: 10, sha256: "x", expiresAt: 1 };
}

function renderSheet(subjectId?: string) {
  const onCreated = vi.fn();
  render(
    <OverlayProvider>
      <NewStudySetSheet subjects={[physiology]} subjectId={subjectId} onClose={vi.fn()} onCreated={onCreated} />
    </OverlayProvider>,
  );
  return { onCreated, create: () => screen.getByRole("button", { name: /Create study set/u }) };
}

describe("NewStudySetSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(before);
    vi.mocked(subjectsIpc.createSubject).mockResolvedValue(after);
    vi.mocked(subjectsIpc.moveDocumentToSubject).mockResolvedValue(after);
    jobs.importStagedDocument.mockImplementation(async (_file: StagedExternalFile, _instructions: string, filed: (id: string) => Promise<unknown>) => {
      await filed("d".repeat(32));
      return "d".repeat(32);
    });
  });

  it("makes a whole set from a prompt alone in a new subject", async () => {
    vi.mocked(sourceNotesIpc.writeSourceNotes).mockResolvedValue(staged("s".repeat(32), "Krebs cycle.pdf"));
    const { onCreated, create } = renderSheet();
    expect(create().hasAttribute("disabled")).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Krebs cycle" } });
    fireEvent.change(screen.getByRole("textbox", { name: /What to learn/u }), { target: { value: "The Krebs cycle for my exam" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Podcast" }));
    fireEvent.click(create());

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(created.id, after));
    expect(subjectsIpc.createSubject).toHaveBeenCalledWith({ name: "Krebs cycle", expectedLibraryRevision: 3 });
    expect(sourceNotesIpc.writeSourceNotes).toHaveBeenCalledWith("The Krebs cycle for my exam");
    expect(jobs.importStagedDocument).toHaveBeenCalledWith(
      expect.objectContaining({ id: "s".repeat(32) }), "The Krebs cycle for my exam", expect.any(Function),
    );
    expect(subjectsIpc.moveDocumentToSubject).toHaveBeenCalledWith({
      documentId: "d".repeat(32), subjectId: created.id, expectedDocumentFilingRevision: 5,
    });
    expect(takeStudySet(created.id)).toEqual({
      flashcards: true, studyGuide: true, lessons: true, podcast: true, documentIds: ["d".repeat(32)],
    });
  });

  it("adds chosen documents to an existing subject without writing notes", async () => {
    vi.mocked(files.selectExternalImport).mockResolvedValueOnce(staged("a".repeat(32), "Chapter 4.pdf"));
    const { onCreated, create } = renderSheet(physiology.id);
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Add a document/u }));
    expect(await screen.findByText("Chapter 4.pdf")).toBeTruthy();
    // A podcast is made from the lessons, so it goes with them.
    fireEvent.click(screen.getByRole("checkbox", { name: "Lessons" }));
    expect(screen.getByRole("checkbox", { name: "Podcast" }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(create());

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(physiology.id, null));
    expect(subjectsIpc.createSubject).not.toHaveBeenCalled();
    expect(sourceNotesIpc.writeSourceNotes).not.toHaveBeenCalled();
    expect(jobs.importStagedDocument).toHaveBeenCalledWith(expect.objectContaining({ id: "a".repeat(32) }), undefined, expect.any(Function));
    expect(takeStudySet(physiology.id)).toEqual(expect.objectContaining({ lessons: false, podcast: false }));
  });

  it("keeps the sheet open with the reason when the notes cannot be written", async () => {
    vi.mocked(sourceNotesIpc.writeSourceNotes).mockRejectedValue(new Error("Choose an AI model first."));
    const { onCreated, create } = renderSheet(physiology.id);
    fireEvent.change(screen.getByRole("textbox", { name: /What to learn/u }), { target: { value: "Renal clearance" } });
    fireEvent.click(create());
    expect(await screen.findByText("Choose an AI model first.")).toBeTruthy();
    expect(onCreated).not.toHaveBeenCalled();
    expect(jobs.importStagedDocument).not.toHaveBeenCalled();
  });
});
