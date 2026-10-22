import { type ReactNode, useId, useRef, useState } from "react";

import { StudyMarkdown } from "../../components/StudyMarkdown";
import type {
  SourceLocator,
  StudyGuideCalloutKind,
  StudyGuideRetrievalCheck,
  StudyGuideSectionContent,
  StudyGuideTable,
} from "../../ipc/studyGuides";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";
import { VisuallyHidden } from "../../ui/primitives/VisuallyHidden";
import { StudyGuideSourceFigures } from "../StudyGuideSourceFigures";
import { sectionBlocks } from "./guideModel";
import type { DesignedSectionPage } from "./useDesignedSectionPage";

/** How far a learner got with one retrieval check. Kept in memory only. */
export interface CheckProgress { revealed: boolean; result: "yes" | "not_yet" | null }
export type CheckProgressMap = Readonly<Record<number, CheckProgress>>;

export interface StudyGuideStructuredContentProps {
  content: StudyGuideSectionContent;
  figureSource?: { documentId: string; locator: SourceLocator } | null;
  /**
   * The level of each block heading. 2 means the section is the page, as in the Chapter reader,
   * and sets the reading size; 3 or 4 fit a section inside a longer page.
   */
  headingLevel?: 2 | 3 | 4;
  /** Prefixes the block heading ids, so an outline can link to them. */
  idPrefix?: string;
  /** Numbers figures "Figure N.M" after this section number. */
  sectionNumber?: number | null;
  /** The Chapter reader lists important terms beside the text instead. */
  showTerms?: boolean;
  /** Check-yourself progress owned by the caller; without it the block keeps its own. */
  checkProgress?: CheckProgressMap;
  onCheckProgressChange?: (next: CheckProgressMap) => void;
  /**
   * The section set as a designed page: its image, or "loading" while it is laid out. The
   * blocks stay for screen readers and the check questions stay interactive below it.
   */
  designedPage?: DesignedSectionPage;
  /** Drawn over the designed page, at its size: the learner's marks. */
  designedOverlay?: ReactNode;
}

type Level = 2 | 3 | 4 | 5;
const headingElements = { 2: "h2", 3: "h3", 4: "h4", 5: "h5" } as const;
const headingSizes: Record<Level, string> = {
  2: "tw:text-section",
  3: "tw:text-body-large",
  4: "tw:text-body",
  5: "tw:text-body",
};

function BlockHeading({ level, id, className, children }: { level: Level; id: string; className?: string; children: ReactNode }) {
  const Heading = headingElements[level];
  // Focusable by script only, so an outline jump can move focus to the block it scrolled to.
  return <Heading id={id} tabIndex={-1}
    className={cn("tw:m-0 tw:scroll-mt-24 tw:break-words tw:font-semibold tw:tracking-tight tw:text-foreground tw:outline-none", headingSizes[level], className)}>
    {children}
  </Heading>;
}

/**
 * The semantic blocks of one generated section, in the order a learner reads them: the summary,
 * explanations, tables, comparisons, process, callouts, figures, worked examples, misconceptions
 * and the retrieval checks. Document and collection guides share it.
 */
export function StudyGuideStructuredContent({
  content,
  figureSource = null,
  headingLevel = 3,
  idPrefix,
  sectionNumber = null,
  showTerms = true,
  checkProgress,
  onCheckProgressChange,
  designedPage = null,
  designedOverlay = null,
}: StudyGuideStructuredContentProps) {
  const generatedId = `guide-block-${useId().replaceAll(":", "")}`;
  const prefix = idPrefix ?? generatedId;
  const page = headingLevel === 2;
  const level = headingLevel as Level;
  const blockKeys = new Set(sectionBlocks(content).map((block) => block.key));
  const id = (key: string) => `${prefix}-${key}`;
  // Interface-font blocks sit a step below the reading text, as in the mockups.
  const uiText = page ? "tw:text-body" : "tw:text-small";
  return (
    <div className={cn(
      "tw:flex tw:min-w-0 tw:flex-col tw:break-words tw:font-(family-name:--card-font-family) tw:leading-(--ostudy-font-line-height-reading)",
      page ? "tw:gap-6 tw:text-body-large tw:compact:gap-5 tw:compact:text-body" : "tw:gap-5 tw:text-body",
    )}>
      {designedPage === "loading" ? (
        <div aria-hidden="true" className="tw:aspect-[3/4] tw:w-full tw:rounded-md tw:bg-card tw:ring-1 tw:ring-border" />
      ) : designedPage !== null ? (
        <div className="tw:relative tw:overflow-hidden tw:rounded-md tw:shadow-md tw:ring-1 tw:ring-border">
          <img src={designedPage.url} alt="" className="tw:block tw:h-auto tw:w-full" />
          {designedOverlay}
        </div>
      ) : null}
      {/* The designed page is a picture of these blocks, which stay for screen readers. */}
      <div className={designedPage === null ? "tw:contents" : "tw:sr-only"}>
        {blockKeys.has("minute") ? (
          <GuideMinuteBox id={id("minute")} page={page} level={level}>
            {content.overview.trim() ? <StudyMarkdown text={content.overview} /> : null}
            {content.keyPoints.length > 0 ? (
              <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-5 tw:marker:text-primary">
                {content.keyPoints.map((point, index) => <li key={`${index}-${point}`} className="tw:pl-1"><StudyMarkdown text={point} /></li>)}
              </ul>
            ) : null}
          </GuideMinuteBox>
        ) : null}
        {content.explanations.map((explanation, index) => (
          <section key={`${index}-${explanation.heading}`} aria-labelledby={id(`explanation-${index}`)} className={cn("tw:grid tw:gap-3", page && "tw:mt-3")}>
            <BlockHeading level={level} id={id(`explanation-${index}`)}><StudyMarkdown text={explanation.heading} inline /></BlockHeading>
            <div className="tw:[&_p]:my-4"><StudyMarkdown text={explanation.body} /></div>
          </section>
        ))}
        {content.tables.map((table, index) => <GuideTable key={`${index}-${table.title}`} table={table} uiText={uiText} />)}
        {blockKeys.has("comparisons") ? (
          <section aria-labelledby={id("comparisons")} className={cn("tw:grid tw:gap-3", page && "tw:mt-3")}>
            <BlockHeading level={level} id={id("comparisons")}>Comparisons</BlockHeading>
            <ul className={cn("tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0 tw:font-sans", uiText)}>
              {content.comparisons.map((comparison, index) => (
                <li key={`${index}-${comparison.subjectA}-${comparison.subjectB}`}
                  className="tw:grid tw:gap-1 tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-card tw:px-4 tw:py-3">
                  <strong className="tw:font-semibold tw:text-foreground">
                    <StudyMarkdown text={comparison.subjectA} inline /> and <StudyMarkdown text={comparison.subjectB} inline />
                  </strong>
                  <div className="tw:text-(--ostudy-color-text-secondary)"><StudyMarkdown text={comparison.relationship} /></div>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
        {blockKeys.has("process") ? (
          <section aria-labelledby={id("process")} className={cn("tw:grid tw:gap-3", page && "tw:mt-3")}>
            <BlockHeading level={level} id={id("process")}>Process</BlockHeading>
            <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
              {content.processSteps.map((step, index) => (
                <li key={`${index}-${step}`} className="tw:flex tw:min-w-0 tw:gap-3">
                  <span aria-hidden="true" className="tw:mt-0.5 tw:grid tw:size-7 tw:shrink-0 tw:place-items-center tw:rounded-full tw:bg-accent tw:font-sans tw:text-label tw:font-semibold tw:text-accent-foreground">{index + 1}</span>
                  <div className="tw:min-w-0 tw:flex-1"><StudyMarkdown text={step} /></div>
                </li>
              ))}
            </ol>
          </section>
        ) : null}
        {content.callouts.map((callout, index) => (
          <GuideCallout key={`${index}-${callout.kind}-${callout.title}`} kind={callout.kind} title={callout.title} body={callout.body} uiText={uiText} />
        ))}
        <StudyGuideSourceFigures
          documentId={figureSource?.documentId ?? null}
          locator={figureSource?.locator ?? null}
          explanations={content.visualExplanations}
          sectionNumber={sectionNumber}
        />
        {content.workedExamples.map((example, index) => (
          <section key={`${index}-${example.title}`} aria-labelledby={id(`example-${index}`)}
            className={cn("tw:grid tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:font-sans", page ? "tw:mt-3 tw:px-6 tw:py-5 tw:compact:px-4 tw:compact:py-4" : "tw:p-4", uiText)}>
            <p className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">Worked example</p>
            <BlockHeading level={level} id={id(`example-${index}`)} className="tw:-mt-2 tw:text-title-small">
              <StudyMarkdown text={example.title} inline />
            </BlockHeading>
            <div className="tw:text-(--ostudy-color-text-secondary)"><StudyMarkdown text={example.prompt} /></div>
            {example.steps.length > 0 ? (
              <ol className="tw:m-0 tw:grid tw:gap-1 tw:pl-6">
                {example.steps.map((step, stepIndex) => <li key={`${stepIndex}-${step}`} className="tw:pl-1"><StudyMarkdown text={step} /></li>)}
              </ol>
            ) : null}
            <div className="tw:grid tw:gap-1 tw:rounded-md tw:bg-success-soft tw:px-4 tw:py-3">
              <strong className="tw:font-semibold">Answer</strong>
              <StudyMarkdown text={example.answer} />
            </div>
            {example.takeaway ? (
              <div className="tw:grid tw:gap-1 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-3 tw:text-(--ostudy-color-text-secondary)">
                <strong className="tw:font-semibold tw:text-foreground">Takeaway</strong>
                <StudyMarkdown text={example.takeaway} />
              </div>
            ) : null}
          </section>
        ))}
        {blockKeys.has("misconceptions") ? (
          <section aria-labelledby={id("misconceptions")} className={cn("tw:grid tw:gap-3", page && "tw:mt-3")}>
            <BlockHeading level={level} id={id("misconceptions")}>Common misconceptions</BlockHeading>
            <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-6 tw:marker:text-warning">
              {content.commonMisconceptions.map((value, index) => <li key={`${index}-${value}`} className="tw:pl-1"><StudyMarkdown text={value} /></li>)}
            </ul>
          </section>
        ) : null}
        {showTerms && content.importantTerms.length > 0 ? (
          <section aria-labelledby={id("terms")} className={cn("tw:grid tw:gap-3", page && "tw:mt-3")}>
            <BlockHeading level={level} id={id("terms")}>Important terms</BlockHeading>
            <GuideTermList terms={content.importantTerms} className={uiText} />
          </section>
        ) : null}
      </div>
      {blockKeys.has("check") ? (
        <CheckYourself checks={content.retrievalChecks} headingId={id("check")} level={level} page={page}
          progress={checkProgress} onProgressChange={onCheckProgressChange} />
      ) : null}
    </div>
  );
}

/** The short summary at the top of a page: the overview or a section in one minute. */
export function GuideMinuteBox({ id, page, level = 2, children }: { id: string; page: boolean; level?: Level; children: ReactNode }) {
  return (
    <section aria-labelledby={id}
      className={cn("tw:grid tw:gap-3 tw:rounded-lg tw:bg-accent tw:font-sans tw:text-foreground", page ? "tw:px-6 tw:pt-5 tw:pb-6 tw:text-body tw:compact:px-4 tw:compact:py-4" : "tw:p-4 tw:text-small")}>
      <BlockHeading level={level} id={id} className="tw:flex tw:items-center tw:gap-2 tw:text-label tw:uppercase tw:tracking-wide tw:text-accent-foreground">
        <Icon name="timer" size={16} />In one minute
      </BlockHeading>
      {children}
    </section>
  );
}

/** A compact definition list of terms, for the rail beside a section or below it. */
export function GuideTermList({ terms, className }: { terms: StudyGuideSectionContent["importantTerms"]; className?: string }) {
  return (
    <dl className={cn("tw:m-0 tw:grid tw:gap-3 tw:font-sans", className)}>
      {terms.map((term, index) => (
        <div key={`${index}-${term.term}`} className="tw:grid tw:min-w-0 tw:gap-0.5">
          <dt className="tw:font-semibold tw:text-foreground"><StudyMarkdown text={term.term} inline /></dt>
          <dd className="tw:m-0 tw:text-(--ostudy-color-text-secondary)"><StudyMarkdown text={term.explanation} /></dd>
        </div>
      ))}
    </dl>
  );
}

// About 6.5rem a column: three columns still fit a phone, and wider tables scroll sideways
// instead of squeezing their columns to a word each.
const tableMinimumWidths = ["", "", "tw:min-w-[14rem]", "tw:min-w-[20rem]", "tw:min-w-[27rem]"] as const;

export function GuideTable({ table, uiText }: { table: StudyGuideTable; uiText: string }) {
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-2 tw:font-sans">
      <div className="tw:max-w-full tw:overflow-x-auto tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-card">
        <table className={cn("tw:w-full tw:border-collapse tw:text-left", uiText, tableMinimumWidths[table.columns.length] ?? "tw:min-w-[34rem]")}>
          <caption className="tw:px-4 tw:pt-3 tw:pb-2.5 tw:text-left tw:font-semibold tw:text-foreground">
            <StudyMarkdown text={table.title} inline />
          </caption>
          <thead>
            <tr>
              {table.columns.map((column, index) => (
                <th key={`${index}-${column}`} scope="col"
                  className="tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-muted tw:px-4 tw:py-2.5 tw:align-bottom tw:font-semibold tw:text-(--ostudy-color-text-secondary)">
                  <StudyMarkdown text={column} inline />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {table.rows.map((row, rowIndex) => (
              <tr key={`${rowIndex}-${row.join("|")}`}>
                {row.map((cell, cellIndex) => (
                  <td key={`${cellIndex}-${cell}`}
                    className={cn("tw:border-0 tw:border-t tw:border-solid tw:border-border tw:px-4 tw:py-2.5 tw:align-top tw:[&_p]:m-0", cellIndex === 0 && "tw:font-medium")}>
                    <StudyMarkdown text={cell} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {table.caption ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground"><StudyMarkdown text={table.caption} inline /></p> : null}
    </div>
  );
}

const calloutTones: Record<StudyGuideCalloutKind, { label: string; icon: IconName; box: string; mark: string }> = {
  key_idea: { label: "Key idea", icon: "lightbulb", box: "tw:bg-accent", mark: "tw:text-accent-foreground" },
  tip: { label: "Tip", icon: "check", box: "tw:bg-success-soft", mark: "tw:text-success" },
  warning: { label: "Warning", icon: "warning", box: "tw:bg-warning-soft", mark: "tw:text-warning" },
  note: { label: "Note", icon: "information", box: "tw:border tw:border-solid tw:border-border tw:bg-card", mark: "tw:text-muted-foreground" },
};

export function GuideCallout({ kind, title, body, uiText }: { kind: StudyGuideCalloutKind; title: string; body: string; uiText: string }) {
  const tone = calloutTones[kind] ?? calloutTones.note;
  return (
    <div role="note" aria-label={tone.label} className={cn("tw:flex tw:min-w-0 tw:items-start tw:gap-3.5 tw:rounded-md tw:px-4.5 tw:py-4 tw:font-sans tw:compact:gap-3 tw:compact:px-4", tone.box, uiText)}>
      <span aria-hidden="true" className={cn("tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-full tw:bg-card", tone.mark)}>
        <Icon name={tone.icon} size={16} />
      </span>
      <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
        <p className={cn("tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide", tone.mark)}>{tone.label}</p>
        <strong className="tw:font-semibold tw:text-foreground"><StudyMarkdown text={title} inline /></strong>
        <div className="tw:text-(--ostudy-color-text-secondary)"><StudyMarkdown text={body} /></div>
      </div>
    </div>
  );
}

const checkResults = [{ value: "yes", label: "Yes" }, { value: "not_yet", label: "Not yet" }] as const;

function CheckYourself({ checks, headingId, level, page, progress, onProgressChange }: {
  checks: StudyGuideRetrievalCheck[];
  headingId: string;
  level: Level;
  page: boolean;
  progress?: CheckProgressMap;
  onProgressChange?: (next: CheckProgressMap) => void;
}) {
  const [ownProgress, setOwnProgress] = useState<CheckProgressMap>({});
  const current = progress ?? ownProgress;
  const answered = checks.filter((_, index) => current[index]?.revealed).length;
  // Show answer removes its own button, so focus moves to the answer it revealed.
  const focusAnswer = useRef<number | null>(null);
  const promptPrefix = `${headingId}-prompt`;
  function update(index: number, change: Partial<CheckProgress>): void {
    const next = { ...current, [index]: { revealed: false, result: null, ...current[index], ...change } };
    if (progress === undefined) setOwnProgress(next);
    onProgressChange?.(next);
  }
  return (
    <section aria-labelledby={headingId} className={cn("tw:grid tw:gap-3 tw:font-sans", page && "tw:mt-3")}>
      <div className="tw:flex tw:flex-wrap tw:items-baseline tw:justify-between tw:gap-x-3 tw:gap-y-1">
        <BlockHeading level={level} id={headingId}>Check yourself</BlockHeading>
        <p aria-live="polite" className="tw:m-0 tw:text-small tw:text-muted-foreground">{answered} of {checks.length} answered</p>
      </div>
      <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
        {checks.map((check, index) => {
          const state = current[index];
          const promptId = `${promptPrefix}-${index}`;
          return (
            <li key={`${index}-${check.prompt}`}
              className={cn("tw:grid tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card", page ? "tw:px-5 tw:py-4 tw:compact:px-4" : "tw:p-4")}>
              <p id={promptId} className={cn("tw:m-0 tw:font-semibold tw:leading-snug tw:text-foreground", page ? "tw:text-body-large tw:compact:text-body" : "tw:text-body")}>
                <StudyMarkdown text={check.prompt} inline />
              </p>
              {state?.revealed ? (
                <>
                  <div ref={(element) => {
                    if (!element || focusAnswer.current !== index) return;
                    focusAnswer.current = null;
                    element.focus();
                  }}
                    tabIndex={-1} className="tw:rounded-md tw:bg-success-soft tw:px-3.5 tw:py-3 tw:text-body tw:leading-relaxed tw:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50">
                    <VisuallyHidden>Answer: </VisuallyHidden>
                    <StudyMarkdown text={check.answer} />
                  </div>
                  <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-3 tw:gap-y-2">
                    {/* The control carries the same question as its name; this copy is for sighted readers. */}
                    <span aria-hidden="true" className="tw:text-small tw:text-muted-foreground">Did you get it?</span>
                    <SegmentedControl label="Did you get it?" value={state.result} options={checkResults}
                      onChange={(result) => update(index, { result })} />
                  </div>
                </>
              ) : (
                <Button variant="secondary" className="tw:justify-self-start" aria-describedby={promptId}
                  onClick={() => { focusAnswer.current = index; update(index, { revealed: true }); }}>
                  Show answer
                </Button>
              )}
            </li>
          );
        })}
      </ol>
    </section>
  );
}
