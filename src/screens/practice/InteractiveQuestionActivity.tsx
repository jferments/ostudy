import type { ComponentProps } from "react";

import { isImageOcclusionAttempt, type PracticeAttemptView, type ImageOcclusionAttemptView, type RevealPracticeQuestionInput } from "../../ipc/questionAttempts";
import { ImageOcclusionActivity } from "./ImageOcclusionActivity";
import { InteractiveQuestionActivity as ObjectiveQuestionActivity } from "./ObjectiveQuestionActivity";

type ObjectiveProps = ComponentProps<typeof ObjectiveQuestionActivity>;
type InteractiveQuestionActivityProps = Omit<ObjectiveProps, "attempt" | "onAttempt" | "onChecked"> & {
  attempt: PracticeAttemptView;
  onReveal?: (input: RevealPracticeQuestionInput) => Promise<ImageOcclusionAttemptView>;
  onAttempt: (attempt: PracticeAttemptView) => void;
  onChecked: (attempt: PracticeAttemptView) => void | Promise<void>;
};

/** A self-assessed image has its own reveal interaction, not an empty objective
 * answer. Both adapters return to the same captured-session/rating host.
 */
export function InteractiveQuestionActivity(props: InteractiveQuestionActivityProps) {
  if (isImageOcclusionAttempt(props.attempt)) {
    return <ImageOcclusionActivity sessionId={props.sessionId} cardId={props.cardId}
      attempt={props.attempt} assets={props.assets} disabled={props.disabled}
      onReveal={props.onReveal ?? unavailableReveal} onAttempt={props.onAttempt} onRevealed={props.onChecked} onError={props.onError} />;
  }
  return <ObjectiveQuestionActivity {...props} attempt={props.attempt} />;
}

function unavailableReveal(_input: RevealPracticeQuestionInput): Promise<ImageOcclusionAttemptView> {
  return Promise.reject(new Error("Image-question revealing is unavailable in this host."));
}
