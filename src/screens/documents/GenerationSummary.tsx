import { useId } from "react";
import type { GenerationReport } from "../../ipc/documents";
import type { QuestionFamily } from "../../ipc/questionGeneration";
import { MetadataItem, MetadataList } from "../../ui/overlays/MetadataDialog";

const labels: Record<QuestionFamily, string> = {
  recall: "Basic / Cloze",
  single_choice: "Single best answer",
  select_all: "Select all",
  true_false: "True / false",
  matching: "Matching",
  grouping: "Grouping",
  ordering: "Ordering",
};

/** Metadata for a completed pass, never a preview or a claim of verified knowledge. */
export function GenerationSummary({ report }: { report: GenerationReport | null | undefined }) {
  const heading = useId();
  if (!report) return null;
  const policy = report.requestedPolicy;
  const weightTotal = policy.allowedTypes.reduce((sum, kind) => sum + (policy.weights[kind] ?? 0), 0);
  return (
    <section aria-labelledby={heading} className="tw:mt-6 tw:grid tw:gap-3">
      <h3 id={heading} className="tw:m-0 tw:text-body tw:font-semibold">Latest card generation</h3>
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
        Completed {new Date(report.generatedAt).toLocaleString()}. These counts describe this pass, not the current deck.
      </p>
      <MetadataList>
        <MetadataItem label="Saved" value={`${report.savedCardCount} cards from ${report.savedNoteCount} notes`} />
        <MetadataItem label="Candidate selection" value={`${report.selectedNoteCount} of ${report.candidateNoteCount} notes retained · ${report.curationRemovedCount} removed during curation`} />
        <MetadataItem label="Duplicates skipped on save" value={String(report.skippedDuplicateCount)} />
        <MetadataItem label="Assessed items" value={String(report.itemCount)} />
        <MetadataItem label="New-card source coverage" value={report.availableKnowledgeUnits > 0 ? `${report.linkedKnowledgeUnits} / ${report.availableKnowledgeUnits} available knowledge units linked` : "No source coverage recorded"} />
        <MetadataItem label="Component evidence links" value={String(report.evidenceBindingCount)} />
        <MetadataItem label="Compound item limit" value={String(policy.maxCompoundItems)} />
      </MetadataList>
      <div className="tw:max-w-full tw:overflow-x-auto">
        <table className="tw:w-full tw:border-collapse tw:text-left tw:text-small tw:[&_th]:p-2 tw:[&_td]:p-2">
          <caption className="tw:sr-only">Requested soft mix and saved question workload</caption>
          <thead>
            <tr>
              <th scope="col">Type</th>
              <th scope="col">Requested mix</th>
              <th scope="col">Saved mix</th>
              <th scope="col">Cards</th>
              <th scope="col">Items</th>
            </tr>
          </thead>
          <tbody>
            {policy.allowedTypes.map((family) => {
              const row = report.types.find((type) => type.family === family);
              const cards = row?.cardCount ?? 0;
              return (
                <tr key={family}>
                  <th scope="row">{labels[family]}</th>
                  <td>{weightTotal > 0 ? `${Math.round((policy.weights[family] ?? 0) / weightTotal * 100)}%` : "—"}</td>
                  <td>{report.savedCardCount > 0 ? `${Math.round(cards / report.savedCardCount * 100)}%` : "—"}</td>
                  <td>{cards}</td>
                  <td>{row?.itemCount ?? 0}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
        The requested mix is a preference, not a quota. Source fit and coverage take priority; missing formats do not require filler.
        Items count options, pairs, group members, ordered steps, or recall prompts. A compound board is still one scheduled card.
      </p>
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
        Coverage counts distinct saved source units available to this pass, not all ideas in the document. Existing cards may already cover other units.
        Source links identify evidence; they do not certify that a generated answer is correct. Review generated material before relying on it.
      </p>
    </section>
  );
}
