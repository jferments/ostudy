import type { QuestionTypeAnalytics } from "../../ipc/analytics";

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

export function questionTypeLabel(type: QuestionTypeAnalytics["questionType"]): string {
  return labels[type];
}

function percent(value: number | null): string {
  return value === null ? "—" : `${Math.round(value * 100)}%`;
}

const tableClass = "tw:w-full tw:border-collapse tw:text-left tw:text-small tw:tabular-nums tw:[&_th]:bg-muted/50 tw:[&_th]:px-3 tw:[&_th]:py-2 tw:[&_th]:text-caption tw:[&_th]:font-medium tw:[&_th]:text-muted-foreground tw:[&_td]:border-0 tw:[&_td]:border-t tw:[&_td]:border-solid tw:[&_td]:border-border tw:[&_td]:px-3 tw:[&_td]:py-2.5 tw:[&_td]:align-top tw:[&_small]:mt-1 tw:[&_small]:block tw:[&_small]:text-caption tw:[&_small]:text-muted-foreground";

/** The full evidence behind Progress's question types. Native aggregates only; confidence never substitutes for a verdict. */
export function QuestionTypeBreakdown({ rows }: { rows: QuestionTypeAnalytics[] }) {
  if (rows.length === 0) return null;
  return (
      <div className="tw:w-full tw:min-w-0 tw:overflow-x-auto tw:rounded-md tw:ring-1 tw:ring-border">
        <table className={tableClass}>
          <caption className="tw:sr-only">
            Reviewed first attempts by question type. Checks awaiting a rating are not included.
            Retries are linked to these reviews, not additional scheduling events.
          </caption>
          <thead>
            <tr>
              <th scope="col">Type</th>
              <th scope="col">Reviews</th>
              <th scope="col">Remembered</th>
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
                <td>{percent(row.rememberedRate)}</td>
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
  );
}
