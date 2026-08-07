import {
  getCardTemplate,
  listCardTemplateSummaries,
  type CardTemplate,
  type CardTemplateSummary,
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

  invalidate(): void {
    this.#library.invalidate();
    this.#template.invalidate();
  }
}
