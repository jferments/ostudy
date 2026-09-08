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
      <summary>Prepare selected material</summary>
      <p className={styles.muted}>Connect documents, guide sections and flashcards by meaning. Opening
        this panel does not send study content. Review the selection and providers before preparing.</p>
      {status ? <p role="status">Last preparation: {status.state}. {status.completedSources.toLocaleString()}
        {" / "}{status.totalSources.toLocaleString()} source segments.
        {status.state === "complete" ? " Edited or newly added material may need preparation again." : ""}</p> : null}
      {status?.errorMessage ? <p role="alert">{status.errorMessage}</p> : null}
      {progress ? <div role="status">{progress.phase}: {progress.completed} / {progress.total}
        {progress.total > 0 ? <progress aria-label="Semantic preparation progress" max={progress.total} value={progress.completed} /> : null}
      </div> : null}
      {error ? <p role="alert">{error}</p> : null}
      {preview ? <section className={styles.notice} aria-label="Preparation consent">
        <p>{preview.sourceCount.toLocaleString()} source segments, {preview.sourceCharacters.toLocaleString()}
          {" "}text characters, and {preview.imageCount.toLocaleString()} image references selected.</p>
        <p>Interpretation: <strong>{preview.extractionProvider} / {preview.extractionModel}</strong>.<br />
          Embeddings: <strong>{preview.embeddingProvider} / {preview.embeddingModel}</strong>
          {preview.embeddingDimensions ? ` (${preview.embeddingDimensions} dimensions)` : " (provider-default dimensions)"}.</p>
        <p>Confirmation sends selected material to these configured services and may incur charges.
          Current cached work is reused. Cards, guide edits, and review schedules are not changed.</p>
        <div className={styles.toolbar}>
          <Button variant="primary" disabled={disabled || busy || Boolean(pending)}
            onClick={() => { void controller.confirm(); }}>Confirm and prepare</Button>
          <Button disabled={busy || Boolean(pending)} onClick={controller.dismissPreview}>Dismiss preview</Button>
        </div>
      </section> : <Button disabled={disabled || controller.locked} onClick={() => { void controller.review(); }}>Review preparation</Button>}
      {pending && !busy ? <Button disabled={disabled} onClick={() => { void controller.confirm(); }}>Retry or check preparation</Button> : null}
      {pending || status?.state === "running" ? <Button variant="dangerSecondary" onClick={() => { void controller.cancel(); }}>Cancel preparation</Button> : null}
      {!busy ? <Button variant="text" disabled={disabled} onClick={() => { void controller.reload(); }}>Refresh preparation status</Button> : null}
    </details>
  );
}
