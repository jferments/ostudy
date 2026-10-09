import { useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import type { QuestionTypeAnalytics, RecentReview, SessionAnalytics } from "../../ipc/analytics";
import { getErrorMessage } from "../../ipc/common";
import { getReviewLog, type ProgressQuery, type ReviewLogPage } from "../../ipc/progress";
import { cn } from "../../ui/internal/cn";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ListRow, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { QuestionTypeBreakdown, questionTypeLabel } from "./QuestionTypeBreakdown";
import { formatDateTime, formatPercent, formatResponseTime, formatStudyTime, plural } from "./progressFormat";

export function QuestionTypesPanel({ rows, loading }: { rows: QuestionTypeAnalytics[] | null; loading: boolean }) {
  const [details, setDetails] = useState(false);
  const reviewed = (rows ?? []).filter((row) => row.reviewCount > 0);
  if (rows !== null && reviewed.length === 0) return null;
  return (
    <Panel headingLevel={2} title="Question types" description="Remembered on the first try" aria-busy={loading || undefined}
      actions={reviewed.length > 0 ? <Button variant="text" onClick={() => setDetails(true)}>Details</Button> : undefined}>
      <ul className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
        {reviewed.map((row) => (
          <li key={row.questionType} className="tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:items-end tw:gap-x-3 tw:gap-y-1">
            <span className="tw:flex tw:min-w-0 tw:justify-between tw:gap-2 tw:text-small">
              <span className="tw:truncate">{questionTypeLabel(row.questionType)}</span>
              <span className="tw:text-muted-foreground tw:tabular-nums">{row.reviewCount.toLocaleString()}</span>
            </span>
            <strong className="tw:row-span-2 tw:w-10 tw:text-right tw:tabular-nums">{formatPercent(row.rememberedRate)}</strong>
            <span aria-hidden="true" className="tw:h-1.5 tw:overflow-hidden tw:rounded-full tw:bg-progress-track">
              <span className="tw:block tw:h-full tw:rounded-full tw:bg-chart-1" style={{ width: `${(row.rememberedRate ?? 0) * 100}%` }} />
            </span>
          </li>
        ))}
      </ul>
      {details && rows ? (
        <Sheet title="Question types" description="First checks and self-ratings are separate. Each board counts as one review." width="large" onClose={() => setDetails(false)}>
          <QuestionTypeBreakdown rows={rows} />
        </Sheet>
      ) : null}
    </Panel>
  );
}

const SESSIONS_SHOWN = 4;

export function SessionsPanel({ sessions, highlightedSessionId, loading }: { sessions: SessionAnalytics[] | null; highlightedSessionId?: string; loading: boolean }) {
  const [showAll, setShowAll] = useState(false);
  const list = sessions ?? [];
  const shown = showAll ? list : list.slice(0, SESSIONS_SHOWN);
  return (
    <Panel headingLevel={2} title="Recent sessions" aria-busy={loading || undefined}
      actions={list.length > SESSIONS_SHOWN ? <Button variant="text" aria-expanded={showAll} onClick={() => setShowAll((value) => !value)}>{showAll ? "Show fewer" : "All sessions"}</Button> : undefined}>
      {sessions !== null && list.length === 0 ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Finish a study session to see it here.</p> : (
        <ListRows>
          {shown.map((session) => (
            <ListRow key={session.id} density="dense" selected={session.id === highlightedSessionId}
              title={session.deckNames.length === 0 ? "Custom selection" : session.deckNames.length > 2 ? "Mixed review" : session.deckNames.join(", ")}
              meta={`${formatDateTime(session.startedAt)} · ${plural(session.ratedCount, "card")} · ${formatStudyTime(session.studyTimeMs)}`}
              trailing={<strong className="tw:tabular-nums">{formatPercent(session.correctRate)}</strong>} />
          ))}
        </ListRows>
      )}
    </Panel>
  );
}

const outcomeLabels: Record<RecentReview["outcome"], string> = {
  wrong: "Wrong",
  not_confident: "Hard",
  somewhat_confident: "Medium",
  very_confident: "Easy",
};
const outcomeColors: Record<RecentReview["outcome"], string> = {
  wrong: "tw:bg-(--ostudy-color-confidence-0)",
  not_confident: "tw:bg-(--ostudy-color-confidence-1)",
  somewhat_confident: "tw:bg-(--ostudy-color-confidence-2)",
  very_confident: "tw:bg-(--ostudy-color-confidence-3)",
};

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}

function AnswerRows({ reviews, onSearchTag }: { reviews: RecentReview[]; onSearchTag: (tagId: string) => void }) {
  return (
    <ListRows>
      {reviews.map((review) => (
        <ListRow key={review.id} density="dense" className="tw:items-start"
          leading={<span aria-hidden="true" className={cn("tw:mt-1.5 tw:size-2.5 tw:shrink-0 tw:rounded-full", outcomeColors[review.outcome])} />}
          title={summarize(review.questionExcerpt)}
          meta={<>
            <span>{review.deckName} · {outcomeLabels[review.outcome]} · {formatResponseTime(review.responseTimeMs)} · {formatDateTime(review.reviewedAt)}</span>
            {review.tags.length > 0 ? (
              <span className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-2 tw:text-caption">
                {review.tags.map((tag) => tag.isCurrent ? (
                  <Button key={tag.tagId} variant="text" className="tw:h-auto tw:min-h-0 tw:p-0 tw:text-caption tw:font-normal tw:underline tw:underline-offset-4"
                    aria-label={`Search cards tagged ${tag.tagName}`} onClick={() => onSearchTag(tag.tagId)}>{tag.tagName}</Button>
                ) : <span key={tag.tagId}>{tag.tagName}</span>)}
              </span>
            ) : null}
          </>} />
      ))}
    </ListRows>
  );
}

const ANSWERS_SHOWN = 3;

interface AnswersPanelProps {
  query: ProgressQuery;
  firstPage: ReviewLogPage | null;
  loading: boolean;
  onSearchTag: (tagId: string) => void;
}

/** The latest answers in scope, and the whole review log a page at a time. */
export function AnswersPanel({ query, firstPage, loading, onSearchTag }: AnswersPanelProps) {
  const [open, setOpen] = useState(false);
  const reviews = firstPage?.reviews ?? [];
  return (
    <Panel headingLevel={2} title="Every answer" description="Each answer with its rating, time taken and date, newest first." aria-busy={loading || undefined}
      actions={reviews.length > 0 ? <Button variant="secondary" size="small" onClick={() => setOpen(true)}>Open review log</Button> : undefined}>
      {firstPage !== null && reviews.length === 0 ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Answers you give while studying show up here.</p>
        : <AnswerRows reviews={reviews.slice(0, ANSWERS_SHOWN)} onSearchTag={onSearchTag} />}
      {open && firstPage ? <ReviewLogSheet query={query} firstPage={firstPage} onSearchTag={(tagId) => { setOpen(false); onSearchTag(tagId); }} onClose={() => setOpen(false)} /> : null}
    </Panel>
  );
}

function ReviewLogSheet({ query, firstPage, onSearchTag, onClose }: { query: ProgressQuery; firstPage: ReviewLogPage; onSearchTag: (tagId: string) => void; onClose: () => void }) {
  const lane = useRequestLane();
  const [pages, setPages] = useState<ReviewLogPage[]>([firstPage]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const next = pages.at(-1)?.next ?? null;
  async function loadMore(): Promise<void> {
    if (!next) return;
    setLoading(true);
    setError(null);
    try {
      const page = await latestResult(lane, () => getReviewLog(query, next));
      if (page) setPages((current) => [...current, page]);
    } catch (reason: unknown) {
      setError(getErrorMessage(reason));
    } finally {
      setLoading(false);
    }
  }
  const reviews = pages.flatMap((page) => page.reviews);
  return (
    <Sheet title="Review log" description={`${plural(reviews.length, "answer")} shown, newest first`} width="large" onClose={onClose}
      actions={next ? <Button variant="secondary" loading={loading} onClick={() => void loadMore()}>Show older answers</Button> : undefined}>
      <AnswerRows reviews={reviews} onSearchTag={onSearchTag} />
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    </Sheet>
  );
}
