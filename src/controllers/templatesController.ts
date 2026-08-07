import {
  beginAssetDraft,
  cancelAssetDraft,
  syncAssetDraft,
  type AssetDraft,
} from "../ipc/assets";
import {
  createCardTemplate,
  deleteCardTemplate,
  getCardTemplate,
  listCardTemplateSummaries,
  updateCardTemplate,
  type CardTemplate,
  type CardTemplateSummary,
  type CreateCardTemplateInput,
  type UpdateCardTemplateInput,
} from "../ipc/templates";
import { latestResult, RequestLane } from "./requestLane";

export class TemplatesController {
  readonly #library = new RequestLane();
  readonly #template = new RequestLane();

  list(): Promise<CardTemplateSummary[] | undefined> {
    return latestResult(this.#library, listCardTemplateSummaries);
  }

  template(templateId: string): Promise<CardTemplate | undefined> {
    return latestResult(this.#template, () => getCardTemplate(templateId));
  }

  beginDraft(ownerId?: string): Promise<AssetDraft> {
    return beginAssetDraft("template", ownerId);
  }

  cancelDraft(draftId: string): Promise<void> {
    return cancelAssetDraft(draftId);
  }

  syncDraft(draftId: string, assetIds: string[]): Promise<void> {
    return syncAssetDraft(draftId, assetIds);
  }

  create(input: CreateCardTemplateInput): Promise<CardTemplate> {
    return createCardTemplate(input);
  }

  update(input: UpdateCardTemplateInput): Promise<CardTemplate> {
    return updateCardTemplate(input);
  }

  delete(templateId: string): Promise<void> {
    return deleteCardTemplate(templateId);
  }

  invalidate(): void {
    this.#library.invalidate();
    this.#template.invalidate();
  }
}
