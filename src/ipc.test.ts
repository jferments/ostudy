import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  analyzeBulkCards,
  archiveDeck,
  applyScheduleAction,
  beginAssetDraft,
  bulkCreateCards,
  bulkTagCards,
  cancelAssetDraft,
  completePracticeSession,
  createCardTemplate,
  createDeck,
  createGeneratedCard,
  createMutationId,
  createOperationId,
  createSubmissionId,
  deleteCards,
  exportDeck,
  exportLibraryBackup,
  findDuplicateCards,
  generateCardsFromTemplate,
  getAppInfo,
  getCardTemplate,
  getCardTypeInfo,
  getDeck,
  getErrorCode,
  getErrorMessage,
  getOrganizedCard,
  getPracticeCardWindow,
  getPracticeWorkload,
  getSettings,
  importDeck,
  importImage,
  listArchivedDecks,
  listCardTemplateSummaries,
  listDeckCards,
  listDecks,
  listMatchingCards,
  listTags,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  prepareLibraryRestore,
  previewTemplateCards,
  saveSettings,
  searchCards,
  startCardPractice,
  startSelectedPractice,
  submitPracticeReview,
  syncAssetDraft,
  transferCards,
  undoDeletion,
  updateCardTemplate,
  updateDeck,
  updateGeneratedCard,
  unarchiveDeck,
  type AppSettings,
  type CardSearchInput,
  type CreateCardTemplateInput,
  type CreateGeneratedCardInput,
} from "./ipc";
import { makeSettings, paragraphDocument } from "./test/factories";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const deckId = "a".repeat(32);
const noteId = "b".repeat(32);
const cardId = "c".repeat(32);
const templateId = "d".repeat(32);
const draftId = "7".repeat(32);
const importOperationId = "9".repeat(32);

const searchInput: CardSearchInput = {
  deckIds: [deckId],
  query: "warfarin",
  tagIds: [],
  dueFilter: "due",
  statusFilter: "active",
  reviewedFilter: "all",
  sort: "relevance",
  limit: 100,
  offset: 0,
};

const generatedInput: CreateGeneratedCardInput = {
  deckId,
  cardType: "basic",
  reverseEnabled: true,
  clozeSource: "",
  clozeMode: "one_at_a_time",
  frontDoc: paragraphDocument("Question"),
  backDoc: paragraphDocument("Answer"),
  frontText: "Question",
  backText: "Answer",
  tags: ["Medication"],
  allowDuplicate: false,
};

const templateInput: CreateCardTemplateInput = {
  name: "Medication",
  description: "",
  fields: [{ id: "e".repeat(32), name: "Name", description: "" }],
  rules: [
    {
      id: "f".repeat(32),
      name: "Name card",
      frontDoc: paragraphDocument("{{Name}}"),
      backDoc: paragraphDocument("{{Name}}"),
    },
  ],
};

async function expectInvocation(
  operation: () => Promise<unknown>,
  command: string,
  payload?: Record<string, unknown>,
): Promise<void> {
  vi.mocked(invoke).mockResolvedValueOnce(undefined);
  await operation();
  if (payload === undefined) {
    expect(invoke).toHaveBeenLastCalledWith(command);
  } else {
    expect(invoke).toHaveBeenLastCalledWith(command, payload);
  }
}

describe("typed Tauri IPC wrappers", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("wires deck and card-library commands with their exact payload shapes", async () => {
    await expectInvocation(listDecks, "list_decks");
    await expectInvocation(listArchivedDecks, "list_archived_decks");
    await expectInvocation(() => archiveDeck(deckId), "archive_deck", { deckId });
    await expectInvocation(() => unarchiveDeck(deckId), "unarchive_deck", { deckId });
    await expectInvocation(() => getDeck(deckId), "get_deck", { deckId });
    await expectInvocation(
      () => createDeck({ name: "Pharmacology", description: "NCLEX" }),
      "create_deck",
      { input: { name: "Pharmacology", description: "NCLEX" } },
    );
    await expectInvocation(
      () =>
        updateDeck({
          deckId,
          name: "Advanced pharmacology",
          description: "",
          revision: 2,
        }),
      "update_deck",
      {
        input: {
          deckId,
          name: "Advanced pharmacology",
          description: "",
          revision: 2,
        },
      },
    );
    await expectInvocation(() => listDeckCards(deckId), "list_deck_cards", { input: { deckId, query: "", limit: 50, offset: 0 } });
    await expectInvocation(() => listTags(), "list_tags", {
      input: { deckIds: [] },
    });
    await expectInvocation(() => searchCards(searchInput), "search_cards", {
      input: searchInput,
    });
    await expectInvocation(
      () => listMatchingCards(searchInput),
      "list_matching_cards",
      { input: searchInput },
    );
    await expectInvocation(
      () => getOrganizedCard(noteId),
      "get_organized_card",
      { noteId },
    );
  });

  it("wires generated-card, template, duplicate, and bulk commands", async () => {
    await expectInvocation(
      () => createGeneratedCard(generatedInput),
      "create_generated_card",
      { input: generatedInput },
    );
    await expectInvocation(
      () =>
        updateGeneratedCard({
          ...generatedInput,
          noteId,
          revision: 3,
        }),
      "update_generated_card",
      { input: { ...generatedInput, noteId, revision: 3 } },
    );
    await expectInvocation(() => getCardTypeInfo(noteId), "get_card_type_info", {
      noteId,
    });
    await expectInvocation(listCardTemplateSummaries, "list_card_template_summaries");
    await expectInvocation(
      () => getCardTemplate(templateId),
      "get_card_template",
      { templateId },
    );
    await expectInvocation(
      () => createCardTemplate(templateInput),
      "create_card_template",
      { input: templateInput },
    );
    await expectInvocation(
      () =>
        updateCardTemplate({
          ...templateInput,
          templateId,
          revision: 4,
        }),
      "update_card_template",
      { input: { ...templateInput, templateId, revision: 4 } },
    );
    const values = [
      {
        fieldId: templateInput.fields[0]?.id ?? "",
        doc: paragraphDocument("Lisinopril"),
        text: "Lisinopril",
      },
    ];
    await expectInvocation(
      () =>
        previewTemplateCards({
          templateId,
          templateRevision: 4,
          values,
        }),
      "preview_template_cards",
      { input: { templateId, templateRevision: 4, values } },
    );
    await expectInvocation(
      () =>
        generateCardsFromTemplate({
          deckId,
          templateId,
          templateRevision: 4,
          values,
          tags: ["ACE inhibitor"],
        }),
      "generate_cards_from_template",
      {
        input: {
          deckId,
          templateId,
          templateRevision: 4,
          values,
          tags: ["ACE inhibitor"],
        },
      },
    );
    await expectInvocation(
      () =>
        findDuplicateCards({
          deckId,
          frontText: "Question",
          backText: "Answer",
          excludeNoteId: noteId,
        }),
      "find_duplicate_cards",
      {
        input: {
          deckId,
          frontText: "Question",
          backText: "Answer",
          excludeNoteId: noteId,
        },
      },
    );
    const rows = [{ question: "Q", answer: "A", tags: [] }];
    await expectInvocation(
      () => analyzeBulkCards({ deckId, rows }),
      "analyze_bulk_cards",
      { input: { deckId, rows } },
    );
    await expectInvocation(
      () => bulkCreateCards({ deckId, rows }),
      "bulk_create_cards",
      { input: { deckId, rows } },
    );
    await expectInvocation(
      () =>
        transferCards({
          noteIds: [noteId],
          targetDeckId: "1".repeat(32),
          mode: "copy",
          skipDuplicates: true,
        }),
      "transfer_cards",
      {
        input: {
          noteIds: [noteId],
          targetDeckId: "1".repeat(32),
          mode: "copy",
          skipDuplicates: true,
        },
      },
    );
    await expectInvocation(
      () => bulkTagCards({ noteIds: [noteId], tags: ["Cardiology"], mode: "add" }),
      "bulk_tag_cards",
      { input: { noteIds: [noteId], tags: ["Cardiology"], mode: "add" } },
    );
    await expectInvocation(() => deleteCards([noteId]), "delete_cards", {
      input: { noteIds: [noteId] },
    });
    await expectInvocation(() => undoDeletion("8".repeat(32)), "undo_deletion", {
      input: { undoToken: "8".repeat(32) },
    });
  });

  it("wires settings, practice, scheduling, asset drafts, transfer, and backup commands", async () => {
    const settings: AppSettings = makeSettings({ dailyNewLimit: 12 });
    await expectInvocation(getSettings, "get_settings");
    await expectInvocation(() => saveSettings(settings), "save_settings", {
      input: settings,
    });
    await expectInvocation(getAppInfo, "get_app_info");
    await expectInvocation(
      () => beginAssetDraft("card", noteId),
      "begin_asset_draft",
      { input: { purpose: "card", ownerId: noteId } },
    );
    await expectInvocation(() => syncAssetDraft(draftId, [cardId]), "sync_asset_draft", {
      input: { draftId, assetIds: [cardId] },
    });
    await expectInvocation(() => importImage("/tmp/image.png", draftId), "import_image", {
      input: { filePath: "/tmp/image.png", draftId },
    });
    await expectInvocation(() => cancelAssetDraft(draftId), "cancel_asset_draft", {
      draftId,
    });
    await expectInvocation(
      () => getPracticeWorkload([deckId]),
      "get_practice_workload",
      { input: { deckIds: [deckId], tagIds: [] } },
    );
    const selectedStart = {
      operationId: "5".repeat(32),
      deckIds: [deckId],
      tagIds: [],
      shuffle: true,
      mode: "scheduled" as const,
    };
    await expectInvocation(
      () => startSelectedPractice(selectedStart),
      "start_selected_practice",
      { input: selectedStart },
    );
    const cardStart = {
      operationId: "6".repeat(32),
      cardIds: [cardId],
      shuffle: false,
    };
    await expectInvocation(() => startCardPractice(cardStart), "start_card_practice", {
      input: cardStart,
    });
    await expectInvocation(() => getPracticeCardWindow("2".repeat(32), 50), "get_practice_card_window", { input: { sessionId: "2".repeat(32), startPosition: 50, limit: 50 } });
    await expectInvocation(
      () => markPracticeCardViewed("2".repeat(32), cardId),
      "mark_practice_card_viewed",
      { input: { sessionId: "2".repeat(32), cardId } },
    );
    await expectInvocation(
      () => markPracticeCardRevealed("2".repeat(32), cardId),
      "mark_practice_card_revealed",
      { input: { sessionId: "2".repeat(32), cardId } },
    );
    const review = {
      sessionId: "2".repeat(32),
      cardId,
      submissionId: "3".repeat(32),
      outcome: "somewhat_confident" as const,
      responseTimeMs: 2_500,
      scheduleRevision: 2,
    };
    await expectInvocation(() => submitPracticeReview(review), "submit_practice_review", {
      input: review,
    });
    await expectInvocation(
      () => completePracticeSession("2".repeat(32)),
      "complete_practice_session",
      { sessionId: "2".repeat(32) },
    );
    const mutation = {
      mutationId: "4".repeat(32),
      cardIds: [cardId],
      action: "bury" as const,
    };
    await expectInvocation(() => applyScheduleAction(mutation), "apply_schedule_action", {
      input: mutation,
    });
    await expectInvocation(
      () => exportDeck(deckId, "/tmp/deck.studydeck"),
      "export_deck",
      { input: { deckId, destinationPath: "/tmp/deck.studydeck" } },
    );
    await expectInvocation(
      () => importDeck("/tmp/deck.studydeck", importOperationId),
      "import_deck",
      { input: { sourcePath: "/tmp/deck.studydeck", operationId: importOperationId } },
    );
    await expectInvocation(
      () => exportLibraryBackup("/tmp/library.ostudybackup"),
      "export_library_backup",
      { input: { destinationPath: "/tmp/library.ostudybackup" } },
    );
    await expectInvocation(
      () => prepareLibraryRestore("/tmp/library.ostudybackup"),
      "prepare_library_restore",
      { input: { sourcePath: "/tmp/library.ostudybackup" } },
    );
  });

  it("creates transport identifiers without hyphens", () => {
    vi.spyOn(globalThis.crypto, "randomUUID")
      .mockReturnValueOnce("00000000-1111-2222-3333-444444444444")
      .mockReturnValueOnce("11111111-2222-3333-4444-555555555555")
      .mockReturnValueOnce("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");

    expect(createOperationId()).toBe("00000000111122223333444444444444");
    expect(createSubmissionId()).toBe("11111111222233334444555555555555");
    expect(createMutationId()).toBe("aaaaaaaabbbbccccddddeeeeeeeeeeee");
  });

  it("extracts stable command errors and preserves useful fallback messages", () => {
    const payload = { code: "deck_changed", message: "Reload the deck." };
    expect(getErrorCode(payload)).toBe("deck_changed");
    expect(getErrorMessage(payload)).toBe("Reload the deck.");
    expect(getErrorCode(new Error("failure"))).toBeNull();
    expect(getErrorMessage(new Error("failure"))).toBe("failure");
    expect(getErrorMessage("native failure")).toBe("native failure");
    expect(getErrorMessage({ code: "unknown" })).toBe(
      "An unexpected error occurred.",
    );
  });
});