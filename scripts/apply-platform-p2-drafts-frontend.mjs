#!/usr/bin/env node

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};

let card = await read("src/screens/CardEditorScreen.tsx");
card = card.replace(
  'import { CardEditorController } from "../controllers/cardEditorController";',
  'import { CardEditorController } from "../controllers/cardEditorController";\nimport { useDurableDraft } from "../controllers/useDurableDraft";',
);
const cardPayloadInterface = `
interface CardEditorDraftPayload extends Record<string, unknown> {
  selectedDeckId: string;
  selectedTemplateKey: string;
  reverseEnabled: boolean;
  clozeSource: string;
  clozeMode: ClozeMode;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  assets: Asset[];
  tags: string[];
  templateValues: TemplateValueInput[];
}
`;
card = card.replace("interface CardEditorScreenProps {", `${cardPayloadInterface}\ninterface CardEditorScreenProps {`);
card = card.replace(
  /\n  useEffect\(\(\) => \{\n    let current = true;\n    setAssetDraftId\(null\);[\s\S]*?\n  \}, \[card\?\.noteId\]\);/u,
  "",
);
const cardHookAnchor = '  const usedAssetKey = usedAssetIds.join(",");';
const cardHook = `  const usedAssetKey = usedAssetIds.join(",");
  const durablePayload = useMemo<CardEditorDraftPayload>(
    () => ({
      selectedDeckId,
      selectedTemplateKey,
      reverseEnabled,
      clozeSource,
      clozeMode,
      frontDoc,
      backDoc,
      frontText,
      backText,
      assets,
      tags,
      templateValues,
    }),
    [
      assets,
      backDoc,
      backText,
      clozeMode,
      clozeSource,
      frontDoc,
      frontText,
      reverseEnabled,
      selectedDeckId,
      selectedTemplateKey,
      tags,
      templateValues,
    ],
  );
  const durableDraft = useDurableDraft<CardEditorDraftPayload>({
    kind: "card",
    targetEntityId: card?.noteId || undefined,
    contextEntityId: card?.noteId ? undefined : deck.id,
    baseRevision: card?.revision,
    payload: durablePayload,
    assetIds: usedAssetIds,
    onRecovered: (payload) => {
      setSelectedDeckId(payload.selectedDeckId);
      setSelectedTemplateKey(payload.selectedTemplateKey);
      setReverseEnabled(payload.reverseEnabled);
      setClozeSource(payload.clozeSource);
      setClozeMode(payload.clozeMode);
      setFrontDoc(payload.frontDoc);
      setBackDoc(payload.backDoc);
      setFrontText(payload.frontText);
      setBackText(payload.backText);
      setAssets(payload.assets);
      setTags(payload.tags);
      setTemplateValues(payload.templateValues);
      setDirty(true);
    },
  });
`;
if (!card.includes(cardHookAnchor)) throw new Error("CardEditor draft anchor missing");
card = card.replace(cardHookAnchor, cardHook);
card = card.replace(
  "  useEffect(() => {\n    onDirtyChange(dirty);\n    return () => onDirtyChange(false);\n  }, [dirty, onDirtyChange]);",
  "  useEffect(() => {\n    onDirtyChange(durableDraft.hasUnsavedLocalChanges);\n    return () => onDirtyChange(false);\n  }, [durableDraft.hasUnsavedLocalChanges, onDirtyChange]);\n\n  useEffect(() => {\n    assetDraftRef.current = durableDraft.assetDraftId;\n    setAssetDraftId(durableDraft.assetDraftId);\n  }, [durableDraft.assetDraftId]);",
);
card = card.replace(
  "    try {\n      if (selectedCustomTemplate) {",
  "    try {\n      await durableDraft.flush();\n      if (selectedCustomTemplate) {",
);
card = card.replace(
  "        markAssetDraftCommitted();\n        setDirty(false);",
  "        await durableDraft.commit();\n        markAssetDraftCommitted();\n        setDirty(false);",
);
card = card.replace(
  "      markAssetDraftCommitted();\n      setDirty(false);",
  "      await durableDraft.commit(saved.noteId);\n      markAssetDraftCommitted();\n      setDirty(false);",
);
card = card.replace(
  /  async function openFreshAssetDraft\(\): Promise<void> \{[\s\S]*?\n  \}/u,
  `  async function openFreshAssetDraft(): Promise<void> {
    await durableDraft.restart({
      selectedDeckId,
      selectedTemplateKey,
      reverseEnabled: false,
      clozeSource: "",
      clozeMode: "one_at_a_time",
      frontDoc: cloneDocument(EMPTY_RICH_DOCUMENT),
      backDoc: cloneDocument(EMPTY_RICH_DOCUMENT),
      frontText: "",
      backText: "",
      assets: selectedCustomTemplate?.assets ?? [],
      tags: [],
      templateValues: selectedCustomTemplate
        ? selectedCustomTemplate.fields.map((field) => ({
            fieldId: field.id,
            doc: cloneDocument(EMPTY_RICH_DOCUMENT),
            text: "",
          }))
        : [],
    });
  }`,
);
card = card.replace(
  /    const draftId = assetDraftRef\.current;[\s\S]*?\n    onCancel\(\);/u,
  "    try {\n      await durableDraft.discard();\n    } catch (error: unknown) {\n      setErrorMessage(`The durable draft could not be discarded: ${getErrorMessage(error)}`);\n      return;\n    }\n    assetDraftRef.current = null;\n    setAssetDraftId(null);\n    onCancel();",
);
await write("src/screens/CardEditorScreen.tsx", card);

let templates = await read("src/screens/TemplatesScreen.tsx");
templates = templates.replace(
  'import { TemplatesController } from "../controllers/templatesController";',
  'import { TemplatesController } from "../controllers/templatesController";\nimport { useDurableDraft } from "../controllers/useDurableDraft";',
);
templates = templates.replace(
  /\n  useEffect\(\(\) => \{\n    return \(\) => \{[\s\S]*?\n  \}, \[\]\);/u,
  "",
);
templates = templates.replace(
  /  async function openDraft\(next: TemplateDraft\): Promise<void> \{[\s\S]*?\n  \}/u,
  `  async function openDraft(next: TemplateDraft): Promise<void> {
    setDraft(next);
    setBaseline(next);
    setErrorMessage(null);
  }`,
);
const templateAnchor = '  const usedAssetKey = usedAssetIds.join(",");';
const templateHook = `  const usedAssetKey = usedAssetIds.join(",");
  const durableDraft = useDurableDraft<TemplateDraft & Record<string, unknown>>({
    kind: "template",
    targetEntityId: draft?.id,
    contextEntityId: draft?.id ? undefined : "0".repeat(32),
    baseRevision: draft?.revision,
    payload: (draft ?? newTemplateDraft()) as TemplateDraft & Record<string, unknown>,
    assetIds: usedAssetIds,
    enabled: draft !== null,
    onRecovered: (payload) => setDraft(payload),
  });
`;
if (!templates.includes(templateAnchor)) throw new Error("Templates draft anchor missing");
templates = templates.replace(templateAnchor, templateHook);
templates = templates.replace(
  "  useEffect(() => {\n    onDirtyChange(dirty);\n    return () => onDirtyChange(false);\n  }, [dirty, onDirtyChange]);",
  "  useEffect(() => {\n    onDirtyChange(durableDraft.hasUnsavedLocalChanges);\n    return () => onDirtyChange(false);\n  }, [durableDraft.hasUnsavedLocalChanges, onDirtyChange]);\n\n  useEffect(() => {\n    assetDraftRef.current = durableDraft.assetDraftId;\n    setAssetDraftId(durableDraft.assetDraftId);\n    setIsOpeningAssetDraft(durableDraft.status === \"opening\");\n  }, [durableDraft.assetDraftId, durableDraft.status]);",
);
templates = templates.replace(
  /    openRequestSequence\.current \+= 1;[\s\S]*?\n    setDraft\(null\);\n    setBaseline\(null\);/u,
  "    openRequestSequence.current += 1;\n    setIsOpeningAssetDraft(false);\n    try {\n      await durableDraft.discard();\n    } catch (error: unknown) {\n      setErrorMessage(`The durable template draft could not be discarded: ${getErrorMessage(error)}`);\n      return;\n    }\n    assetDraftRef.current = null;\n    setAssetDraftId(null);\n    setDraft(null);\n    setBaseline(null);",
);
templates = templates.replace(
  "    try {\n      const input = {",
  "    try {\n      await durableDraft.flush();\n      const input = {",
);
templates = templates.replace(
  /      if \(draft\.id\) \{\n        await controller\.update\(\{[\s\S]*?\n      \} else \{\n        await controller\.create\(input\);\n      \}/u,
  `      const saved = draft.id
        ? await controller.update({
            ...input,
            templateId: draft.id,
            revision: draft.revision ?? 1,
          })
        : await controller.create(input);
      await durableDraft.commit(saved.id);`,
);
await write("src/screens/TemplatesScreen.tsx", templates);

let bulk = await read("src/screens/cards/BulkEntryModal.tsx");
bulk = bulk.replace(
  'import { Modal } from "../../components/Modal";',
  'import { Modal } from "../../components/Modal";\nimport { useDurableDraft } from "../../controllers/useDurableDraft";',
);
const bulkAnchor = "  const canSubmit =";
const bulkHook = `  const durableDraft = useDurableDraft({
    kind: "bulk_cards",
    contextEntityId: "b".repeat(32),
    payload: { deckId, rows },
    onRecovered: (payload: { deckId: string; rows: DraftRow[] }) => {
      setDeckId(payload.deckId);
      setRows(payload.rows);
      sequence.current = Math.max(4, ...payload.rows.map((row) => row.id + 1));
    },
  });

  const canSubmit =`;
if (!bulk.includes(bulkAnchor)) throw new Error("Bulk draft anchor missing");
bulk = bulk.replace(bulkAnchor, bulkHook);
bulk = bulk.replace(
  "      const result = await bulkCreateCards({ deckId, rows: cardRows });",
  "      await durableDraft.flush();\n      const result = await bulkCreateCards({ deckId, rows: cardRows });\n      await durableDraft.commit();",
);
bulk = bulk.replace(
  "            onClick={onClose}\n          >\n            Cancel",
  "            onClick={() => void durableDraft.flush().finally(onClose)}\n          >\n            Close",
);
bulk = bulk.replace(
  '<div className="modal-actions bulk-entry-actions">',
  '<div className="modal-actions bulk-entry-actions">\n          <button type="button" className="text-button danger-text" disabled={isSaving} onClick={() => void durableDraft.discard().then(onClose)}>Discard draft</button>',
);
await write("src/screens/cards/BulkEntryModal.tsx", bulk);

let ai = await read("src/controllers/aiProfilesController.ts");
ai = ai.replace(
  'import { useRequestLane } from "./requestLane";',
  'import { useRequestLane } from "./requestLane";\nimport { useDurableDraft } from "./useDurableDraft";',
);
const aiAnchor = "  const modelLane = useRequestLane();";
const aiHook = `  const modelLane = useRequestLane();
  const durableDraft = useDurableDraft<AiProfileDraft & Record<string, unknown>>({
    kind: "ai_profile",
    targetEntityId: selectedProfile?.id,
    contextEntityId: selectedProfile?.id ? undefined : "a".repeat(32),
    baseRevision: selectedProfile?.revision,
    payload: (draft ?? {
      displayName: "",
      providerKind: "llama_cpp",
      baseUrl: "",
      modelId: "",
      authMode: "none",
      enabled: true,
      allowInsecureHttp: false,
      requestTimeoutMs: 60_000,
    }) as AiProfileDraft & Record<string, unknown>,
    enabled: draft !== null,
    onRecovered: (payload) => setDraft(payload),
  });`;
if (!ai.includes(aiAnchor)) throw new Error("AI draft anchor missing");
ai = ai.replace(aiAnchor, aiHook);
ai = ai.replace(
  /  const dirty = useMemo\(\(\) => \{[\s\S]*?\n  \}, \[credentialDraft, draft, selectedProfile, selectedProfileId\]\);/u,
  "  const dirty = credentialDraft !== \"\" || durableDraft.hasUnsavedLocalChanges;",
);
ai = ai.replace(
  "    try {\n      const wasExisting = selectedProfile !== null;",
  "    try {\n      await durableDraft.flush();\n      const wasExisting = selectedProfile !== null;",
);
ai = ai.replace(
  "      setSelectedProfileId(saved.id);",
  "      setSelectedProfileId(saved.id);\n      await durableDraft.commit(saved.id);",
);
ai = ai.replace(
  "  function discardDraft(): void {\n    setCredentialDraft(\"\");",
  "  function discardDraft(): void {\n    setCredentialDraft(\"\");\n    void durableDraft.discard().catch(() => undefined);",
);
await write("src/controllers/aiProfilesController.ts", ai);

await rm(join(root, "scripts", "apply-platform-p2-drafts-frontend.mjs"), { force: true });
await rm(join(root, ".github/workflows/apply-platform-p2-drafts-frontend.yml"), { force: true });
console.log("Applied durable drafts to card, template, bulk, and AI profile workflows.");
