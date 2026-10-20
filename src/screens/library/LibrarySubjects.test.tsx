import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import * as homeIpc from "../../ipc/home";
import * as subjectsIpc from "../../ipc/subjects";
import { makeHomeSubjectSummary, makeHomeSummary, makeSubject, makeSubjectLibrary } from "../../test/factories";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { LibrarySubjectHeader, useLibrarySubjects } from "./LibrarySubjects";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  renameSubject: vi.fn(),
  moveSubject: vi.fn(),
  deleteSubject: vi.fn(),
}));

const physiology = makeSubject({ id: "p".repeat(32), name: "Physiology", sortOrder: 0, revision: 3 });
const pharmacology = makeSubject({ id: "f".repeat(32), name: "Pharmacology", sortOrder: 1 });
const library = makeSubjectLibrary({ libraryRevision: 7, subjects: [physiology, pharmacology] });

function Header({ onChooseSubject }: { onChooseSubject: () => void }) {
  const scope = { kind: "subject", subjectId: physiology.id } as const;
  const subjects = useLibrarySubjects(scope);
  return (
    <LibrarySubjectHeader
      scope={scope}
      subjects={subjects}
      onChooseSubject={onChooseSubject}
      onStartReview={vi.fn()}
      onPracticeSubject={vi.fn()}
      showToast={vi.fn()}
    />
  );
}

function renderHeader() {
  const onChooseSubject = vi.fn();
  render(
    <OverlayProvider>
      <HomeSummaryProvider section="overview" activeWorkflow={null} decks={[]}>
        <Header onChooseSubject={onChooseSubject} />
      </HomeSummaryProvider>
    </OverlayProvider>,
  );
  return { onChooseSubject };
}

async function chooseAction(name: string): Promise<void> {
  const trigger = await screen.findByRole("button", { name: "Physiology actions" });
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => {
      fireEvent.click(trigger);
    });
  }
  fireEvent.click(await within(await screen.findByRole("menu")).findByRole("menuitem", { name }));
}

describe("LibrarySubjectHeader", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary({
      subjects: [makeHomeSubjectSummary({ subjectId: physiology.id }), makeHomeSubjectSummary({ subjectId: pharmacology.id })],
    }));
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(library);
  });

  it("renames the chosen subject from its menu against the current revisions", async () => {
    vi.mocked(subjectsIpc.renameSubject).mockResolvedValue({
      ...library,
      libraryRevision: 8,
      subjects: [{ ...physiology, name: "Human physiology", revision: 4 }, pharmacology],
    });
    renderHeader();

    await chooseAction("Rename…");
    const sheet = await screen.findByRole("dialog", { name: "Rename subject" });
    fireEvent.change(within(sheet).getByRole("textbox", { name: "Name" }), { target: { value: "Human physiology" } });
    fireEvent.click(within(sheet).getByRole("button", { name: "Rename" }));

    await waitFor(() => expect(subjectsIpc.renameSubject).toHaveBeenCalledWith({
      subjectId: physiology.id, name: "Human physiology", revision: 3, expectedLibraryRevision: 7,
    }));
    expect(await screen.findByRole("heading", { name: "Human physiology" })).toBeTruthy();
  });

  it("deletes the chosen subject after confirming and returns to Everything", async () => {
    vi.mocked(subjectsIpc.deleteSubject).mockResolvedValue({ ...library, libraryRevision: 8, subjects: [pharmacology] });
    const { onChooseSubject } = renderHeader();

    await chooseAction("Delete subject…");
    const confirm = await screen.findByRole("alertdialog", { name: "Delete “Physiology”?" });
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete subject" }));

    await waitFor(() => expect(subjectsIpc.deleteSubject).toHaveBeenCalledWith({
      subjectId: physiology.id, revision: 3, expectedLibraryRevision: 7,
    }));
    await waitFor(() => expect(onChooseSubject).toHaveBeenCalledWith({ kind: "all" }));
  });
});
