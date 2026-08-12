#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
let source = execFileSync("git", ["show", "HEAD:src/screens/TemplatesScreen.tsx"], {
  cwd: root,
  encoding: "utf8",
});

source = source.replace(
  'import { TemplatesController } from "../controllers/templatesController";',
  'import { TemplatesController } from "../controllers/templatesController";\nimport { useDurableDraft } from "../controllers/useDurableDraft";',
);

const cleanupStart = source.indexOf("  useEffect(() => {\n    return () => {");
if (cleanupStart >= 0) {
  const cleanupEnd = source.indexOf("\n  }, []);", cleanupStart);
  if (cleanupEnd < 0) throw new Error("Templates cleanup effect boundary missing");
  source = source.slice(0, cleanupStart) + source.slice(cleanupEnd + "\n  }, []);".length);
}

const openStart = source.indexOf("  async function openDraft(next: TemplateDraft): Promise<void> {");
const beginCreateStart = source.indexOf("\n  function beginCreate(): void {", openStart);
if (openStart < 0 || beginCreateStart < 0) throw new Error("Templates openDraft boundary missing");
source = source.slice(0, openStart) + `  async function openDraft(next: TemplateDraft): Promise<void> {
    setDraft(next);
    setBaseline(next);
    setErrorMessage(null);
  }
` + source.slice(beginCreateStart);

const anchor = '  const usedAssetKey = usedAssetIds.join(",");';
if (!source.includes(anchor)) throw new Error("Templates durable hook anchor missing");
source = source.replace(anchor, `${anchor}
  const durableDraft = useDurableDraft<TemplateDraft & Record<string, unknown>>({
    kind: "template",
    targetEntityId: draft?.id,
    contextEntityId: draft?.id ? undefined : "0".repeat(32),
    baseRevision: draft?.revision,
    payload: (draft ?? newTemplateDraft()) as TemplateDraft & Record<string, unknown>,
    assetIds: usedAssetIds,
    enabled: draft !== null,
    onRecovered: (payload) => setDraft(payload),
  });`);
source = source.replace(
  `  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);`,
  `  useEffect(() => {
    onDirtyChange(durableDraft.hasUnsavedLocalChanges);
    return () => onDirtyChange(false);
  }, [durableDraft.hasUnsavedLocalChanges, onDirtyChange]);

  useEffect(() => {
    assetDraftRef.current = durableDraft.assetDraftId;
    setAssetDraftId(durableDraft.assetDraftId);
    setIsOpeningAssetDraft(durableDraft.status === "opening");
  }, [durableDraft.assetDraftId, durableDraft.status]);`,
);

const cancelStart = source.indexOf("  async function cancelEdit(): Promise<void> {");
const saveStart = source.indexOf("\n  async function saveTemplate(): Promise<void> {", cancelStart);
if (cancelStart < 0 || saveStart < 0) throw new Error("Templates cancelEdit boundary missing");
source = source.slice(0, cancelStart) + `  async function cancelEdit(): Promise<void> {
    if (isSaving) return;
    if (dirty) {
      const accepted = await confirm({
        title: "Discard template changes?",
        description: "The unsaved fields, card rules, and layout changes will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
    }
    openRequestSequence.current += 1;
    setIsOpeningAssetDraft(false);
    try {
      await durableDraft.discard();
    } catch (error: unknown) {
      setErrorMessage(\`The durable template draft could not be discarded: \${getErrorMessage(error)}\`);
      return;
    }
    assetDraftRef.current = null;
    setAssetDraftId(null);
    setDraft(null);
    setBaseline(null);
    setErrorMessage(null);
  }
` + source.slice(saveStart);

source = source.replace(
  `    try {
      const input = {`,
  `    try {
      await durableDraft.flush();
      const input = {`,
);
const saveChoice = /      if \(draft\.id\) \{[\s\S]*?      \} else \{\n        await controller\.create\(input\);\n      \}/u;
if (!saveChoice.test(source)) throw new Error("Templates save branch missing");
source = source.replace(
  saveChoice,
  `      const saved = draft.id
        ? await controller.update({
            ...input,
            templateId: draft.id,
            revision: draft.revision ?? 1,
          })
        : await controller.create(input);
      await durableDraft.commit(saved.id);`,
);

await writeFile(join(root, "src/screens/TemplatesScreen.tsx"), source, "utf8");
console.log("Rebuilt TemplatesScreen durable-draft integration from the clean branch source.");
