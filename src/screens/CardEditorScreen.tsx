import { useState, type ComponentProps } from "react";

import { SourceCitationPanel } from "../components/SourceCitationPanel";
import type { SourcePassage } from "../components/sourcePassage";
import type { ShowToast } from "../components/Toast";
import { CardEditorScreen as CardEditorScreenBase } from "./CardEditorScreenBase";
import { SourcePassageDialog } from "./documents/annotator/SourcePassageDialog";

type CardEditorScreenProps = ComponentProps<typeof CardEditorScreenBase> & {
  /** Reports annotator trouble; without it a cited page is not offered. */
  showToast?: ShowToast;
};

/**
 * Card-detail shell shared by Search, Decks, and Practice.
 *
 * Existing generated notes load their durable source citation only after the
 * learner opens the card. Search rows therefore stay bounded while exact source
 * locators remain one interaction away and survive source-document removal.
 * A cited PDF page opens over the editor, so unsaved edits stay put.
 */
export function CardEditorScreen({ showToast, ...props }: CardEditorScreenProps) {
  const [passage, setPassage] = useState<SourcePassage | null>(null);
  return (
    <>
      {props.card?.noteId ? (
        <SourceCitationPanel noteId={props.card.noteId} compact onOpenPassage={showToast ? setPassage : undefined} />
      ) : null}
      <CardEditorScreenBase {...props} />
      {passage && showToast ? <SourcePassageDialog passage={passage} onClose={() => setPassage(null)} showToast={showToast} /> : null}
    </>
  );
}
