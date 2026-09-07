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
        have simplified formatting.
      </p>
      <p>
        Cards are now independent, editable oStudy cards with fresh schedules. Original
        deck paths are stored as tags. Anki review history, linked-note editing, and
        exact CSS styling are not retained. Review the converted cards before studying.
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
