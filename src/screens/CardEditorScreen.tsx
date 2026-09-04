import type { ComponentProps } from "react";

import { SourceCitationPanel } from "../components/SourceCitationPanel";
import { CardEditorScreen as CardEditorScreenBase } from "./CardEditorScreenBase";

type CardEditorScreenProps = ComponentProps<typeof CardEditorScreenBase>;

/**
 * Card-detail shell shared by Search, Decks, and Practice.
 *
 * Existing generated notes load their durable source citation only after the
 * learner opens the card. Search rows therefore stay bounded while exact source
 * locators remain one interaction away and survive source-document removal.
 */
export function CardEditorScreen(props: CardEditorScreenProps) {
  return (
    <>
      {props.card?.noteId ? (
        <SourceCitationPanel noteId={props.card.noteId} compact />
      ) : null}
      <CardEditorScreenBase {...props} />
    </>
  );
}
