import styles from "./CardListingPreview.module.css";

interface CardListingPreviewProps {
  question: string;
  answer: string;
  emptyFallback?: string;
  className?: string;
}

/**
 * Shared compact card-content presentation used anywhere a card is listed.
 * The surrounding row may expose different controls or metadata, but question
 * and answer previews always keep the same two-line, width-driven contract.
 */
export function CardListingPreview({
  question,
  answer,
  emptyFallback = "Image card",
  className = "",
}: CardListingPreviewProps) {
  return (
    <span className={`${styles.preview} ${className}`.trim()}>
      <PreviewLine label="Q:" accessibleLabel="Question:" value={question} emptyFallback={emptyFallback} />
      <PreviewLine label="A:" accessibleLabel="Answer:" value={answer} emptyFallback={emptyFallback} />
    </span>
  );
}

function PreviewLine({
  label,
  accessibleLabel,
  value,
  emptyFallback,
}: {
  label: "Q:" | "A:";
  accessibleLabel: "Question:" | "Answer:";
  value: string;
  emptyFallback: string;
}) {
  return (
    <span className={styles.line}>
      <span className={styles.label} aria-hidden="true">
        {label}
      </span>
      <span className="sr-only">{accessibleLabel} </span>
      <strong className={styles.text}>{formatCardListingText(value, emptyFallback)}</strong>
    </span>
  );
}

export function formatCardListingText(value: string, emptyFallback = "Image card"): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return emptyFallback;
  return normalized.length > 512 ? `${normalized.slice(0, 509)}…` : normalized;
}
