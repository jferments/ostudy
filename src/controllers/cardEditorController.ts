import {
  beginAssetDraft,
  cancelAssetDraft,
  syncAssetDraft,
  type AssetDraft,
} from "../ipc/assets";
import {
  createCardNote,
  findDuplicateCards,
  listTags,
  updateCardNote,
  type CardNote,
  type CreateCardNoteInput,
  type DuplicateCard,
  type DuplicateCheckInput,
  type TagSummary,
  type UpdateCardNoteInput,
} from "../ipc/cards";
import { listDecks, type DeckSummary } from "../ipc/decks";
import {
  generateCardsFromTemplate,
  getCardTemplate,
  listCardTemplateSummaries,
  previewTemplateCards,
  type CardTemplate,
  type CardTemplateSummary,
  type GenerateTemplateCardsInput,
  type GenerateTemplateCardsResult,
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

  beginDraft(ownerId?: string): Promise<AssetDraft> {
    return beginAssetDraft("card", ownerId);
  }

  cancelDraft(draftId: string): Promise<void> {
    return cancelAssetDraft(draftId);
  }

  syncDraft(draftId: string, assetIds: string[]): Promise<void> {
    return syncAssetDraft(draftId, assetIds);
  }

  create(input: CreateCardNoteInput): Promise<CardNote> {
    return createCardNote(input);
  }

  update(input: UpdateCardNoteInput): Promise<CardNote> {
    return updateCardNote(input);
  }

  duplicates(input: DuplicateCheckInput): Promise<DuplicateCard[]> {
    return findDuplicateCards(input);
  }

  generate(input: GenerateTemplateCardsInput): Promise<GenerateTemplateCardsResult> {
    return generateCardsFromTemplate(input);
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
