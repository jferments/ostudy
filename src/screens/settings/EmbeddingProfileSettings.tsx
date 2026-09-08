import { useEffect, useRef, useState } from "react";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { getErrorCode, getErrorMessage } from "../../ipc/common";
import {
  getEmbeddingOptions, listEmbeddingModels, saveEmbeddingOptions, testEmbeddingProfile,
  type EmbeddingInputPolicy, type EmbeddingModelChoice, type EmbeddingOptions, type EmbeddingTestResult,
} from "../../ipc/embeddings";
import { Button } from "../../ui/primitives/Button";
import styles from "./Settings.module.css";

const DIMENSIONS = [256, 512, 768, 1024, 1536, 2048, 3072, 4096];

/** Mounted by saved profile/revision so late reads cannot populate another model. */
export function EmbeddingProfileSettings({ controller }: { controller: AiProfilesController }) {
  const profile = controller.selectedProfile;
  if (!profile || !["openrouter", "llama_cpp"].includes(profile.providerKind)) return null;
  return <SavedEmbeddingSettings key={`${profile.id}:${profile.revision}`} controller={controller} profileId={profile.id} />;
}

function SavedEmbeddingSettings({ controller, profileId }: { controller: AiProfilesController; profileId: string }) {
  const [options, setOptions] = useState<EmbeddingOptions | null>(null);
  const [models, setModels] = useState<EmbeddingModelChoice[]>([]);
  const [result, setResult] = useState<EmbeddingTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<Parameters<typeof saveEmbeddingOptions>[0] | null>(null);
  const writing = useRef(false);
  const alive = useRef(true);
  const disabled = busy || controller.mutating || controller.dirty || Boolean(pending);

  useEffect(() => {
    alive.current = true;
    void getEmbeddingOptions(profileId).then((saved) => { if (alive.current) setOptions(saved); })
      .catch((failure: unknown) => { if (alive.current) setError(getErrorMessage(failure)); });
    return () => { alive.current = false; };
  }, [profileId]);

  async function run(action: () => Promise<void>): Promise<void> {
    if (writing.current || controller.mutating || controller.dirty) return;
    writing.current = true;
    setBusy(true);
    setError(null);
    try { await action(); }
    catch (failure: unknown) { if (alive.current) setError(getErrorMessage(failure)); }
    finally { writing.current = false; if (alive.current) setBusy(false); }
  }

  async function save(input: Parameters<typeof saveEmbeddingOptions>[0]): Promise<void> {
    setPending(input);
    setResult(null);
    try {
      const saved = await saveEmbeddingOptions(input);
      if (alive.current) { setOptions(saved); setPending(null); }
    } catch (failure: unknown) {
      if (getErrorCode(failure) === "embedding_options_changed") {
        // A competing options edit is a rejection, not an ambiguous save. Read
        // its current revision; never retry our old selection over that edit.
        const saved = await getEmbeddingOptions(profileId);
        if (alive.current) { setOptions(saved); setPending(null); }
      }
      throw failure;
    }
  }

  function change(patch: Partial<Pick<EmbeddingOptions, "dimensions" | "inputPolicy">>): void {
    if (!options || disabled) return;
    const input = { profileId, expectedRevision: options.revision,
      dimensions: options.dimensions, inputPolicy: options.inputPolicy, ...patch };
    void run(() => save(input));
  }

  return (
    <section className={styles.settingsCard} aria-label="Embedding model settings">
      <h2>Semantic embeddings</h2>
      <p className={styles.note}>Use a separate profile for an embedding model. It runs at the configured
        provider, not on this device. The chat-model catalog may not list embedding models.</p>
      <div className={styles.actionRow}>
        <Button disabled={disabled} onClick={() => { void run(async () => {
          const loaded = await listEmbeddingModels(profileId);
          if (alive.current) setModels(loaded);
        }); }}>Load embedding models</Button>
        <Button disabled={disabled || !options || !controller.selectedProfile?.modelId} onClick={() => { void run(async () => {
          setResult(null);
          const checked = await testEmbeddingProfile(profileId);
          if (alive.current) setResult(checked);
        }); }}>Test embeddings</Button>
      </div>
      {models.length ? <label className={styles.stackedField}>
        <span>Embedding model</span>
        <select value={models.some((model) => model.id === controller.draft?.modelId) ? controller.draft?.modelId : ""}
          disabled={busy || controller.mutating} onChange={(event) => {
            if (event.currentTarget.value) controller.updateDraft({ modelId: event.currentTarget.value, reasoningEffort: "none" });
          }}>
          <option value="">Choose an embedding model</option>
          {models.map((model) => <option key={model.id} value={model.id}>{model.name} — {model.id}</option>)}
        </select>
      </label> : null}
      <p className={styles.note}>Save a changed model using “Save profile” above before testing. Loading
        the catalog sends no study content. The embedding test sends only built-in, non-personal
        query/document examples; provider charges may apply.</p>
      <div className={styles.formGrid}>
        <label className={styles.stackedField}><span>Embedding dimensions</span>
          <select disabled={disabled || !options} value={options?.dimensions ?? ""}
            onChange={(event) => change({ dimensions: event.currentTarget.value ? Number(event.currentTarget.value) : null })}>
            <option value="">Provider default</option>
            {options?.dimensions && !DIMENSIONS.includes(options.dimensions) ? <option value={options.dimensions}>{options.dimensions} (saved)</option> : null}
            {DIMENSIONS.map((count) => <option key={count} value={count}>{count}</option>)}
          </select>
        </label>
        <label className={styles.stackedField}><span>Embedding input policy</span>
          <select disabled={disabled || !options} value={options?.inputPolicy ?? "automatic"}
            onChange={(event) => change({ inputPolicy: event.currentTarget.value as EmbeddingInputPolicy })}>
            <option value="automatic">Automatic for the selected model</option>
            <option value="symmetric">Same format for query and document</option>
            <option value="qwen_retrieval">Qwen retrieval instructions</option>
            <option value="search_types" disabled={controller.selectedProfile?.providerKind !== "openrouter"}>Provider search-query / search-document types</option>
          </select>
        </label>
      </div>
      <p className={styles.note}>Options save immediately. Use provider-default dimensions unless the
        model supports a smaller size. Changing dimensions or input policy creates a different
        index space; prepare the selected material again. A successful test verifies the protocol
        and dimensions, not retrieval quality.</p>
      {controller.dirty ? <p role="status">Save or discard the profile changes before configuring or testing embeddings.</p> : null}
      {busy ? <p role="status">Checking embedding configuration…</p> : null}
      {result ? <p role="status">Query and document embeddings verified: {result.dimensions} dimensions.</p> : null}
      {error ? <p role="alert">{error}</p> : null}
      {pending ? <Button disabled={busy || controller.dirty} onClick={() => { void run(() => save(pending)); }}>Retry or check options save</Button> : null}
    </section>
  );
}
