import { invoke } from "@tauri-apps/api/core";

import type { Asset } from "./assets";
import type { RichDocument } from "./common";

export interface CardTemplateField {
  id: string;
  name: string;
  description: string;
  position: number;
}

export interface CardTemplateRule {
  id: string;
  name: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  position: number;
}

export interface CardTemplate {
  id: string;
  name: string;
  description: string;
  fields: CardTemplateField[];
  rules: CardTemplateRule[];
  assets: Asset[];
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface CardTemplateSummary {
  id: string;
  name: string;
  description: string;
  fieldCount: number;
  generatedCardRuleCount: number;
  imageCount: number;
  updatedAt: number;
  revision: number;
}

export interface CardTemplateFieldInput {
  id?: string;
  name: string;
  description: string;
}

export interface CardTemplateRuleInput {
  id?: string;
  name: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
}

export interface CreateCardTemplateInput {
  name: string;
  description: string;
  fields: CardTemplateFieldInput[];
  rules: CardTemplateRuleInput[];
  assetDraftId?: string;
}

export interface UpdateCardTemplateInput extends CreateCardTemplateInput {
  templateId: string;
  revision: number;
}

export interface TemplateValueInput {
  fieldId: string;
  doc: RichDocument;
  text: string;
}

export interface TemplateCardPreview {
  ruleId: string;
  ruleName: string;
  questionDoc: RichDocument;
  answerDoc: RichDocument;
  questionText: string;
  answerText: string;
  assets: Asset[];
}

export interface PreviewTemplateCardsInput {
  templateId: string;
  templateRevision: number;
  values: TemplateValueInput[];
}

export interface GenerateTemplateCardsInput extends PreviewTemplateCardsInput {
  deckId: string;
  tags: string[];
  assetDraftId?: string;
}

export interface GenerateTemplateCardsResult {
  createdCount: number;
  skippedDuplicates: number;
  createdNoteIds: string[];
}

export async function listCardTemplateSummaries(): Promise<CardTemplateSummary[]> {
  return invoke<CardTemplateSummary[]>("list_card_template_summaries");
}

export async function getCardTemplate(templateId: string): Promise<CardTemplate> {
  return invoke<CardTemplate>("get_card_template", { templateId });
}

export async function createCardTemplate(input: CreateCardTemplateInput): Promise<CardTemplate> {
  return invoke<CardTemplate>("create_card_template", { input });
}

export async function updateCardTemplate(input: UpdateCardTemplateInput): Promise<CardTemplate> {
  return invoke<CardTemplate>("update_card_template", { input });
}

export async function deleteCardTemplate(templateId: string): Promise<void> {
  return invoke<void>("delete_card_template", { templateId });
}

export async function previewTemplateCards(
  input: PreviewTemplateCardsInput,
): Promise<TemplateCardPreview[]> {
  return invoke<TemplateCardPreview[]>("preview_template_cards", { input });
}

export async function generateCardsFromTemplate(
  input: GenerateTemplateCardsInput,
): Promise<GenerateTemplateCardsResult> {
  return invoke<GenerateTemplateCardsResult>("generate_cards_from_template", { input });
}
