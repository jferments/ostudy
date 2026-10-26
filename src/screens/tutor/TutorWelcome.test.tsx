import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAiTaskDefaults, listAiProviderProfiles } from "../../ipc/aiProfiles";
import { getLearningTaskProgress, getSemanticPreparation, listSemanticScopeChoices } from "../../ipc/knowledge";
import {
  createTutorConversation,
  getTutorConversation,
  listTutorConversations,
  listTutorProjects,
  sendTutorTurn,
  type ConversationSummary,
  type TutorTurn,
} from "../../ipc/tutor";
import { listSubjectLessons } from "../../ipc/lessons";
import { getSettings, type AppSettings } from "../../ipc/settings";
import { getSpeechInputStatus, installSpeechInputModel, transcribeSpeech, type SpeechInputStatus } from "../../ipc/speechInput";
import { microphoneAvailable, startRecording } from "../../platform/microphone";
import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { TutorContext } from "./TutorContext";
import { TutorWorkspace } from "./TutorWorkspace";
import { useTutorController, type TutorController } from "./useTutorController";

vi.mock("../../ipc/aiProfiles", () => ({
  getAiTaskDefaults: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("../../ipc/knowledge", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/knowledge")>();
  return {
    ...actual,
    cancelLearningTask: vi.fn(),
    listSemanticScopeChoices: vi.fn(),
    getSemanticPreparation: vi.fn(),
    getSemanticSourceCitation: vi.fn(),
    prepareSemanticMaterial: vi.fn(),
    previewSemanticPreparation: vi.fn(),
    getLearningTaskProgress: vi.fn(),
  };
});
vi.mock("../../ipc/tutor", () => ({
  createTutorConversation: vi.fn(),
  createTutorProject: vi.fn(),
  createTutorProjectConversation: vi.fn(),
  deleteTutorConversation: vi.fn(),
  deleteTutorProject: vi.fn(),
  getTutorConversation: vi.fn(),
  listTutorConversations: vi.fn(),
  listTutorProjects: vi.fn(),
  moveTutorConversation: vi.fn(),
  renameTutorConversation: vi.fn(),
  sendTutorTurn: vi.fn(),
  setTutorConversationArchived: vi.fn(),
  setTutorProjectArchived: vi.fn(),
  updateTutorProject: vi.fn(),
}));
vi.mock("../../ui/overlays/overlayContext", () => {
  const actions = {
    register: () => () => undefined,
    isTop: () => true,
    confirm: async () => true,
  };
  return {
    useConfirm: () => actions.confirm,
    useOverlayParentId: () => null,
    useOverlayActions: () => actions,
    useOverlayContext: () => ({ topId: null, ...actions }),
  };
});
vi.mock("../../ipc/lessons", () => ({ listSubjectLessons: vi.fn() }));
vi.mock("../../ipc/settings", () => ({ getSettings: vi.fn() }));
vi.mock("../../ipc/speechInput", () => ({
  getSpeechInputStatus: vi.fn(),
  installSpeechInputModel: vi.fn(),
  transcribeSpeech: vi.fn(),
}));
vi.mock("../../platform/microphone", () => ({ microphoneAvailable: vi.fn(), startRecording: vi.fn() }));
vi.mock("../../controllers/HomeSummaryProvider", () => ({ useOptionalHomeSummary: vi.fn() }));
vi.mock("./TutorMemoryControls", () => ({ TutorMemoryControls: () => null }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Dialog: ({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) => (
    <section role="dialog" aria-label={title}>{children}<div>{actions}</div></section>
  ),
  Sheet: ({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) => (
    <section role="dialog" aria-label={title}>{children}<div>{actions}</div></section>
  ),
}));

const renal = { kind: "collection" as const, id: "r".repeat(32) };
const other = { kind: "collection" as const, id: "s".repeat(32) };

function summaryRow(subjectId: string, dueReviews: number, guideCount: number) {
  return {
    subjectId, dueReviews, dueNew: 0, scheduledReviews: dueReviews, scheduledNew: 0, cardCount: 40, deckCount: 1,
    sourceCount: 1, hasOverviewGuide: false, guideCount, lessonCount: 0, podcastCount: 0, cover: null, dueDeckIds: [],
  };
}

function conversation(id: string, title: string, scope: ConversationSummary["scope"]): ConversationSummary {
  return { id, title, scope, projectId: null, revision: 1, organizationRevision: 1, archivedAt: null, createdAt: 1, updatedAt: 1 };
}

const quizTurn: TutorTurn = {
  id: "q".repeat(32), operationId: "o".repeat(32), sequence: 1, mode: "quiz", userText: "",
  assistantText: "Where is renin made?", state: "complete", citations: [], evidenceCurrent: true,
  pendingQuestion: true, feedback: null, provider: "fixture", model: "fixture", errorCode: "", errorMessage: "",
  totalTokens: null, createdAt: 1, replacedByTurnId: null,
};

const speechStatus: SpeechInputStatus = {
  supported: true, modelLabel: "Moonshine v2 (English)", downloadBytes: 141_300_566, installed: true,
  installingBytes: null, installError: null,
};
const recordedAudio = { samplesBase64: "AAAA", sampleRate: 16_000, seconds: 3 };

let controller: TutorController;
function Harness() {
  controller = useTutorController();
  return (
    <TutorContext.Provider value={controller}>
      <TutorWorkspace showHistory />
    </TutorContext.Provider>
  );
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null, tutorProfileId: null, embeddingsProfileId: null });
  vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(listSemanticScopeChoices).mockResolvedValue([{ scope: renal, title: "Renal" }, { scope: other, title: "Cardiac" }]);
  vi.mocked(listTutorConversations).mockResolvedValue([]);
  vi.mocked(listTutorProjects).mockResolvedValue([]);
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(getSpeechInputStatus).mockResolvedValue({ ...speechStatus, supported: false });
  vi.mocked(microphoneAvailable).mockReturnValue(true);
  vi.mocked(getSettings).mockResolvedValue({ speechAutoSendSeconds: 0 } as AppSettings);
  vi.mocked(listSubjectLessons).mockImplementation(async (subjectId: string) => ({ subjectId, lessons: [] }));
  vi.mocked(useOptionalHomeSummary).mockReturnValue({
    summary: {
      studyDate: "2026-10-25",
      today: {} as never,
      subjects: [summaryRow(other.id, 0, 2), summaryRow(renal.id, 12, 0)],
      noSubject: summaryRow("", 0, 0) as never,
    },
    library: {
      libraryRevision: 1, documentFilingRevision: 1, decks: [], documents: [], studyRanges: [], leftOutDocumentIds: [], checkTheseCount: 0,
      subjects: [
        { id: renal.id, name: "Renal", sortOrder: 0, createdAt: 1, updatedAt: 1, revision: 1 },
        { id: other.id, name: "Cardiac", sortOrder: 1, createdAt: 1, updatedAt: 1, revision: 1 },
      ],
    },
    summaryError: null, libraryError: null, isLoading: false, hasArchivedDecks: false,
    refresh: () => undefined, acceptLibrary: () => undefined,
  });
});

describe("Tutor new chat", () => {
  it("welcomes a new chat with the raccoon, one question and starters from real study state", async () => {
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    expect(screen.getByRole("heading", { name: "What are we studying today?" })).toBeTruthy();
    expect(document.querySelector('img[data-artwork-slot="tutor.welcome"]')).not.toBeNull();
    const starters = await screen.findByRole("list", { name: "Start from your study" });
    const names = [...starters.querySelectorAll("strong")].map((node) => node.textContent);
    expect(names).toEqual(["Quiz me on Renal", "Sum up Cardiac"]);
    expect(starters.textContent).toContain("12 cards due");
    expect(starters.textContent).toContain("2 study guides");
  });

  it("starts a quiz chat on the starter's subject", async () => {
    vi.mocked(createTutorConversation).mockResolvedValue(conversation("c".repeat(32), "Quiz: Renal", renal));
    vi.mocked(sendTutorTurn).mockResolvedValue(quizTurn);
    vi.mocked(getTutorConversation).mockResolvedValue({
      conversation: conversation("c".repeat(32), "Quiz: Renal", renal), turns: [quizTurn], hasEarlierTurns: false,
    });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    fireEvent.click(await screen.findByRole("button", { name: /Quiz me on Renal/ }));
    await waitFor(() => expect(sendTutorTurn).toHaveBeenCalled());
    expect(createTutorConversation).toHaveBeenCalledWith(expect.objectContaining({ scope: renal, title: "Quiz: Renal" }));
    expect(vi.mocked(sendTutorTurn).mock.calls[0]?.[0]).toMatchObject({ mode: "quiz", focus: null, includeConversations: false });
    expect(await screen.findByText("Where is renin made?")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "What are we studying today?" })).toBeNull();
  });

  it("sends on Enter and keeps Shift+Enter for a new line", async () => {
    vi.mocked(createTutorConversation).mockResolvedValue(conversation("c".repeat(32), "Why", { kind: "general", id: null }));
    vi.mocked(sendTutorTurn).mockResolvedValue({ ...quizTurn, mode: "explain", pendingQuestion: false, userText: "Why?" });
    vi.mocked(getTutorConversation).mockResolvedValue({
      conversation: conversation("c".repeat(32), "Why", { kind: "general", id: null }), turns: [], hasEarlierTurns: false,
    });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    const box = screen.getByRole("textbox", { name: "Message" });
    fireEvent.change(box, { target: { value: "Why?" } });
    fireEvent.keyDown(box, { key: "Enter", shiftKey: true });
    expect(sendTutorTurn).not.toHaveBeenCalled();
    await act(async () => { fireEvent.keyDown(box, { key: "Enter" }); });
    await waitFor(() => expect(sendTutorTurn).toHaveBeenCalledWith(expect.objectContaining({ mode: "explain", text: "Why?" })));
  });
});

describe("Tutor live answers", () => {
  it("shows the explanation while it is written, then the checked answer", async () => {
    const id = "c".repeat(32);
    vi.mocked(createTutorConversation).mockResolvedValue(conversation(id, "Osmosis", { kind: "general", id: null }));
    let finish: (turn: TutorTurn) => void = () => undefined;
    vi.mocked(sendTutorTurn).mockImplementation(() => new Promise<TutorTurn>((resolve) => { finish = resolve; }));
    vi.mocked(getLearningTaskProgress).mockResolvedValue({
      operationId: "x", phase: "writing response", completed: 2, total: 4, canceled: false, draft: "Osmosis is the movement",
    });
    const done = { ...quizTurn, mode: "explain" as const, pendingQuestion: false, userText: "What is osmosis?",
      assistantText: "Osmosis is the movement of water across a membrane." };
    vi.mocked(getTutorConversation).mockResolvedValue({
      conversation: conversation(id, "Osmosis", { kind: "general", id: null }), turns: [done], hasEarlierTurns: false,
    });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    const box = screen.getByRole("textbox", { name: "Message" });
    fireEvent.change(box, { target: { value: "What is osmosis?" } });
    await act(async () => { fireEvent.keyDown(box, { key: "Enter" }); });
    const live = await screen.findByTestId("tutor-live-answer");
    await waitFor(() => expect(live.textContent).toContain("Osmosis is the movement"));
    expect(within(live).getByRole("status").textContent).toBe("Writing…");
    const operationId = vi.mocked(sendTutorTurn).mock.calls[0]?.[0].operationId;
    expect(getLearningTaskProgress).toHaveBeenCalledWith(operationId);
    await act(async () => { finish(done); });
    expect(await screen.findByText("Osmosis is the movement of water across a membrane.")).toBeTruthy();
    expect(screen.queryByTestId("tutor-live-answer")).toBeNull();
  });
});

describe("Tutor chat list", () => {
  it("groups chats by subject and filters them by search", async () => {
    vi.mocked(listTutorConversations).mockResolvedValue([
      conversation("1".repeat(32), "Renin and GFR", renal),
      conversation("2".repeat(32), "Heart sounds", other),
      conversation("3".repeat(32), "Spaced repetition", { kind: "general", id: null }),
    ]);
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    expect(await screen.findByRole("region", { name: "Renal chats" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Cardiac chats" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Other chats" }).textContent).toContain("Spaced repetition");
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "renin" } });
    expect(screen.getByText("Renin and GFR")).toBeTruthy();
    expect(screen.queryByText("Heart sounds")).toBeNull();
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "zzz" } });
    expect(screen.getByText("No chats match.")).toBeTruthy();
  });
});

describe("Tutor speech input", () => {
  it("asks before downloading the speech model, then puts what was said in the message box", async () => {
    vi.mocked(getSpeechInputStatus).mockResolvedValue({ ...speechStatus, installed: false });
    vi.mocked(installSpeechInputModel).mockResolvedValue(speechStatus);
    const stop = vi.fn().mockResolvedValue(recordedAudio);
    vi.mocked(startRecording).mockResolvedValue({ stop, cancel: vi.fn() });
    vi.mocked(transcribeSpeech).mockResolvedValue({ text: "why do cells need ATP?", durationMs: 3000, elapsedMs: 400 });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    const box = screen.getByRole("textbox", { name: "Message" });
    fireEvent.change(box, { target: { value: "Quick one:" } });

    await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "Speak" })); });
    expect(installSpeechInputModel).toHaveBeenCalledTimes(1);
    expect(startRecording).not.toHaveBeenCalled();

    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Speak" })); });
    const recording = await screen.findByRole("button", { name: "Stop and use recording" });
    expect(recording.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Send" }).hasAttribute("disabled")).toBe(true);

    await act(async () => { fireEvent.click(recording); });
    await waitFor(() => expect((box as HTMLTextAreaElement).value).toBe("Quick one: why do cells need ATP?"));
    expect(transcribeSpeech).toHaveBeenCalledWith("AAAA", 16_000);
    expect(installSpeechInputModel).toHaveBeenCalledTimes(1);
    expect(sendTutorTurn).not.toHaveBeenCalled();
  });

  it("drops a recording on Escape without turning it into text", async () => {
    vi.mocked(getSpeechInputStatus).mockResolvedValue(speechStatus);
    const cancel = vi.fn();
    vi.mocked(startRecording).mockResolvedValue({ stop: vi.fn(), cancel });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "Speak" })); });
    const recording = await screen.findByRole("button", { name: "Stop and use recording" });
    fireEvent.keyDown(recording, { key: "Escape" });
    expect(cancel).toHaveBeenCalled();
    expect(await screen.findByRole("button", { name: "Speak" })).toBeTruthy();
    expect(transcribeSpeech).not.toHaveBeenCalled();
  });

  it("says so when nothing was heard and leaves the message box as it was", async () => {
    vi.mocked(getSpeechInputStatus).mockResolvedValue(speechStatus);
    vi.mocked(startRecording).mockResolvedValue({ stop: vi.fn().mockResolvedValue(recordedAudio), cancel: vi.fn() });
    vi.mocked(transcribeSpeech).mockResolvedValue({ text: "", durationMs: 3000, elapsedMs: 200 });
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "Speak" })); });
    await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "Stop and use recording" })); });
    expect(await screen.findByText(/Nothing was heard/)).toBeTruthy();
    expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toBe("");
  });

  it("sends a spoken question once the learner has been quiet for the chosen pause", async () => {
    vi.mocked(getSettings).mockResolvedValue({ speechAutoSendSeconds: 2 } as AppSettings);
    vi.mocked(getSpeechInputStatus).mockResolvedValue(speechStatus);
    let hear: (level: number) => void = () => undefined;
    vi.mocked(startRecording).mockImplementation(async (onLevel) => {
      hear = onLevel;
      return { stop: vi.fn().mockResolvedValue(recordedAudio), cancel: vi.fn() };
    });
    vi.mocked(transcribeSpeech).mockResolvedValue({ text: "why do cells need ATP?", durationMs: 3000, elapsedMs: 400 });
    vi.mocked(createTutorConversation).mockResolvedValue(conversation("c".repeat(32), "Why", { kind: "general", id: null }));
    vi.mocked(sendTutorTurn).mockResolvedValue({ ...quizTurn, mode: "explain", pendingQuestion: false, userText: "why do cells need ATP?" });
    vi.mocked(getTutorConversation).mockResolvedValue({
      conversation: conversation("c".repeat(32), "Why", { kind: "general", id: null }),
      turns: [{ ...quizTurn, mode: "explain", pendingQuestion: false, userText: "why do cells need ATP?" }],
      hasEarlierTurns: false,
    });
    let now = 0;
    const clock = vi.spyOn(performance, "now").mockImplementation(() => now);
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "Speak" })); });
    await screen.findByRole("button", { name: "Stop and use recording" });

    const listen = (level: number, ms: number) => {
      for (const end = now + ms; now < end; now += 100) hear(level);
    };
    await act(async () => { listen(0.01, 500); listen(0.5, 1000); listen(0.01, 1500); });
    expect(transcribeSpeech).not.toHaveBeenCalled();

    await act(async () => { listen(0.01, 600); });
    await waitFor(() => expect(sendTutorTurn).toHaveBeenCalledTimes(1));
    expect(vi.mocked(sendTutorTurn).mock.calls[0]?.[0]).toMatchObject({ mode: "explain", text: "why do cells need ATP?" });
    clock.mockRestore();
  });

  it("offers no microphone where speech input is not supported", async () => {
    render(<Harness />);
    await act(async () => { await controller.initialize(); });
    expect(screen.queryByRole("button", { name: "Speak" })).toBeNull();
  });
});
