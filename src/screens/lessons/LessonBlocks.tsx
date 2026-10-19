import type { LessonBlock } from "../../ipc/lessons";
import { cn } from "../../ui/internal/cn";
import { GuideCallout, GuideTable } from "../study-guides/StudyGuideContent";

/** A step's teaching blocks in the card font, styled like a guide's. */
export function LessonBlocks({ blocks }: { blocks: readonly LessonBlock[] }) {
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-5 tw:break-words tw:font-(family-name:--card-font-family) tw:text-body-large tw:leading-(--ostudy-font-line-height-reading) tw:compact:text-body">
      {blocks.map((block, index) => (
        <Block key={`${index}-${block.kind}`} block={block} />
      ))}
    </div>
  );
}

const uiText = "tw:text-body";

function Block({ block }: { block: LessonBlock }) {
  switch (block.kind) {
    case "explanation":
      return (
        <div className="tw:grid tw:gap-2">
          {block.heading ? <h3 className="tw:m-0 tw:text-title-small tw:font-semibold">{block.heading}</h3> : null}
          <Paragraphs text={block.body} />
        </div>
      );
    case "keyPoints":
      return (
        <ul className="tw:m-0 tw:grid tw:list-disc tw:gap-2 tw:pl-6 tw:marker:text-primary">
          {block.items.map((item, index) => (
            <li key={`${index}-${item}`} className="tw:pl-1">
              {item}
            </li>
          ))}
        </ul>
      );
    case "term":
      return (
        <dl className="tw:m-0 tw:grid tw:gap-1 tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-card tw:px-4 tw:py-3 tw:font-sans tw:text-body">
          <dt className="tw:font-semibold">{block.term}</dt>
          <dd className="tw:m-0 tw:text-(--ostudy-color-text-secondary)">{block.explanation}</dd>
        </dl>
      );
    case "process":
      return (
        <div className="tw:grid tw:gap-3">
          <h3 className="tw:m-0 tw:text-title-small tw:font-semibold">{block.title}</h3>
          <ol className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
            {block.steps.map((step, index) => (
              <li key={`${index}-${step}`} className="tw:flex tw:min-w-0 tw:gap-3">
                <span
                  aria-hidden="true"
                  className="tw:mt-0.5 tw:grid tw:size-7 tw:shrink-0 tw:place-items-center tw:rounded-full tw:bg-accent tw:font-sans tw:text-label tw:font-semibold tw:text-accent-foreground"
                >
                  {index + 1}
                </span>
                <span className="tw:min-w-0 tw:flex-1">{step}</span>
              </li>
            ))}
          </ol>
        </div>
      );
    case "comparison":
      return (
        <div className="tw:grid tw:gap-1 tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-card tw:px-4 tw:py-3 tw:font-sans tw:text-body">
          <strong className="tw:font-semibold">
            {block.subjectA} and {block.subjectB}
          </strong>
          <span className="tw:text-(--ostudy-color-text-secondary)">{block.relationship}</span>
        </div>
      );
    case "callout":
      return <GuideCallout kind={block.callout} title={block.title} body={block.body} uiText={uiText} />;
    case "table":
      return (
        <GuideTable
          table={{ title: block.title, columns: block.columns, rows: block.rows, caption: block.caption ?? "" }}
          uiText={uiText}
        />
      );
    case "workedExample":
      return (
        <div className="tw:grid tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:font-sans tw:text-body">
          <p className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
            Worked example
          </p>
          <h3 className="tw:-mt-2 tw:mb-0 tw:text-title-small tw:font-semibold">{block.title}</h3>
          <p className="tw:m-0 tw:text-(--ostudy-color-text-secondary)">{block.prompt}</p>
          {block.steps.length > 0 ? (
            <ol className="tw:m-0 tw:grid tw:gap-1 tw:pl-6">
              {block.steps.map((step, index) => (
                <li key={`${index}-${step}`} className="tw:pl-1">
                  {step}
                </li>
              ))}
            </ol>
          ) : null}
          <div className="tw:grid tw:gap-1 tw:rounded-md tw:bg-success-soft tw:px-4 tw:py-3">
            <strong className="tw:font-semibold">Answer</strong>
            <span>{block.answer}</span>
          </div>
          {block.takeaway ? (
            <p className="tw:m-0 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-3 tw:text-(--ostudy-color-text-secondary)">
              {block.takeaway}
            </p>
          ) : null}
        </div>
      );
    case "misconception":
      return (
        <div
          role="note"
          aria-label="Misconception"
          className={cn("tw:grid tw:gap-1 tw:rounded-md tw:bg-warning-soft tw:px-4 tw:py-3 tw:font-sans", uiText)}
        >
          <p className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-warning">
            Common mistake
          </p>
          <span className="tw:text-(--ostudy-color-text-secondary)">{block.misconception}</span>
          <span className="tw:font-medium">{block.correction}</span>
        </div>
      );
  }
}

function Paragraphs({ text }: { text: string }) {
  return (
    <>
      {text
        .split(/\n{2,}/u)
        .filter((paragraph) => paragraph.trim())
        .map((paragraph, index) => (
          <p key={index} className="tw:m-0 tw:whitespace-pre-wrap">
            {paragraph.trim()}
          </p>
        ))}
    </>
  );
}
