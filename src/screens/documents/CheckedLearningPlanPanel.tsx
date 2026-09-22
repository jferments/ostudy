import type { DocumentSummary } from "../../ipc/documents";
import type { DocumentLearningPlan, SourceLocator } from "../../ipc/studyGuides";

export function CheckedLearningPlanPanel({
  document,
  plan,
}: {
  document: DocumentSummary;
  plan: DocumentLearningPlan;
}) {
  const total = document.generationWindowCount;
  const completed = Math.min(document.generationCompletedWindowCount, total);
  const remaining = Math.max(total - completed, 0);

  return (
    <details className="tw:mt-3 tw:rounded-lg tw:border tw:border-solid tw:border-primary/20 tw:bg-accent/20 tw:p-3">
      <summary className="tw:cursor-pointer tw:select-none tw:text-sm tw:font-medium tw:text-primary">
        Checked learning plan ready · {plan.topics.length} topic{plan.topics.length === 1 ? "" : "s"}
      </summary>
      <div className="tw:mt-3 tw:grid tw:min-w-0 tw:gap-4">
        <p className="tw:m-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground">
          This read-only plan has already passed native source-grounding checks. Flashcard authoring
          and final curation are still running; nothing shown here is a provisional learner card.
        </p>
        <p className="tw:m-0 tw:text-sm tw:font-medium tw:tabular-nums">
          {total > 0
            ? `Card authoring: ${completed} of ${total} windows finished · ${remaining} remaining.`
            : "Card authoring is still preparing its source windows."}
        </p>
        <section className="tw:grid tw:gap-2">
          <h3 className="tw:m-0 tw:text-base tw:font-semibold">Summary</h3>
          <p className="tw:m-0 tw:leading-relaxed">{plan.summary}</p>
        </section>
        {plan.learningObjectives.length > 0 ? (
          <section className="tw:grid tw:gap-2">
            <h3 className="tw:m-0 tw:text-base tw:font-semibold">Learning objectives</h3>
            <ul className="tw:m-0 tw:grid tw:gap-1 tw:pl-5">
              {plan.learningObjectives.map((objective, index) => (
                <li key={`objective:${index}`}>{objective}</li>
              ))}
            </ul>
          </section>
        ) : null}
        {plan.keyConcepts.length > 0 ? (
          <section className="tw:grid tw:gap-2">
            <h3 className="tw:m-0 tw:text-base tw:font-semibold">Key concepts</h3>
            <div className="tw:flex tw:flex-wrap tw:gap-2">
              {plan.keyConcepts.map((concept, index) => (
                <span
                  key={`concept:${index}`}
                  className="tw:rounded-md tw:bg-muted tw:px-2 tw:py-1 tw:text-sm"
                >
                  {concept}
                </span>
              ))}
            </div>
          </section>
        ) : null}
        {plan.topics.length > 0 ? (
          <section className="tw:grid tw:gap-2">
            <h3 className="tw:m-0 tw:text-base tw:font-semibold">Topics</h3>
            <ol className="tw:m-0 tw:grid tw:gap-3 tw:pl-5">
              {plan.topics.map((topic) => (
                <li key={topic.id} className="tw:min-w-0">
                  <strong className="tw:block tw:break-words">{topic.title}</strong>
                  <span className="tw:text-xs tw:text-muted-foreground">
                    {sourceLabel(topic.sourceLocator)}
                  </span>
                  <p className="tw:my-1 tw:break-words tw:leading-relaxed">{topic.summary}</p>
                </li>
              ))}
            </ol>
          </section>
        ) : null}
      </div>
    </details>
  );
}

function sourceLabel(locator: SourceLocator): string {
  const numbers = locator.unitNumbers;
  const unit = locator.unitKind.replaceAll("_", " ");
  const range = numbers.length === 0
    ? unit
    : numbers.length === 1
      ? `${unit} ${numbers[0]}`
      : `${unit}s ${numbers[0]}–${numbers[numbers.length - 1]}`;
  return locator.sectionLabel ? `${locator.sectionLabel} · ${range}` : range;
}
