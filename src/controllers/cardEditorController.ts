import { listTags, type TagSummary } from "../ipc/cards";
import { listDecks, type DeckSummary } from "../ipc/decks";
import {
  getCardTemplate,
  listCardTemplateSummaries,
  previewTemplateCards,
  type CardTemplate,
  type CardTemplateSummary,
  type PreviewTemplateCardsInput,
  type TemplateCardPreview,
} from "../ipc/templates";
import { latestResult, RequestLane } from "./requestLane";

export class CardEditorController {
  readonly #decks = new RequestLane();
  readonly #templates = new RequestLane();
  readonly #template = new RequestLane();
  readonly #tags = new RequestLane();
  readonly #preview = new RequestLane();

  decks(): Promise<DeckSummary[] | undefined> {
    return latestResult(this.#decks, listDecks);
  }

  templateSummaries(): Promise<CardTemplateSummary[] | undefined> {
    return latestResult(this.#templates, listCardTemplateSummaries);
  }

  template(templateId: string): Promise<CardTemplate | undefined> {
    return latestResult(this.#template, () => getCardTemplate(templateId));
  }

  tags(): Promise<TagSummary[] | undefined> {
    return latestResult(this.#tags, () => listTags());
  }

  preview(input: PreviewTemplateCardsInput): Promise<TemplateCardPreview[] | undefined> {
    return latestResult(this.#preview, () => previewTemplateCards(input));
  }

  invalidateTemplate(): void {
    this.#template.invalidate();
    this.#preview.invalidate();
  }

  invalidate(): void {
    this.#decks.invalidate();
    this.#templates.invalidate();
    this.#template.invalidate();
    this.#tags.invalidate();
    this.#preview.invalidate();
  }
}
