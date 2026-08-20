import type { DeckCardSummary } from "../../ipc/decks";
import styles from "./CardPreviewRow.module.css";

interface CardPreviewRowProps {
  card: DeckCardSummary;
  onEdit: () => void;
  onDelete: () => void;
}

export function CardPreviewRow({ card, onEdit, onDelete }: CardPreviewRowProps) {
  return (
    <article className={styles.row} data-testid={`card-preview-${card.noteId}`}>
      <button
        type="button"
        className={styles.main}
        aria-label={`Edit flashcard. Question: ${accessiblePreview(card.questionExcerpt, card.imageCount > 0)}. Answer: ${accessiblePreview(card.answerExcerpt, card.imageCount > 0)}.`}
        onClick={onEdit}
      >
        <span className={styles.preview}>
          <PreviewLine
            prefix="Q:"
            text={card.questionExcerpt}
            hasImage={card.imageCount > 0}
          />
          <PreviewLine
            prefix="A:"
            text={card.answerExcerpt}
            hasImage={card.imageCount > 0}
          />
        </span>
      </button>

      <div className={styles.wideActions}>
        {card.reversible ? (
          <span className={styles.reversible} title="Reversible card" aria-label="Reversible card">
            ↔
          </span>
        ) : null}
        <button type="button" onClick={onEdit}>Edit</button>
        <button type="button" className={styles.danger} onClick={onDelete}>Delete</button>
      </div>

      <details className={styles.menu}>
        <summary aria-label="Flashcard actions" title="Flashcard actions">⋮</summary>
        <div role="menu">
          <button type="button" role="menuitem" onClick={onEdit}>Edit</button>
          <button type="button" role="menuitem" className={styles.danger} onClick={onDelete}>
            Delete
          </button>
        </div>
      </details>
    </article>
  );
}

function PreviewLine({
  prefix,
  text,
  hasImage,
}: {
  prefix: "Q:" | "A:";
  text: string;
  hasImage: boolean;
}) {
  return (
    <span className={styles.line}>
      <span className={styles.prefix} aria-hidden="true">{prefix}</span>
      <strong title={accessiblePreview(text, hasImage)}>{accessiblePreview(text, hasImage)}</strong>
    </span>
  );
}

function accessiblePreview(text: string, hasImage: boolean): string {
  const normalized = text.replace(/\s+/gu, " ").trim();
  if (normalized !== "") return normalized;
  return hasImage ? "Image content" : "Empty";
}
