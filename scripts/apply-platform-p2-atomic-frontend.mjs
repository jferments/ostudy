#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) throw new Error(`${label}: expected source was not found`);
  return text.replace(oldValue, newValue);
}

// Shared draft commit reference and hook handoff.
let drafts = await read("src/ipc/drafts.ts");
drafts = replaceExact(
  drafts,
  `export type EditorDraftState = "active" | "committed" | "discarded" | "conflicted";\n`,
  `export type EditorDraftState = "active" | "committed" | "discarded" | "conflicted";\n\nexport interface EditorDraftCommitRef {\n  draftId: string;\n  revision: number;\n}\n`,
  "add frontend editor draft commit reference",
);
await write("src/ipc/drafts.ts", drafts);

let hook = await read("src/controllers/useDurableDraft.ts");
hook = replaceExact(
  hook,
  `  type EditorDraft,\n  type EditorDraftKind,\n`,
  `  type EditorDraft,\n  type EditorDraftCommitRef,\n  type EditorDraftKind,\n`,
  "import editor draft commit reference",
);
hook = replaceExact(
  hook,
  `  flush: () => Promise<void>;\n  discard: () => Promise<void>;\n  commit: (targetEntityId?: string) => Promise<void>;\n`,
  `  flush: () => Promise<void>;\n  discard: () => Promise<void>;\n  prepareCanonicalCommit: () => Promise<EditorDraftCommitRef>;\n  acceptCanonicalCommit: () => void;\n  commit: (targetEntityId?: string) => Promise<void>;\n`,
  "extend durable draft controller contract",
);
hook = replaceExact(
  hook,
  `  const commit = useCallback(\n    async (targetEntityId?: string): Promise<void> => {\n      await flush();\n      const current = draftRef.current;\n      if (!current) return;\n      setStatus("committing");\n      await commitEditorDraft(current.id, current.revision, targetEntityId);\n      draftRef.current = null;\n      setDraft(null);\n      pendingPayloadRef.current = null;\n      setStatus("clean");\n    },\n    [flush],\n  );\n`,
  `  const prepareCanonicalCommit = useCallback(async (): Promise<EditorDraftCommitRef> => {\n    await flush();\n    const current = draftRef.current;\n    if (!current || current.state !== "active") {\n      throw new Error("The durable editor draft is not available for its final save.");\n    }\n    return { draftId: current.id, revision: current.revision };\n  }, [flush]);\n\n  const acceptCanonicalCommit = useCallback((): void => {\n    draftRef.current = null;\n    setDraft(null);\n    pendingPayloadRef.current = null;\n    pendingAssetIdsRef.current = [];\n    savedPayloadKeyRef.current = "";\n    setRecovered(false);\n    setErrorMessage(null);\n    setStatus("clean");\n  }, []);\n\n  const commit = useCallback(\n    async (targetEntityId?: string): Promise<void> => {\n      const reference = await prepareCanonicalCommit();\n      setStatus("committing");\n      await commitEditorDraft(reference.draftId, reference.revision, targetEntityId);\n      acceptCanonicalCommit();\n    },\n    [acceptCanonicalCommit, prepareCanonicalCommit],\n  );\n`,
  "add canonical durable draft handoff",
);
hook = replaceExact(
  hook,
  `    flush,\n    discard,\n    commit,\n    restart,\n`,
  `    flush,\n    discard,\n    prepareCanonicalCommit,\n    acceptCanonicalCommit,\n    commit,\n    restart,\n`,
  "return canonical durable draft handoff",
);
await write("src/controllers/useDurableDraft.ts", hook);

// Typed IPC write contracts.
let cards = await read("src/ipc/cards.ts");
cards = replaceExact(
  cards,
  `import type { DeletionResult, RichDocument, UndoDeletionResult } from "./common";\n`,
  `import type { DeletionResult, RichDocument, UndoDeletionResult } from "./common";\nimport type { EditorDraftCommitRef } from "./drafts";\n`,
  "import card editor draft reference",
);
cards = replaceExact(
  cards,
  `  assetDraftId?: string;\n}\n`,
  `  assetDraftId?: string;\n  editorDraft?: EditorDraftCommitRef;\n}\n`,
  "add card editor draft reference",
);
cards = replaceExact(
  cards,
  `export interface BulkCreateCardsInput {\n  deckId: string;\n  rows: BulkCardRowInput[];\n}\n`,
  `export interface BulkCreateCardsInput {\n  deckId: string;\n  rows: BulkCardRowInput[];\n  editorDraft?: EditorDraftCommitRef;\n}\n`,
  "add bulk editor draft reference",
);
await write("src/ipc/cards.ts", cards);

let templates = await read("src/ipc/templates.ts");
templates = replaceExact(
  templates,
  `import type { RichDocument } from "./common";\n`,
  `import type { RichDocument } from "./common";\nimport type { EditorDraftCommitRef } from "./drafts";\n`,
  "import template editor draft reference",
);
templates = replaceExact(
  templates,
  `  assetDraftId?: string;\n}\n\nexport interface UpdateCardTemplateInput`,
  `  assetDraftId?: string;\n  editorDraft?: EditorDraftCommitRef;\n}\n\nexport interface UpdateCardTemplateInput`,
  "add template editor draft reference",
);
templates = replaceExact(
  templates,
  `  tags: string[];\n  assetDraftId?: string;\n}\n`,
  `  tags: string[];\n  assetDraftId?: string;\n  editorDraft?: EditorDraftCommitRef;\n}\n`,
  "add generated-card editor draft reference",
);
await write("src/ipc/templates.ts", templates);

let ai = await read("src/ipc/aiProfiles.ts");
ai = replaceExact(
  ai,
  `import { createOperationId } from "./common";\n`,
  `import { createOperationId } from "./common";\nimport type { EditorDraftCommitRef } from "./drafts";\n`,
  "import AI editor draft reference",
);
ai = replaceExact(
  ai,
  `export interface AiProfileDraft {\n  displayName: string;\n  providerKind: AiProviderKind;\n  baseUrl: string;\n  modelId: string;\n  authMode: AiAuthMode;\n  enabled: boolean;\n  allowInsecureHttp: boolean;\n  requestTimeoutMs: number;\n}\n`,
  `export interface AiProfileDraft {\n  displayName: string;\n  providerKind: AiProviderKind;\n  baseUrl: string;\n  modelId: string;\n  authMode: AiAuthMode;\n  enabled: boolean;\n  allowInsecureHttp: boolean;\n  requestTimeoutMs: number;\n}\n\nexport interface AiProfileWriteInput extends AiProfileDraft {\n  editorDraft?: EditorDraftCommitRef;\n}\n`,
  "add AI profile write contract",
);
ai = replaceExact(
  ai,
  `  input: AiProfileDraft,\n  operationId = createOperationId(),\n`,
  `  input: AiProfileWriteInput,\n  operationId = createOperationId(),\n`,
  "accept AI create draft reference",
);
ai = replaceExact(
  ai,
  `  input: AiProfileDraft,\n  operationId = createOperationId(),\n): Promise<AiProviderProfileDetail> {\n  return invoke<AiProviderProfileDetail>("update_ai_provider_profile", {`,
  `  input: AiProfileWriteInput,\n  operationId = createOperationId(),\n): Promise<AiProviderProfileDetail> {\n  return invoke<AiProviderProfileDetail>("update_ai_provider_profile", {`,
  "accept AI update draft reference",
);
ai = replaceExact(
  ai,
  `      requestTimeoutMs: input.requestTimeoutMs,\n    },\n`,
  `      requestTimeoutMs: input.requestTimeoutMs,\n      editorDraft: input.editorDraft,\n    },\n`,
  "send AI update draft reference",
);
await write("src/ipc/aiProfiles.ts", ai);

// Card editor canonical writes.
let cardScreen = await read("src/screens/CardEditorScreen.tsx");
cardScreen = replaceExact(
  cardScreen,
  `    try {\n      await durableDraft.flush();\n      if (selectedCustomTemplate) {`,
  `    try {\n      const editorDraft = await durableDraft.prepareCanonicalCommit();\n      if (selectedCustomTemplate) {`,
  "prepare card canonical draft reference",
);
cardScreen = replaceExact(
  cardScreen,
  `          assetDraftId: assetDraftId ?? undefined,\n        });\n        await durableDraft.commit();\n`,
  `          assetDraftId: assetDraftId ?? undefined,\n          editorDraft,\n        });\n        durableDraft.acceptCanonicalCommit();\n`,
  "atomically commit generated template cards",
);
const cardInputAnchor = `          assetDraftId: assetDraftId ?? undefined,\n        };`;
const cardInputReplacement = `          assetDraftId: assetDraftId ?? undefined,\n          editorDraft,\n        };`;
const cardInputMatches = cardScreen.split(cardInputAnchor).length - 1;
if (cardInputMatches !== 2) {
  throw new Error(`card input draft references: expected 2 matches, found ${cardInputMatches}`);
}
cardScreen = cardScreen.split(cardInputAnchor).join(cardInputReplacement);
cardScreen = replaceExact(
  cardScreen,
  `      await durableDraft.commit(saved.noteId);\n      markAssetDraftCommitted();\n`,
  `      durableDraft.acceptCanonicalCommit();\n      markAssetDraftCommitted();\n`,
  "atomically commit built-in card",
);
await write("src/screens/CardEditorScreen.tsx", cardScreen);

// Template editor canonical writes.
let templateScreen = await read("src/screens/TemplatesScreen.tsx");
templateScreen = replaceExact(
  templateScreen,
  `    try {\n      await durableDraft.flush();\n      const input = {`,
  `    try {\n      const editorDraft = await durableDraft.prepareCanonicalCommit();\n      const input = {`,
  "prepare template canonical draft reference",
);
templateScreen = replaceExact(
  templateScreen,
  `        assetDraftId,\n      };\n`,
  `        assetDraftId,\n        editorDraft,\n      };\n`,
  "send template canonical draft reference",
);
templateScreen = replaceExact(
  templateScreen,
  `      await durableDraft.commit(saved.id);\n      openRequestSequence.current += 1;\n`,
  `      durableDraft.acceptCanonicalCommit();\n      openRequestSequence.current += 1;\n`,
  "accept template canonical commit",
);
await write("src/screens/TemplatesScreen.tsx", templateScreen);

// Bulk-entry canonical write.
let bulk = await read("src/screens/cards/BulkEntryModal.tsx");
bulk = replaceExact(
  bulk,
  `      await durableDraft.flush();\n      const result = await bulkCreateCards({ deckId, rows: cardRows });\n      await durableDraft.commit();\n`,
  `      const editorDraft = await durableDraft.prepareCanonicalCommit();\n      const result = await bulkCreateCards({ deckId, rows: cardRows, editorDraft });\n      durableDraft.acceptCanonicalCommit();\n`,
  "atomically commit bulk cards",
);
await write("src/screens/cards/BulkEntryModal.tsx", bulk);

// AI profile metadata canonical write. Credentials remain outside the payload.
let aiController = await read("src/controllers/aiProfilesController.ts");
aiController = replaceExact(
  aiController,
  `      await durableDraft.flush();\n      const wasExisting = selectedProfile !== null;\n      const saved = selectedProfile\n        ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft)\n        : await createAiProviderProfile(draft);\n`,
  `      const editorDraft = await durableDraft.prepareCanonicalCommit();\n      const wasExisting = selectedProfile !== null;\n      const input = { ...draft, editorDraft };\n      const saved = selectedProfile\n        ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, input)\n        : await createAiProviderProfile(input);\n`,
  "send AI canonical draft reference",
);
aiController = replaceExact(
  aiController,
  `      await durableDraft.commit(saved.id);\n      setModels([]);\n`,
  `      durableDraft.acceptCanonicalCommit();\n      setModels([]);\n`,
  "accept AI canonical commit",
);
await write("src/controllers/aiProfilesController.ts", aiController);

// Update durable-draft mocks and assertions in screen-level tests.
const testFiles = [
  "src/screens/CardEditorScreen.test.tsx",
  "src/screens/TemplatesScreen.test.tsx",
  "src/screens/TemplatesScreen.assets.test.tsx",
  "src/screens/SettingsScreen.test.tsx",
  "src/screens/SettingsScreen.extended.test.tsx",
  "src/screens/cards/BulkEntryModal.test.tsx",
];
for (const path of testFiles) {
  let text = await read(path);
  if (!text.includes("prepareCanonicalCommit:")) {
    const anchor = `    commit: vi.fn().mockResolvedValue(undefined),\n`;
    const replacement = `    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),\n    acceptCanonicalCommit: vi.fn(),\n${anchor}`;
    if (text.includes(anchor)) text = text.replaceAll(anchor, replacement);
  }
  text = text.replaceAll("durableDraftMock.commit", "durableDraftMock.acceptCanonicalCommit");
  await write(path, text);
}

console.log("Applied atomic durable-draft handoff across frontend authoring workflows.");
