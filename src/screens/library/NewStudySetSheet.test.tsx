import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DocumentSummary } from "../../ipc/documents";
import type { StagedExternalFile } from "../../ipc/externalFiles";
import * as sourceNotesIpc from "../../ipc/sourceNotes";
import * as subjectsIpc from "../../ipc/subjects";
import * as files from "../../platform/files";
import { makeSubject, makeSubjectLibrary } from "../../test/factories";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { takeStudySet } from "../documents/pendingStudySets";
import { NewStudySetSheet } from "./NewStudySetSheet";

const jobs = { importStagedDocument: vi.fn(), retry: vi.fn(), documents: [] as DocumentSummary[] };
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
vi.mock("../../platform/files", () => ({ selectExternalImports: vi.fn() }));
vi.mock("../../ipc/settings", () => ({ getSettings: vi.fn(async () => ({ lectureAudioWithNewLessons: true })) }));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn(async () => ({ profileId: "voice" })) }));

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
  const label = subjectId === undefined ? "Make study set" : "Add materials";
  return { onCreated, create: () => screen.getByRole("button", { name: label }) };
}

describe("NewStudySetSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(before);
    vi.mocked(subjectsIpc.createSubject).mockResolvedValue(after);
    vi.mocked(subjectsIpc.moveDocumentToSubject).mockResolvedValue(after);
    jobs.documents = [];
    jobs.importStagedDocument.mockImplementation(async (_file: StagedExternalFile, _instructions: string, filed: (id: string) => Promise<unknown>) => {
      await filed("d".repeat(32));
      return "d".repeat(32);
    });
  });

  it("makes a whole set from a prompt alone in a new subject", async () => {
    vi.mocked(sourceNotesIpc.writeSourceNotes).mockResolvedValue(staged("s".repeat(32), "Krebs cycle.pdf"));
    const { onCreated, create } = renderSheet();
    expect(create().hasAttribute("disabled")).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Subject" }), { target: { value: "Krebs cycle" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt" }), { target: { value: "The Krebs cycle for my exam" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Podcast" }));
    // Lecture audio starts on, as the setting says, since a voice is set up.
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Lecture audio" }).getAttribute("aria-checked")).toBe("true"));
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
      flashcards: true, studyGuide: true, lessons: true, podcast: true, lectureAudio: true, documentIds: ["d".repeat(32)],
    });
  });

  it("makes an empty subject from a name alone", async () => {
    const { onCreated, create } = renderSheet();
    expect(screen.queryByRole("radiogroup")).toBeNull();
    fireEvent.change(screen.getByRole("textbox", { name: "Subject" }), { target: { value: "Krebs cycle" } });
    fireEvent.click(create());
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(created.id, after));
    expect(sourceNotesIpc.writeSourceNotes).not.toHaveBeenCalled();
    expect(jobs.importStagedDocument).not.toHaveBeenCalled();
    expect(takeStudySet(created.id)).toBeNull();
  });

  it("adds nothing to an existing subject until it has documents or a prompt", () => {
    const { create } = renderSheet(physiology.id);
    expect(screen.getByRole("dialog", { name: "Add to Physiology" })).toBeTruthy();
    expect(create().hasAttribute("disabled")).toBe(true);
  });

  it("adds chosen documents to an existing subject without writing notes", async () => {
    vi.mocked(files.selectExternalImports).mockResolvedValueOnce({
      files: [staged("a".repeat(32), "Chapter 4.pdf"), staged("b".repeat(32), "Chapter 5.pdf")],
      skipped: [{ displayName: "notes.txt", message: "This kind of file cannot be imported." }],
    });
    const { onCreated, create } = renderSheet(physiology.id);
    expect(screen.queryByRole("textbox", { name: "Subject" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Choose files/u }));
    expect(await screen.findByText("Chapter 4.pdf")).toBeTruthy();
    expect(screen.getByText("Chapter 5.pdf")).toBeTruthy();
    expect(screen.getByText(/notes\.txt was not added: This kind of file cannot be imported\./u)).toBeTruthy();
    // A podcast is made from the lessons, so it goes with them.
    fireEvent.click(screen.getByRole("checkbox", { name: "Lessons" }));
    expect(screen.getByRole("checkbox", { name: "Podcast" }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(create());

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(physiology.id, null));
    expect(subjectsIpc.createSubject).not.toHaveBeenCalled();
    expect(sourceNotesIpc.writeSourceNotes).not.toHaveBeenCalled();
    expect(jobs.importStagedDocument).toHaveBeenCalledWith(expect.objectContaining({ id: "a".repeat(32) }), undefined, expect.any(Function));
    expect(jobs.importStagedDocument).toHaveBeenCalledWith(expect.objectContaining({ id: "b".repeat(32) }), undefined, expect.any(Function));
    expect(takeStudySet(physiology.id)).toEqual(expect.objectContaining({ lessons: false, podcast: false, lectureAudio: false }));
  });

  it("files a library document in the subject and makes its first cards", async () => {
    const unfiled = { id: "u".repeat(32), title: "Costanzo GI", sourceFormat: "pdf", generatedDeckId: null, lifecycleState: "ready" } as DocumentSummary;
    jobs.documents = [unfiled];
    const { onCreated, create } = renderSheet(physiology.id);
    fireEvent.click(screen.getByRole("button", { name: /From the library/u }));
    fireEvent.click(within(screen.getByRole("list", { name: "Documents in your library" })).getByRole("checkbox"));
    expect(await screen.findByText(/in your library/u)).toBeTruthy();
    fireEvent.click(await screen.findByRole("checkbox", { name: "Lecture audio" }));
    fireEvent.click(create());
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(physiology.id, null));
    expect(subjectsIpc.moveDocumentToSubject).toHaveBeenCalledWith(expect.objectContaining({ documentId: unfiled.id, subjectId: physiology.id }));
    expect(jobs.retry).toHaveBeenCalledWith(unfiled, undefined);
    expect(jobs.importStagedDocument).not.toHaveBeenCalled();
    expect(takeStudySet(physiology.id)).toEqual(expect.objectContaining({ documentIds: [unfiled.id], lectureAudio: false }));
  });

  it("keeps the sheet open with the reason when the notes cannot be written", async () => {
    vi.mocked(sourceNotesIpc.writeSourceNotes).mockRejectedValue(new Error("Choose an AI model first."));
    const { onCreated, create } = renderSheet(physiology.id);
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt" }), { target: { value: "Renal clearance" } });
    fireEvent.click(create());
    expect(await screen.findByText("Choose an AI model first.")).toBeTruthy();
    expect(onCreated).not.toHaveBeenCalled();
    expect(jobs.importStagedDocument).not.toHaveBeenCalled();
  });
});
