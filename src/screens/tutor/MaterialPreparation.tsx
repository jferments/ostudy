import { Button } from "../../ui/primitives/Button";
import type { MaterialPreparationController } from "./useMaterialPreparation";
import styles from "./TutorWorkspace.module.css";

export function MaterialPreparation({ controller, disabled }: {
  controller: MaterialPreparationController;
  disabled: boolean;
}) {
  const { preview, status, pending, busy, progress, error } = controller;
  return (
    <details className={styles.options}>
      <summary>Search index maintenance</summary>
      <p className={styles.muted}>Indexing runs automatically during document processing and catches up when you send a message.
        This advanced control indexes saved material only; it never repeats knowledge extraction.</p>
      {status ? <p role="status">Last index update: {status.state}. {status.completedSources.toLocaleString()}
        {" / "}{status.totalSources.toLocaleString()} source segments.
        {status.state === "complete" ? " Changed material is indexed automatically on use." : ""}</p> : null}
      {status?.state === "partial" ? <p role="status">This is a historical partial result. Indexing saved material does not reinterpret unresolved knowledge.</p> : null}
      {status?.errorMessage ? <p role="alert">{status.errorMessage}</p> : null}
      {progress ? <div role="status">{progress.phase}: {progress.completed} / {progress.total}
        {progress.total > 0 ? <progress aria-label="Semantic preparation progress" max={progress.total} value={progress.completed} /> : null}
      </div> : null}
      {error ? <p role="alert">{error}</p> : null}
      {preview ? <section className={styles.notice} aria-label="Preparation consent">
        <p>{preview.sourceCount.toLocaleString()} source segments, {preview.sourceCharacters.toLocaleString()}
          {" "}text characters, and {preview.imageCount.toLocaleString()} image references selected.</p>
        <p>{preview.selection.includeConversations
          ? "Permitted conversations in this material selection are included; forgotten or revoked messages remain excluded."
          : "Conversation messages are excluded from this preparation."}</p>
        <p>Saved knowledge is reused without interpretation. Embeddings: <strong>{preview.embeddingProvider} / {preview.embeddingModel}</strong>
          {preview.embeddingDimensions ? ` (${preview.embeddingDimensions} dimensions)` : " (provider-default dimensions)"}.</p>
        <p>Confirmation sends selected material to these configured services and may incur charges.
          Current cached work is reused. Cards, guide edits, and review schedules are not changed.</p>
        <div className={styles.toolbar}>
          <Button variant="primary" disabled={disabled || busy || Boolean(pending)}
            onClick={() => { void controller.confirm(); }}>Update index</Button>
          <Button disabled={busy || Boolean(pending)} onClick={controller.dismissPreview}>Dismiss preview</Button>
        </div>
      </section> : <Button disabled={disabled || controller.locked} onClick={() => { void controller.review(); }}>Review index update</Button>}
      {pending && !busy ? <Button disabled={disabled} onClick={() => { void controller.confirm(); }}>Check index update</Button> : null}
      {pending || status?.state === "running" ? <Button variant="dangerSecondary" onClick={() => { void controller.cancel(); }}>Cancel index update</Button> : null}
      {!busy ? <Button variant="text" disabled={disabled} onClick={() => { void controller.reload(); }}>Refresh index status</Button> : null}
    </details>
  );
}
