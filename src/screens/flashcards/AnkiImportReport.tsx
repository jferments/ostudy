import type { AnkiImportReport } from "../../ipc/decks";
import { Disclosure } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Badge } from "../../ui/primitives/Badge";

interface AnkiImportReportViewProps { report: AnkiImportReport; message: string; }

/** This is a receipt of a committed import, not an import preview. */
export function AnkiImportReportView({ report, message }: AnkiImportReportViewProps) {
  const nativeStyleCardCount = report.nativeStyleCardCount ?? 0;
  const typedAnswerCardCount = report.typedAnswerCardCount ?? 0;
  const nativeClozeCardCount = report.nativeClozeCardCount ?? 0;
  const clozeFallbackCardCount = report.clozeFallbackCardCount ?? 0;
  const javascriptSalvagedCardCount = report.javascriptSalvagedCardCount ?? 0;
  return <section className="tw:grid tw:min-w-0 tw:gap-4 tw:break-words tw:text-label tw:leading-relaxed tw:[&_p]:m-0" aria-label="Anki import report">
    <InlineNotice kind="success">{message}</InlineNotice>
    <div className="tw:flex tw:flex-wrap tw:gap-2"><Badge tone="success">{report.importedCardCount} imported</Badge><Badge>{report.skippedCardCount} skipped</Badge></div>
    <p>{report.importedCardCount} of {report.sourceCardCount} Anki cards imported. {" "}{report.skippedCardCount} skipped; {report.simplifiedCardCount} imported cards required content simplification.</p>
    {nativeStyleCardCount > 0 ? <p>{nativeStyleCardCount} imported cards use oStudy&apos;s native appearance instead of Anki CSS. This is an appearance conversion, not a skipped-card error.</p> : null}
    {javascriptSalvagedCardCount > 0 ? <p>{javascriptSalvagedCardCount} cards contained JavaScript or inline script handlers. oStudy did not execute that code; complete static question/answer content was retained after removing it.</p> : null}
    {typedAnswerCardCount > 0 ? <p>{typedAnswerCardCount} typed-answer cards were preserved as ordinary question/reveal-answer cards; Anki&apos;s typing/comparison widget is not retained.</p> : null}
    {nativeClozeCardCount > 0 ? <p>{nativeClozeCardCount} compatible Anki cloze cards were converted to native oStudy Fill in the blank cards with independent schedules.</p> : null}
    {clozeFallbackCardCount > 0 ? <p>{clozeFallbackCardCount} complex Anki cloze cards kept their rendered content as Basic cards because their extra fields or media cannot yet be represented by the native Fill in the blank type without losing information.</p> : null}
    <p className="tw:text-muted-foreground">Cards are now editable oStudy cards with fresh schedules. Original deck paths are stored as tags. Anki review history and linked-note editing are not retained. Review converted cards against the source before studying.</p>
    {report.issues.length > 0 ? <Disclosure title={`Compatibility details (${report.issues.length})`}>
      <ol className="tw:m-0 tw:grid tw:max-h-80 tw:gap-3 tw:overflow-auto tw:pl-5">
        {report.issues.map((issue, index) => <li key={`${issue.sourceCardId}-${index}`}><strong>Anki card {issue.sourceCardId}:</strong> {issue.message}</li>)}
      </ol>
      {report.omittedIssueCount > 0 ? <p>{report.omittedIssueCount} additional details omitted from this bounded report.</p> : null}
    </Disclosure> : null}
  </section>;
}
