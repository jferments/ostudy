import { type CSSProperties, type ReactNode, useId, useRef, useState } from "react";

import { StudyMarkdown } from "../../components/StudyMarkdown";
import type {
  SourceLocator,
  StudyGuideCalloutKind,
  StudyGuidePdfTheme,
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
   * Sets the reading blocks as a page of paper in this guide theme, with colored boxes; the
   * check questions follow the page. Without it the blocks sit in the app's own colors.
   */
  paper?: StudyGuidePdfTheme | null;
}

type Level = 2 | 3 | 4 | 5;
const headingElements = { 2: "h2", 3: "h3", 4: "h4", 5: "h5" } as const;
const headingSizes: Record<Level, string> = {
  2: "tw:text-section",
  3: "tw:text-body-large",
  4: "tw:text-body",
  5: "tw:text-body",
};

/** The classes a section's blocks take: in the app's colors, or on a page of guide paper. */
interface Palette {
  /** Secondary reading text. */
  secondary: string;
  heading: string;
  minute: string;
  minuteMark: string;
  surface: string;
  border: string;
  tableHead: string;
  /** Key idea tiles, taken in turn so neighbouring ideas differ in color. */
  tiles: ReadonlyArray<{ box: string; mark: string }>;
  /** Explanation boxes, taken in turn. */
  explanations: readonly string[];
  stepMark: string;
  misconceptions: string;
  answer: string;
  callouts: Record<StudyGuideCalloutKind, { box: string; mark: string }>;
}

const appPalette: Palette = {
  secondary: "tw:text-(--ostudy-color-text-secondary)",
  heading: "tw:text-foreground",
  minute: "tw:bg-accent tw:text-foreground",
  minuteMark: "tw:text-accent-foreground",
  surface: "tw:bg-card",
  border: "tw:border-border",
  tableHead: "tw:bg-muted tw:text-(--ostudy-color-text-secondary)",
  tiles: [
    { box: "tw:bg-subject-1-soft", mark: "tw:bg-subject-1 tw:text-primary-foreground" },
    { box: "tw:bg-subject-2-soft", mark: "tw:bg-subject-2 tw:text-primary-foreground" },
    { box: "tw:bg-subject-3-soft", mark: "tw:bg-subject-3 tw:text-primary-foreground" },
  ],
  explanations: ["tw:border-primary tw:bg-foreground/3", "tw:border-accent-secondary tw:bg-foreground/3"],
  stepMark: "tw:bg-accent tw:text-accent-foreground",
  misconceptions: "tw:bg-warning-soft",
  answer: "tw:bg-success-soft",
  callouts: {
    key_idea: { box: "tw:bg-accent", mark: "tw:text-accent-foreground" },
    tip: { box: "tw:bg-success-soft", mark: "tw:text-success" },
    warning: { box: "tw:bg-warning-soft", mark: "tw:text-warning" },
    note: { box: "tw:border tw:border-solid tw:border-border tw:bg-card", mark: "tw:text-muted-foreground" },
  },
};

const paperPalette: Palette = {
  secondary: "tw:text-(--guide-ink)",
  heading: "tw:text-(--guide-primary)",
  minute: "tw:bg-(--guide-primary) tw:text-(--guide-paper)",
  minuteMark: "tw:text-(--guide-band)",
  surface: "tw:bg-(--guide-paper)",
  border: "tw:border-(--guide-rule)",
  tableHead: "tw:bg-(--guide-primary) tw:text-(--guide-paper)",
  tiles: [
    { box: "tw:bg-(--guide-accent-soft)", mark: "tw:bg-(--guide-accent) tw:text-(--guide-paper)" },
    { box: "tw:bg-(--guide-highlight-soft)", mark: "tw:bg-(--guide-highlight-ink) tw:text-(--guide-paper)" },
    { box: "tw:bg-(--guide-good-soft)", mark: "tw:bg-(--guide-good) tw:text-(--guide-paper)" },
  ],
  explanations: ["tw:border-(--guide-accent) tw:bg-(--guide-accent-soft)/50", "tw:border-(--guide-highlight) tw:bg-(--guide-highlight-soft)/60"],
  stepMark: "tw:bg-(--guide-accent) tw:text-(--guide-paper)",
  misconceptions: "tw:bg-(--guide-warn-soft)",
  answer: "tw:bg-(--guide-good-soft)",
  callouts: {
    key_idea: { box: "tw:bg-(--guide-accent-soft)", mark: "tw:text-(--guide-accent)" },
    tip: { box: "tw:bg-(--guide-good-soft)", mark: "tw:text-(--guide-good)" },
    warning: { box: "tw:bg-(--guide-warn-soft)", mark: "tw:text-(--guide-warn)" },
    note: { box: "tw:border tw:border-solid tw:border-(--guide-rule) tw:bg-(--guide-paper)", mark: "tw:text-(--guide-muted)" },
  },
};

/** The guide themes' paper colors, the same as the exported PDF's (typst/themes.typ). */
const paperThemes: Record<StudyGuidePdfTheme, Record<string, string>> = {
  navy: { primary: "#1d2b4f", accent: "#2f6690", "accent-soft": "#e8f0f6", band: "#8fb3d1", highlight: "#e8b04a",
    "highlight-soft": "#fbf3e1", "highlight-ink": "#8a5d0f", warn: "#b8513c", "warn-soft": "#f9ece8", good: "#2e6e52",
    "good-soft": "#eaf3ee", ink: "#1c2130", muted: "#646b7a", rule: "#d5dae2", paper: "#ffffff" },
  sage: { primary: "#4a6b57", accent: "#4a6b57", "accent-soft": "#f1eadb", band: "#d6e2d6", highlight: "#b0614a",
    "highlight-soft": "#f5e6dd", "highlight-ink": "#8e4633", warn: "#a8432f", "warn-soft": "#f6e1d8", good: "#3f6a52",
    "good-soft": "#e5ece3", ink: "#2d2a26", muted: "#716b60", rule: "#d9d0bd", paper: "#fbf7ef" },
  slate: { primary: "#263241", accent: "#1f7a7a", "accent-soft": "#e3f1f0", band: "#9fd3cf", highlight: "#e07a5f",
    "highlight-soft": "#fcece6", "highlight-ink": "#a4472f", warn: "#b23a48", "warn-soft": "#f8e5e8", good: "#2f7d4f",
    "good-soft": "#e6f2ea", ink: "#1d232c", muted: "#5e6875", rule: "#d3d9df", paper: "#ffffff" },
  plum: { primary: "#3f2a4a", accent: "#7b4b8a", "accent-soft": "#f2ebf5", band: "#cdb6d6", highlight: "#d4a03c",
    "highlight-soft": "#fbf3e2", "highlight-ink": "#87600f", warn: "#b6455a", "warn-soft": "#f9e6ea", good: "#3d7a6a",
    "good-soft": "#e5f1ee", ink: "#241c29", muted: "#6b6372", rule: "#ddd4e2", paper: "#fffdfb" },
};

/**
 * A page of guide paper: the theme's paper and ink whatever the app's theme, as notebook pages
 * are, under the theme's colored band.
 */
export function GuidePaper({ theme, className, children }: { theme: StudyGuidePdfTheme; className?: string; children: ReactNode }) {
  const style = Object.fromEntries(Object.entries(paperThemes[theme]).map(([name, value]) => [`--guide-${name}`, value])) as CSSProperties;
  return (
    <div data-guide-paper={theme} style={style}
      className={cn("tw:relative tw:min-w-0 tw:overflow-hidden tw:rounded-lg tw:bg-(--guide-paper) tw:text-(--guide-ink) tw:shadow-md tw:ring-1 tw:ring-(--guide-rule)",
        "tw:px-10 tw:pt-11 tw:pb-10 tw:compact:px-5 tw:compact:pt-8 tw:compact:pb-6", className)}>
      <div aria-hidden="true" className="tw:absolute tw:inset-x-0 tw:top-0 tw:h-2 tw:bg-(--guide-primary)" />
      <div aria-hidden="true" className="tw:absolute tw:top-2 tw:left-0 tw:h-1 tw:w-1/4 tw:bg-(--guide-highlight)" />
      {children}
    </div>
  );
}

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
 * key ideas, explanations, tables, comparisons, process, callouts, figures, worked examples,
 * misconceptions and the retrieval checks. Document and collection guides share it.
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
  paper = null,
}: StudyGuideStructuredContentProps) {
  const generatedId = `guide-block-${useId().replaceAll(":", "")}`;
  const prefix = idPrefix ?? generatedId;
  const page = headingLevel === 2;
  const level = headingLevel as Level;
  const blockKeys = new Set(sectionBlocks(content).map((block) => block.key));
  const id = (key: string) => `${prefix}-${key}`;
  const palette = paper ? paperPalette : appPalette;
  // Interface-font blocks sit a step below the reading text, as in the mockups.
  const uiText = page ? "tw:text-body" : "tw:text-small";
  // On a page the key points are tiles of their own; elsewhere they stay in the summary's list.
  const tiles = page && content.keyPoints.length > 0;
  const blocks = <>
    {blockKeys.has("minute") ? (
      <GuideMinuteBox id={id("minute")} page={page} level={level} palette={palette}>
        {content.overview.trim() ? <StudyMarkdown text={content.overview} /> : null}
        {!tiles && content.keyPoints.length > 0 ? (
          <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-5 tw:marker:text-primary">
            {content.keyPoints.map((point, index) => <li key={`${index}-${point}`} className="tw:pl-1"><StudyMarkdown text={point} /></li>)}
          </ul>
        ) : null}
      </GuideMinuteBox>
    ) : null}
    {tiles ? (
      <section aria-labelledby={id("key-ideas")} className="tw:grid tw:gap-3">
        <BlockHeading level={level} id={id("key-ideas")} className={palette.heading}>Key ideas</BlockHeading>
        <ol className={cn("tw:m-0 tw:grid tw:list-none tw:gap-2.5 tw:p-0 tw:font-sans", uiText,
          content.keyPoints.length > 3 && "tw:wide:grid-cols-2")}>
          {content.keyPoints.map((point, index) => {
            const tone = palette.tiles[index % palette.tiles.length] ?? palette.tiles[0];
            return (
              <li key={`${index}-${point}`} className={cn("tw:flex tw:min-w-0 tw:items-start tw:gap-3 tw:rounded-md tw:px-4 tw:py-3", tone?.box)}>
                <span aria-hidden="true" className={cn("tw:grid tw:size-6 tw:shrink-0 tw:place-items-center tw:rounded-full tw:text-caption tw:font-bold", tone?.mark)}>
                  {index + 1}
                </span>
                <div className="tw:min-w-0 tw:flex-1"><StudyMarkdown text={point} /></div>
              </li>
            );
          })}
        </ol>
      </section>
    ) : null}
    {content.explanations.map((explanation, index) => (
      <section key={`${index}-${explanation.heading}`} aria-labelledby={id(`explanation-${index}`)}
        className={cn("tw:grid tw:gap-2", page && cn("tw:rounded-r-md tw:border-0 tw:border-l-4 tw:border-solid tw:px-5 tw:py-4 tw:compact:px-4",
          palette.explanations[index % palette.explanations.length]))}>
        <BlockHeading level={level} id={id(`explanation-${index}`)} className={palette.heading}><StudyMarkdown text={explanation.heading} inline /></BlockHeading>
        <div className="tw:[&_p]:my-2 tw:[&_p:first-child]:mt-0 tw:[&_p:last-child]:mb-0"><StudyMarkdown text={explanation.body} /></div>
      </section>
    ))}
    {content.tables.map((table, index) => <GuideTable key={`${index}-${table.title}`} table={table} uiText={uiText} palette={palette} />)}
    {blockKeys.has("comparisons") ? (
      <section aria-labelledby={id("comparisons")} className={cn("tw:grid tw:gap-3", page && "tw:mt-1")}>
        <BlockHeading level={level} id={id("comparisons")} className={palette.heading}>Comparisons</BlockHeading>
        <ul className={cn("tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0 tw:font-sans", uiText)}>
          {content.comparisons.map((comparison, index) => (
            <li key={`${index}-${comparison.subjectA}-${comparison.subjectB}`}
              className={cn("tw:grid tw:gap-1 tw:rounded-md tw:border tw:border-solid tw:px-4 tw:py-3", palette.border, palette.surface)}>
              <strong className={cn("tw:font-semibold", palette.heading)}>
                <StudyMarkdown text={comparison.subjectA} inline /> and <StudyMarkdown text={comparison.subjectB} inline />
              </strong>
              <div className={palette.secondary}><StudyMarkdown text={comparison.relationship} /></div>
            </li>
          ))}
        </ul>
      </section>
    ) : null}
    {blockKeys.has("process") ? (
      <section aria-labelledby={id("process")} className={cn("tw:grid tw:gap-3", page && "tw:mt-1")}>
        <BlockHeading level={level} id={id("process")} className={palette.heading}>Process</BlockHeading>
        <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
          {content.processSteps.map((step, index) => (
            <li key={`${index}-${step}`} className="tw:flex tw:min-w-0 tw:gap-3">
              <span aria-hidden="true" className={cn("tw:mt-0.5 tw:grid tw:size-7 tw:shrink-0 tw:place-items-center tw:rounded-full tw:font-sans tw:text-label tw:font-semibold", palette.stepMark)}>{index + 1}</span>
              <div className="tw:min-w-0 tw:flex-1"><StudyMarkdown text={step} /></div>
            </li>
          ))}
        </ol>
      </section>
    ) : null}
    {content.callouts.map((callout, index) => (
      <GuideCallout key={`${index}-${callout.kind}-${callout.title}`} kind={callout.kind} title={callout.title} body={callout.body} uiText={uiText} palette={palette} />
    ))}
    <StudyGuideSourceFigures
      documentId={figureSource?.documentId ?? null}
      locator={figureSource?.locator ?? null}
      explanations={content.visualExplanations}
      sectionNumber={sectionNumber}
    />
    {content.workedExamples.map((example, index) => (
      <section key={`${index}-${example.title}`} aria-labelledby={id(`example-${index}`)}
        className={cn("tw:grid tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:font-sans", palette.border, palette.surface,
          page ? "tw:mt-1 tw:px-6 tw:py-5 tw:compact:px-4 tw:compact:py-4" : "tw:p-4", uiText)}>
        <p className={cn("tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide", paper ? "tw:text-(--guide-good)" : "tw:text-muted-foreground")}>Worked example</p>
        <BlockHeading level={level} id={id(`example-${index}`)} className={cn("tw:-mt-2 tw:text-title-small", palette.heading)}>
          <StudyMarkdown text={example.title} inline />
        </BlockHeading>
        <div className={palette.secondary}><StudyMarkdown text={example.prompt} /></div>
        {example.steps.length > 0 ? (
          <ol className="tw:m-0 tw:grid tw:gap-1 tw:pl-6">
            {example.steps.map((step, stepIndex) => <li key={`${stepIndex}-${step}`} className="tw:pl-1"><StudyMarkdown text={step} /></li>)}
          </ol>
        ) : null}
        <div className={cn("tw:grid tw:gap-1 tw:rounded-md tw:px-4 tw:py-3", palette.answer)}>
          <strong className="tw:font-semibold">Answer</strong>
          <StudyMarkdown text={example.answer} />
        </div>
        {example.takeaway ? (
          <div className={cn("tw:grid tw:gap-1 tw:border-0 tw:border-t tw:border-solid tw:pt-3", palette.border, palette.secondary)}>
            <strong className={cn("tw:font-semibold", palette.heading)}>Takeaway</strong>
            <StudyMarkdown text={example.takeaway} />
          </div>
        ) : null}
      </section>
    ))}
    {blockKeys.has("misconceptions") ? (
      <section aria-labelledby={id("misconceptions")}
        className={cn("tw:grid tw:gap-3", page && cn("tw:mt-1 tw:rounded-md tw:px-5 tw:py-4 tw:compact:px-4", palette.misconceptions))}>
        <BlockHeading level={level} id={id("misconceptions")} className={palette.heading}>Common misconceptions</BlockHeading>
        <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-6 tw:marker:text-warning">
          {content.commonMisconceptions.map((value, index) => <li key={`${index}-${value}`} className="tw:pl-1"><StudyMarkdown text={value} /></li>)}
        </ul>
      </section>
    ) : null}
    {showTerms && content.importantTerms.length > 0 ? (
      <section aria-labelledby={id("terms")} className={cn("tw:grid tw:gap-3", page && "tw:mt-1")}>
        <BlockHeading level={level} id={id("terms")} className={palette.heading}>Important terms</BlockHeading>
        <GuideTermList terms={content.importantTerms} className={uiText} />
      </section>
    ) : null}
  </>;
  const reading = "tw:flex tw:min-w-0 tw:flex-col tw:break-words tw:font-(family-name:--card-font-family) tw:leading-(--ostudy-font-line-height-reading)";
  const size = page ? "tw:gap-6 tw:text-body-large tw:compact:gap-5 tw:compact:text-body" : "tw:gap-5 tw:text-body";
  const check = blockKeys.has("check") ? (
    <CheckYourself checks={content.retrievalChecks} headingId={id("check")} level={level} page={page}
      progress={checkProgress} onProgressChange={onCheckProgressChange} />
  ) : null;
  if (paper) {
    return (
      <div className={cn(reading, size)}>
        <GuidePaper theme={paper}><div className={cn(reading, page ? "tw:gap-5 tw:compact:gap-4" : "tw:gap-4")}>{blocks}</div></GuidePaper>
        {check}
      </div>
    );
  }
  return <div className={cn(reading, size)}>{blocks}{check}</div>;
}

/** The short summary at the top of a page: the overview or a section in one minute. */
export function GuideMinuteBox({ id, page, level = 2, palette: given, paper = false, children }: {
  id: string;
  page: boolean;
  level?: Level;
  palette?: Palette;
  /** Sits on guide paper, in the paper's colors. */
  paper?: boolean;
  children: ReactNode;
}) {
  const palette = given ?? (paper ? paperPalette : appPalette);
  return (
    <section aria-labelledby={id}
      className={cn("tw:grid tw:gap-3 tw:rounded-lg tw:font-sans", palette.minute, page ? "tw:px-6 tw:pt-5 tw:pb-6 tw:text-body tw:compact:px-4 tw:compact:py-4" : "tw:p-4 tw:text-small")}>
      <BlockHeading level={level} id={id} className={cn("tw:flex tw:items-center tw:gap-2 tw:text-label tw:uppercase tw:tracking-wide", palette.minuteMark)}>
        <Icon name="timer" size={16} />In one minute
      </BlockHeading>
      {children}
    </section>
  );
}

/** A compact definition list of terms, for the rail beside a section or below it. */
/**
 * A section's terms and what each means. With `hideMeanings` each meaning waits behind a
 * Show button, so the learner can say it first; turning it off shows them all again.
 */
export function GuideTermList({ terms, className, hideMeanings = false }: {
  terms: StudyGuideSectionContent["importantTerms"];
  className?: string;
  hideMeanings?: boolean;
}) {
  const [shown, setShown] = useState<ReadonlySet<number>>(new Set());
  // Hiding again, or another section's terms, starts every meaning hidden.
  const [hiddenFor, setHiddenFor] = useState({ terms, hideMeanings });
  if (hiddenFor.terms !== terms || hiddenFor.hideMeanings !== hideMeanings) {
    setHiddenFor({ terms, hideMeanings });
    setShown(new Set());
  }
  return (
    <dl className={cn("tw:m-0 tw:grid tw:gap-3 tw:font-sans", className)}>
      {terms.map((term, index) => (
        <div key={`${index}-${term.term}`} className="tw:grid tw:min-w-0 tw:gap-0.5">
          <dt className="tw:font-semibold tw:text-foreground"><StudyMarkdown text={term.term} inline /></dt>
          <dd className="tw:m-0 tw:text-(--ostudy-color-text-secondary)">
            {hideMeanings && !shown.has(index) ? (
              <Button size="small" variant="text" className="tw:-ms-2"
                aria-label={`Show what ${term.term} means`}
                onClick={() => setShown((current) => new Set(current).add(index))}>
                Show
              </Button>
            ) : <StudyMarkdown text={term.explanation} />}
          </dd>
        </div>
      ))}
    </dl>
  );
}

// About 6.5rem a column: three columns still fit a phone, and wider tables scroll sideways
// instead of squeezing their columns to a word each.
const tableMinimumWidths = ["", "", "tw:min-w-[14rem]", "tw:min-w-[18rem]", "tw:min-w-[27rem]"] as const;

export function GuideTable({ table, uiText, palette = appPalette }: { table: StudyGuideTable; uiText: string; palette?: Palette }) {
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-2 tw:font-sans">
      <div className={cn("tw:max-w-full tw:overflow-x-auto tw:rounded-md tw:border tw:border-solid", palette.border, palette.surface)}>
        <table className={cn("tw:w-full tw:border-collapse tw:text-left", uiText, tableMinimumWidths[table.columns.length] ?? "tw:min-w-[34rem]")}>
          <caption className={cn("tw:px-4 tw:pt-3 tw:pb-2.5 tw:text-left tw:font-semibold", palette.heading)}>
            <StudyMarkdown text={table.title} inline />
          </caption>
          <thead>
            <tr>
              {table.columns.map((column, index) => (
                <th key={`${index}-${column}`} scope="col"
                  className={cn("tw:border-0 tw:border-t tw:border-solid tw:px-4 tw:py-2.5 tw:align-bottom tw:font-semibold tw:compact:px-3", palette.border, palette.tableHead)}>
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
                    className={cn("tw:border-0 tw:border-t tw:border-solid tw:px-4 tw:py-2.5 tw:align-top tw:compact:px-3 tw:[&_p]:m-0", palette.border, cellIndex === 0 && "tw:font-medium")}>
                    <StudyMarkdown text={cell} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {table.caption ? <p className={cn("tw:m-0 tw:text-small", palette === appPalette ? "tw:text-muted-foreground" : "tw:text-(--guide-muted)")}><StudyMarkdown text={table.caption} inline /></p> : null}
    </div>
  );
}

const calloutTones: Record<StudyGuideCalloutKind, { label: string; icon: IconName }> = {
  key_idea: { label: "Key idea", icon: "lightbulb" },
  tip: { label: "Tip", icon: "check" },
  warning: { label: "Warning", icon: "warning" },
  note: { label: "Note", icon: "information" },
};

export function GuideCallout({ kind, title, body, uiText, palette = appPalette }: {
  kind: StudyGuideCalloutKind;
  title: string;
  body: string;
  uiText: string;
  palette?: Palette;
}) {
  const known = kind in calloutTones ? kind : "note";
  const tone = calloutTones[known];
  const colors = palette.callouts[known];
  return (
    <div role="note" aria-label={tone.label} className={cn("tw:flex tw:min-w-0 tw:items-start tw:gap-3.5 tw:rounded-md tw:px-4.5 tw:py-4 tw:font-sans tw:compact:gap-3 tw:compact:px-4", colors.box, uiText)}>
      <span aria-hidden="true" className={cn("tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-full", palette.surface, colors.mark)}>
        <Icon name={tone.icon} size={16} />
      </span>
      <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
        <p className={cn("tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide", colors.mark)}>{tone.label}</p>
        <strong className={cn("tw:font-semibold", palette.heading)}><StudyMarkdown text={title} inline /></strong>
        <div className={palette.secondary}><StudyMarkdown text={body} /></div>
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
