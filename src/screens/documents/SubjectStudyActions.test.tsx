import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { listDecks } from "../../ipc/decks";
import type { DocumentSummary } from "../../ipc/documents";
import * as lessons from "../../ipc/lessons";
import * as studyMaterials from "../../ipc/studyMaterials";
import { resetCourseRunsForTests } from "../lessons/useSubjectCourse";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { makeDeck, makeSubjectLibrary } from "../../test/factories";
import { queueStudySet, resetStudySetsForTests, takeStudySet } from "./pendingStudySets";
import { StudySetMaterialsStarter } from "./StudySetMaterialsStarter";
import { SubjectStudyActions } from "./SubjectStudyActions";
import { resetStudyTargetBundleRuns } from "./useStudyTargetBundleController";

vi.mock("../../ipc/studyMaterials", () => ({
  beginStudyTargetBundle: vi.fn(),
  composeStudyTargetBundle: vi.fn(),
  cancelStudyTargetBundle: vi.fn(),
  getStudyTargetBundle: vi.fn(),
}));

vi.mock("../../ipc/decks", () => ({ listDecks: vi.fn() }));

vi.mock("../../ipc/lessons", () => ({
  listSubjectLessons: vi.fn(),
  writeSubjectCourse: vi.fn(),
  getSubjectCourseWritingProgress: vi.fn(),
  cancelSubjectCourseWriting: vi.fn(),
}));

const subjectId = "c".repeat(32);

const withCards = "a".repeat(32);
const withoutCards = "b".repeat(32);
const deckId = "d".repeat(32);

function document(id: string, title: string, generatedDeckId: string | null): DocumentSummary {
  return {
    id, title, generatedDeckId, lifecycleState: "ready", sourceFormat: "pdf", sourceUnitKind: "page", sourceUnitCount: 4,
  } as DocumentSummary;
}

function renderActions(props: Partial<ComponentProps<typeof SubjectStudyActions>> = {}) {
  const view = (next: Partial<ComponentProps<typeof SubjectStudyActions>>) => (
    <OverlayProvider>
      <SubjectStudyActions subjectId={subjectId} refreshKey={1} hasDocuments {...props} {...next} />
    </OverlayProvider>
  );
  const rendered = render(view({}));
  return { ...rendered, update: (next: Partial<ComponentProps<typeof SubjectStudyActions>>) => rendered.rerender(view(next)) };
}

function chip(sheet: HTMLElement, name: string | RegExp): HTMLElement {
  return within(sheet).getByRole("button", { name });
}

describe("SubjectStudyActions", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValue(null);
    vi.mocked(studyMaterials.beginStudyTargetBundle).mockRejectedValue(new Error("stop here"));
    vi.mocked(lessons.listSubjectLessons).mockResolvedValue({ subjectId, lessons: [] });
    vi.mocked(lessons.writeSubjectCourse).mockReturnValue(new Promise(() => undefined));
    vi.mocked(lessons.getSubjectCourseWritingProgress).mockResolvedValue({ planned: null, written: 0 });
    vi.mocked(listDecks).mockResolvedValue([makeDeck({ id: deckId, name: "Renal physiology" })]);
    resetCourseRunsForTests();
    resetStudySetsForTests();
    resetStudyTargetBundleRuns();
  });

  it("asks what to make, then makes flashcards, a study guide and lessons together", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));

    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    for (const name of ["Flashcards", "Study guide", "Lessons"]) {
      expect(chip(sheet, name).getAttribute("aria-pressed")).toBe("true");
    }

    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith(expect.objectContaining({
      target: { kind: "collection", id: subjectId },
      outputPolicy: { flashcards: true, studyGuide: true },
    })));
    expect(lessons.writeSubjectCourse).toHaveBeenCalledWith(subjectId, expect.any(String), false);
    expect(await screen.findByText("Writing lessons")).toBeTruthy();
  });

  it("shows a New study set waiting for its documents", async () => {
    const added = "e".repeat(32);
    queueStudySet(subjectId, { flashcards: true, studyGuide: true, lessons: false, documentIds: [added] });
    renderActions({ documents: [] });
    expect(await screen.findByText("Making flashcards")).toBeTruthy();
  });

  it("starts a New study set's materials once its documents are read, with no subject page open", async () => {
    const added = "e".repeat(32);
    queueStudySet(subjectId, { flashcards: true, studyGuide: false, lessons: true, podcast: true, documentIds: [added] });
    const working = (documents: DocumentSummary[]) => (documentId: string) =>
      documents.some((item) => item.id === documentId && item.lifecycleState === "processing");
    // The new document is not listed yet, then it is being read, then it is read.
    const { rerender } = render(<StudySetMaterialsStarter documents={[]} isDocumentWorking={working([])} />);
    const processing = [{ ...document(added, "Notes", deckId), lifecycleState: "processing" } as DocumentSummary];
    rerender(<StudySetMaterialsStarter documents={processing} isDocumentWorking={working(processing)} />);
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    const read = [document(added, "Notes", deckId)];
    rerender(<StudySetMaterialsStarter documents={read} isDocumentWorking={working(read)} />);
    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith(expect.objectContaining({
      outputPolicy: { flashcards: true, studyGuide: false },
    })));
    expect(lessons.writeSubjectCourse).toHaveBeenCalledWith(subjectId, expect.any(String), false);
    expect(takeStudySet(subjectId)).toBeNull();
  });

  it("makes only what is ticked", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    fireEvent.click(chip(sheet, "Flashcards"));
    fireEvent.click(chip(sheet, "Study guide"));
    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(lessons.writeSubjectCourse).toHaveBeenCalled());
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
  });

  it("offers only what is not made yet", async () => {
    vi.mocked(lessons.listSubjectLessons).mockResolvedValue({
      subjectId,
      lessons: [{ id: "l".repeat(32) } as lessons.SubjectLessons["lessons"][number]],
    });
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    const made = chip(sheet, "Lessons, made");
    expect(made.getAttribute("aria-pressed")).toBe("false");
    expect((made as HTMLButtonElement).disabled).toBe(true);
  });

  it("will not make nothing", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    for (const name of ["Flashcards", "Study guide", "Lessons"]) {
      fireEvent.click(chip(sheet, name));
    }

    expect((within(sheet).getByRole("button", { name: "Make" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("picks documents, then makes first cards into a chosen deck before the guide", async () => {
    const onSetLeftOut = vi.fn().mockResolvedValue(true);
    const working = new Set<string>();
    // Starting a document's cards marks it working until they are saved.
    const onMakeFirstCards = vi.fn(async (started: DocumentSummary) => { working.add(started.id); });
    const props = {
      documents: [document(withCards, "Nephron notes", "e".repeat(32)), document(withoutCards, "Renal lecture 4", null)],
      library: makeSubjectLibrary({
        decks: [{ deckId, subjectId, sortOrder: 0 }],
        leftOutDocumentIds: [withoutCards],
      }),
      onSetLeftOut,
      onMakeFirstCards,
      isDocumentWorking: (id: string) => working.has(id),
    };
    const view = renderActions(props);
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    const from = within(sheet).getByRole("group", { name: "From" });
    expect(within(from).getByRole("checkbox", { name: /Nephron notes/u }).getAttribute("aria-checked")).toBe("true");
    const leftOut = within(from).getByRole("checkbox", { name: /Renal lecture 4/u });
    expect(leftOut.getAttribute("aria-checked")).toBe("false");
    // Every chosen document has cards, so there is nothing to write them with.
    expect(within(sheet).queryByRole("combobox", { name: "Cards go into" })).toBeNull();

    fireEvent.click(leftOut);
    const deck = await within(sheet).findByRole("combobox", { name: "Cards go into" });
    await within(deck).findByRole("option", { name: "Renal physiology" });
    fireEvent.change(deck, { target: { value: deckId } });
    fireEvent.change(within(sheet).getByRole("textbox", { name: /Instructions/u }), { target: { value: "Focus on the countercurrent mechanism" } });
    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(onMakeFirstCards).toHaveBeenCalledTimes(1));
    expect(onSetLeftOut).toHaveBeenCalledWith([]);
    expect(onMakeFirstCards).toHaveBeenCalledWith(
      expect.objectContaining({ id: withoutCards }), "Focus on the countercurrent mechanism", deckId,
    );
    expect(await screen.findByText("Making flashcards")).toBeTruthy();
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    expect(lessons.writeSubjectCourse).not.toHaveBeenCalled();

    working.delete(withoutCards);
    view.update({});
    // The app's starter begins the rest once the cards are saved.
    render(<StudySetMaterialsStarter documents={props.documents} isDocumentWorking={(id) => working.has(id)} />);
    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith(expect.objectContaining({
      outputPolicy: { flashcards: true, studyGuide: true },
    })));
    expect(lessons.writeSubjectCourse).toHaveBeenCalled();
  });

  it("leaves unticked documents out and makes from the rest at once", async () => {
    const onSetLeftOut = vi.fn().mockResolvedValue(true);
    const onMakeFirstCards = vi.fn();
    renderActions({
      documents: [document(withCards, "Nephron notes", "e".repeat(32)), document("c".repeat(32), "Cardiac review", "f".repeat(32))],
      library: makeSubjectLibrary(),
      onSetLeftOut,
      onMakeFirstCards,
    });
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Cardiac review/u }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(onSetLeftOut).toHaveBeenCalledWith(["c".repeat(32)]));
    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalled());
    expect(onMakeFirstCards).not.toHaveBeenCalled();
  });

  it("needs at least one document", async () => {
    renderActions({ documents: [document(withCards, "Nephron notes", "e".repeat(32))], library: makeSubjectLibrary() });
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Nephron notes/u }));

    expect((within(sheet).getByRole("button", { name: "Make" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
