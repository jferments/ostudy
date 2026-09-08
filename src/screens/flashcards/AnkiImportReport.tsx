import type { AnkiImportReport } from "../../ipc/decks";
import styles from "./AnkiImportReport.module.css";

interface AnkiImportReportViewProps {
  report: AnkiImportReport;
  message: string;
}

export function AnkiImportReportView({ report, message }: AnkiImportReportViewProps) {
  return (
    <section className={styles.report} aria-label="Anki import report">
      <p role="status">{message}</p>
      <p>
        {report.importedCardCount} of {report.sourceCardCount} Anki cards imported.
        {" "}{report.skippedCardCount} skipped; {report.simplifiedCardCount} imported cards
        required content simplification.
      </p>
      {report.nativeStyleCardCount > 0 ? (
        <p>
          {report.nativeStyleCardCount} imported cards use oStudy&apos;s native appearance instead
          of Anki CSS. This is an appearance conversion, not a skipped-card error.
        </p>
      ) : null}
      {report.typedAnswerCardCount > 0 ? (
        <p>
          {report.typedAnswerCardCount} typed-answer cards were preserved as ordinary
          question/reveal-answer cards; Anki&apos;s typing/comparison widget is not retained.
        </p>
      ) : null}
      {report.nativeClozeCardCount > 0 ? (
        <p>
          {report.nativeClozeCardCount} compatible Anki cloze cards were converted to native
          oStudy Fill in the blank cards with independent schedules.
        </p>
      ) : null}
      {report.clozeFallbackCardCount > 0 ? (
        <p>
          {report.clozeFallbackCardCount} complex Anki cloze cards kept their rendered content
          as Basic cards because their extra fields or media cannot yet be represented by the
          native Fill in the blank type without losing information.
        </p>
      ) : null}
      <p>
        Cards are now editable oStudy cards with fresh schedules. Original deck paths are stored
        as tags. Anki review history and linked-note editing are not retained. Review converted
        cards against the source before studying.
      </p>
      {report.issues.length > 0 ? (
        <details className={styles.details}>
          <summary>Compatibility details ({report.issues.length})</summary>
          <ol className={styles.issues}>
            {report.issues.map((issue, index) => (
              <li key={`${issue.sourceCardId}-${index}`}>
                <strong>Anki card {issue.sourceCardId}:</strong> {issue.message}
              </li>
            ))}
          </ol>
          {report.omittedIssueCount > 0 ? (
            <p>{report.omittedIssueCount} additional details omitted from this bounded report.</p>
          ) : null}
        </details>
      ) : null}
    </section>
  );
}
