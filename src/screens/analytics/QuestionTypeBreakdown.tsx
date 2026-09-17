import type { QuestionTypeAnalytics } from "../../ipc/analytics";
import { Surface } from "../../ui/primitives/Surface";
import { styles } from "./presentation";

const labels: Record<QuestionTypeAnalytics["questionType"], string> = {
  basic: "Basic",
  cloze: "Cloze",
  single_choice: "Single best answer",
  select_all: "Select all",
  true_false: "True / false",
  matching: "Matching",
  grouping: "Grouping",
  ordering: "Ordering",
  image_occlusion: "Image occlusion",
};

function percent(value: number | null): string {
  return value === null ? "—" : `${Math.round(value * 100)}%`;
}

/** Render native aggregates only; confidence never substitutes for a verdict. */
export function QuestionTypeBreakdown({ rows }: { rows: QuestionTypeAnalytics[] }) {
  if (rows.length === 0) return null;
  return (
    <Surface as="section" className={styles.analyticsPanel} aria-labelledby="question-types-heading">
      <div className={styles.panelHeadingRow}>
        <div>
          <h2 id="question-types-heading">Question types</h2>
          <p className={styles.analyticsPanelNote}>
            First checks and self-ratings are separate. Each board counts as one review.
          </p>
        </div>
      </div>
      <div className={styles.analyticsTableWrap}>
        <table className={styles.analyticsTable}>
          <caption className="tw:sr-only">
            Reviewed first attempts by question type. Checks awaiting a rating are not included.
            Retries are linked to these reviews, not additional scheduling events.
          </caption>
          <thead>
            <tr>
              <th scope="col">Type</th>
              <th scope="col">Reviews</th>
              <th scope="col">First-check accuracy</th>
              <th scope="col">Without recorded hints</th>
              <th scope="col">Self-rated</th>
              <th scope="col">Recorded hints</th>
              <th scope="col">Overrides</th>
              <th scope="col">Retries</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.questionType}>
                <th scope="row">{labels[row.questionType]}</th>
                <td>{row.reviewCount}{row.unavailableEvidenceCount > 0 ? <small>{row.unavailableEvidenceCount} without assessment evidence</small> : null}</td>
                <td>
                  {percent(row.objectiveAccuracy)}
                  {row.objectiveCount > 0 ? <>
                    <small>{row.correctCount} / {row.objectiveCount} checked</small>
                    <small>{row.incorrectCount} incorrect · {row.incompleteCount} incomplete</small>
                  </> : row.selfAssessedCount > 0 ? <small>Self-assessed only</small> : <small>No objective evidence</small>}
                </td>
                <td>{percent(row.unassistedAccuracy)}{row.unassistedObjectiveCount > 0 ? <small>{row.unassistedCorrectCount} / {row.unassistedObjectiveCount} checked</small> : null}</td>
                <td>{row.selfAssessedCount}</td>
                <td>{row.assistanceTrackedCount > 0 ? row.assistedCount : "—"}</td>
                <td>{row.overrideCount}</td>
                <td>{row.remediationCount}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Surface>
  );
}
