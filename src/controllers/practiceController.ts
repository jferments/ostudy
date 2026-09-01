import {
  completePracticeSession,
  getPracticeCardWindow,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  refreshPracticeAfterCardEdit,
  resumePracticeSession,
  startCardPractice,
  startSelectedPractice,
  submitPracticeReview,
  type PracticeCardWindow,
  type PracticeCardWindowInput,
  type PracticeEditRefresh,
  type PracticeSession,
  type RefreshPracticeAfterCardEditInput,
  type ResumedPracticeSession,
  type ReviewPreviewSet,
  type ReviewResult,
  type StartCardPracticeInput,
  type StartPracticeInput,
  type SubmitReviewInput,
} from "../ipc/practice";
import {
  applyScheduleAction,
  type ScheduleActionInput,
  type ScheduleActionResult,
} from "../ipc/scheduling";
import { latestResult, RequestLane } from "./requestLane";

export type PracticeStartRequest =
  | ({ kind: "selected" } & StartPracticeInput)
  | ({ kind: "cards" } & StartCardPracticeInput);

export class PracticeController {
  readonly #window = new RequestLane();

  window(input: PracticeCardWindowInput): Promise<PracticeCardWindow | undefined> {
    return latestResult(this.#window, () => getPracticeCardWindow(input));
  }

  refreshAfterEdit(input: RefreshPracticeAfterCardEditInput): Promise<PracticeEditRefresh> {
    return refreshPracticeAfterCardEdit(input);
  }

  start(request: PracticeStartRequest): Promise<PracticeSession> {
    if (request.kind === "cards") {
      const { kind: _kind, ...input } = request;
      return startCardPractice(input);
    }
    const { kind: _kind, ...input } = request;
    return startSelectedPractice(input);
  }

  resume(sessionId: string): Promise<ResumedPracticeSession> {
    return resumePracticeSession(sessionId);
  }

  markViewed(sessionId: string, cardId: string): Promise<void> {
    return markPracticeCardViewed(sessionId, cardId);
  }

  reveal(sessionId: string, cardId: string): Promise<ReviewPreviewSet> {
    return markPracticeCardRevealed(sessionId, cardId);
  }

  review(input: SubmitReviewInput): Promise<ReviewResult> {
    return submitPracticeReview(input);
  }

  schedule(input: ScheduleActionInput): Promise<ScheduleActionResult> {
    return applyScheduleAction(input);
  }

  complete(sessionId: string): Promise<void> {
    return completePracticeSession(sessionId);
  }

  invalidateWindow(): void {
    this.#window.invalidate();
  }

  invalidate(): void {
    this.#window.invalidate();
  }
}
