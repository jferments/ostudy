import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { TagInput } from "../components/TagInput";
import { CardEditorController } from "../controllers/cardEditorController";
import { useDurableDraft } from "../controllers/useDurableDraft";
import {
  EMPTY_RICH_DOCUMENT,
  cloneDocument,
  collectAssetIds,
  isDocumentEmpty,
} from "../editor/richText";
import type { Asset } from "../ipc/assets";
import { isQuestionNote, type AuthoringNote } from "../ipc/authoringNotes";
import {
  getCardNote,
  type BuiltInCardType,
  type CardNote,
  type ClozeMode,
  type DuplicateCard,
  type TagSummary,
} from "../ipc/cards";
import {
  createOperationId,
  getErrorCode,
  getErrorMessage,
  type RichDocument,
} from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import {
  type CardTemplate,
  type CardTemplateSummary,
  type GenerateTemplateCardsResult,
  type TemplateCardPreview,
  type TemplateValueInput,
} from "../ipc/templates";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { OverflowMenu, type OverflowMenuItem } from "../ui/overlays/OverflowMenu";
import { useConfirm } from "../ui/overlays/overlayContext";
import { PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { IconButton } from "../ui/icons/IconButton";
import { FormField } from "../ui/primitives/FormField";
import { InlineNotice, InlineProgress } from "../ui/primitives/InlineNotice";
import { NativeSelect } from "../ui/primitives/NativeSelect";
import { MoveCopyModal } from "./cards/MoveCopyModal";
import { BasicCardFields } from "./card-editor/BasicCardFields";
import { ClozeEditor } from "./card-editor/ClozeEditor";
import { InteractiveQuestionEditor } from "./card-editor/InteractiveQuestionEditor";
import { QuestionDraftPreview } from "./card-editor/QuestionDraftPreview";
import { CardPreviewWorkspace } from "./card-editor/CardPreviewWorkspace";
import { useEditorPreview } from "./card-editor/useEditorPreview";
import { RecallDraftPreview, type RecallPreviewPresentation } from "./card-editor/RecallDraftPreview";
import { TemplateEntryForm } from "./card-editor/TemplateEntryForm";
import { distinctClozeGroups } from "./card-editor/cloze";
import { presentation } from "./card-editor/presentation";
import {
  collectQuestionDraftAssetIds,
  definitionFromDraft,
  draftFromQuestionNote,
  isQuestionEditorKind,
  newQuestionDraft,
  questionDraftReady,
  type QuestionDraftChange,
  type QuestionEditorDraft,
  type QuestionEditorKind,
} from "./card-editor/questionDraft";

interface CardEditorDraftPayload extends Record<string, unknown> {
  selectedDeckId: string;
  selectedTemplateKey: string;
  reverseEnabled: boolean;
  clozeSource: string;
  clozeMode: ClozeMode;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  assets: Asset[];
  tags: string[];
  templateValues: TemplateValueInput[];
  questionDrafts?: Partial<Record<QuestionEditorKind, QuestionEditorDraft>>;
}

interface CardEditorScreenProps {
  deck: DeckSummary;
  decks?: DeckSummary[];
  card: AuthoringNote | null;
  onCancel: () => void;
  onSaved: (card: CardNote, continueAdding: boolean) => void | Promise<void>;
  onQuestionSaved?: (noteId: string, continueAdding: boolean) => void | Promise<void>;
  onTemplateGenerated: (
    result: GenerateTemplateCardsResult,
    continueAdding: boolean,
    template: CardTemplate,
  ) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  onMoveCopy?: () => void;
  /** More actions for a saved card, such as Suspend, beside Move / copy in its menu. */
  moreActions?: OverflowMenuItem[];
  /** Read-only facts below the fields, such as the card's schedule. */
  details?: ReactNode;
  /** Close with an × at the end of the header instead of a Cancel button, for an editor shown as a panel. */
  closeButton?: boolean;
}

export function CardEditorScreen({
  deck,
  decks: providedDecks,
  card,
  onCancel,
  onSaved,
  onQuestionSaved,
  onTemplateGenerated,
  onDirtyChange,
  onMoveCopy,
  moreActions = [],
  details,
  closeButton = false,
}: CardEditorScreenProps) {
  const existing = card !== null && card.noteId !== "";
  const questionNote = card && isQuestionNote(card) ? card : null;
  const recallNote = card && !isQuestionNote(card) ? card : null;
  const initialQuestionDraft = questionNote ? draftFromQuestionNote(questionNote) : null;
  const [loadedDecks, setLoadedDecks] = useState<DeckSummary[]>(providedDecks ?? []);
  const availableDecks = useMemo(() => {
    const byId = new Map<string, DeckSummary>();
    for (const candidate of [deck, ...loadedDecks]) {
      if (candidate.archivedAt === null) byId.set(candidate.id, candidate);
    }
    return [...byId.values()];
  }, [deck, loadedDecks]);
  const [selectedDeckId, setSelectedDeckId] = useState(card?.deckId ?? deck.id);
  const selectedDeck = availableDecks.find((candidate) => candidate.id === selectedDeckId) ?? deck;

  const [templateSummaries, setTemplateSummaries] = useState<CardTemplateSummary[]>([]);
  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string>(() => {
    if (questionNote) return questionNote.definition.question.kind;
    return recallNote?.cardType ?? "basic";
  });
  const [selectedCustomTemplate, setSelectedCustomTemplate] = useState<CardTemplate | null>(null);
  const [isLoadingTemplate, setIsLoadingTemplate] = useState(false);
  const builtInCardType: BuiltInCardType | null = selectedTemplateKey === "cloze"
    ? "cloze"
    : selectedTemplateKey === "basic"
      ? "basic"
      : null;
  const [questionDrafts, setQuestionDrafts] = useState<Partial<Record<QuestionEditorKind, QuestionEditorDraft>>>(() => {
    if (!initialQuestionDraft) return {};
    return { [initialQuestionDraft.kind]: initialQuestionDraft };
  });
  const activeQuestionDraft = isQuestionEditorKind(selectedTemplateKey)
    ? questionDrafts[selectedTemplateKey] ?? null
    : null;
  const questionOperationRef = useRef<string | null>(null);

  const [reverseEnabled, setReverseEnabled] = useState(
    recallNote?.cardType === "basic" ? recallNote.reverseEnabled : false,
  );
  const [clozeSource, setClozeSource] = useState(
    recallNote?.cardType === "cloze" ? recallNote.clozeSource : "",
  );
  const [clozeMode, setClozeMode] = useState<ClozeMode>(
    recallNote?.cardType === "cloze" ? recallNote.clozeMode : "one_at_a_time",
  );
  const [frontDoc, setFrontDoc] = useState<RichDocument>(() =>
    recallNote?.cardType === "basic"
      ? cloneDocument(recallNote.frontDoc)
      : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [backDoc, setBackDoc] = useState<RichDocument>(() =>
    recallNote?.cardType === "basic"
      ? cloneDocument(recallNote.backDoc)
      : cloneDocument(EMPTY_RICH_DOCUMENT),
  );
  const [frontText, setFrontText] = useState(recallNote?.cardType === "basic" ? recallNote.frontText : "");
  const [backText, setBackText] = useState(recallNote?.cardType === "basic" ? recallNote.backText : "");
  const [assets, setAssets] = useState<Asset[]>(
    questionNote?.assets ?? (recallNote?.cardType === "basic" ? recallNote.assets : []),
  );
  const [assetDraftId, setAssetDraftId] = useState<string | null>(null);
  const assetDraftRef = useRef<string | null>(null);
  const [tags, setTags] = useState<string[]>(card?.tags ?? []);
  const [tagSuggestions, setTagSuggestions] = useState<TagSummary[]>([]);
  const [templateValues, setTemplateValues] = useState<TemplateValueInput[]>([]);
  const [templatePreviews, setTemplatePreviews] = useState<TemplateCardPreview[]>([]);
  const [isPreviewingTemplate, setIsPreviewingTemplate] = useState(false);

  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [duplicateMatches, setDuplicateMatches] = useState<DuplicateCard[]>([]);
  const [moveCopyOpen, setMoveCopyOpen] = useState(false);
  const preview = useEditorPreview();
  const previewOpen = preview.isOpen;
  const [previewPresentations, setPreviewPresentations] = useState<RecallPreviewPresentation[]>([]);
  const [isPreparingPreview, setIsPreparingPreview] = useState(false);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [dirty, setDirty] = useState(false);
  const platformBackCancelRef = useRef<() => void>(() => undefined);
  const controllerRef = useRef<CardEditorController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardEditorController();
  const controller = controllerRef.current;
  const confirm = useConfirm();

  const title = existing ? "Edit card" : "Add card";
  const hasFront = !isDocumentEmpty(frontText, frontDoc);
  const hasBack = !isDocumentEmpty(backText, backDoc);
  const clozeGroups = useMemo(() => distinctClozeGroups(clozeSource), [clozeSource]);
  const templateValuesComplete = selectedCustomTemplate !== null
    && templateValues.length === selectedCustomTemplate.fields.length
    && templateValues.every((entry) => !isDocumentEmpty(entry.text, entry.doc));
  const contentReady = activeQuestionDraft
    ? questionDraftReady(activeQuestionDraft)
    : selectedCustomTemplate
      ? templateValuesComplete
      : builtInCardType === "cloze"
        ? clozeSource.trim() !== "" && clozeGroups.length > 0
        : builtInCardType === "basic" && hasFront && hasBack;
  const canSave = contentReady && selectedDeckId !== "" && !isSaving && !isLoadingTemplate;
  const canPreview = !isLoadingTemplate
    && (builtInCardType !== null || activeQuestionDraft !== null || selectedCustomTemplate !== null);
  const assetIds = useMemo(() => new Set(assets.map((asset) => asset.id)), [assets]);
  const usedAssetIds = useMemo(() => {
    const ids = new Set<string>();
    for (const id of collectAssetIds(frontDoc)) ids.add(id);
    for (const id of collectAssetIds(backDoc)) ids.add(id);
    for (const value of templateValues) {
      for (const id of collectAssetIds(value.doc)) ids.add(id);
    }
    for (const draft of Object.values(questionDrafts)) {
      if (!draft) continue;
      for (const id of collectQuestionDraftAssetIds(draft)) ids.add(id);
    }
    return [...ids].sort();
  }, [backDoc, frontDoc, questionDrafts, templateValues]);
  const usedAssetKey = usedAssetIds.join(",");
  const durablePayload = useMemo<CardEditorDraftPayload>(() => ({
    selectedDeckId,
    selectedTemplateKey,
    reverseEnabled,
    clozeSource,
    clozeMode,
    frontDoc,
    backDoc,
    frontText,
    backText,
    assets,
    tags,
    templateValues,
    questionDrafts,
  }), [
    assets,
    backDoc,
    backText,
    clozeMode,
    clozeSource,
    frontDoc,
    frontText,
    questionDrafts,
    reverseEnabled,
    selectedDeckId,
    selectedTemplateKey,
    tags,
    templateValues,
  ]);
  const durableDraft = useDurableDraft<CardEditorDraftPayload>({
    kind: "card",
    targetEntityId: card?.noteId || undefined,
    contextEntityId: card?.noteId ? undefined : deck.id,
    // A prefilled card that is not saved yet has no revision to build on.
    baseRevision: card?.noteId ? card.revision : undefined,
    payload: durablePayload,
    assetIds: usedAssetIds,
    onRecovered: (payload) => {
      setSelectedDeckId(payload.selectedDeckId);
      setSelectedTemplateKey(payload.selectedTemplateKey);
      setReverseEnabled(payload.reverseEnabled);
      setClozeSource(payload.clozeSource);
      setClozeMode(payload.clozeMode);
      setFrontDoc(payload.frontDoc);
      setBackDoc(payload.backDoc);
      setFrontText(payload.frontText);
      setBackText(payload.backText);
      setAssets(payload.assets);
      setTags(payload.tags);
      setTemplateValues(payload.templateValues);
      setQuestionDrafts(payload.questionDrafts ?? {});
      setDirty(true);
    },
  });

  useEffect(() => () => controller.invalidate(), [controller]);

  useEffect(() => {
    onDirtyChange(durableDraft.hasUnsavedLocalChanges);
    return () => onDirtyChange(false);
  }, [durableDraft.hasUnsavedLocalChanges, onDirtyChange]);

  useEffect(() => {
    platformBackCancelRef.current = () => {
      if (previewOpen) preview.close();
      else void cancel();
    };
  });

  useEffect(
    () => registerPlatformBackHandler(() => {
      platformBackCancelRef.current();
      return true;
    }),
    [],
  );

  useEffect(() => {
    assetDraftRef.current = durableDraft.assetDraftId;
    setAssetDraftId(durableDraft.assetDraftId);
  }, [durableDraft.assetDraftId]);

  useEffect(() => {
    if (!assetDraftId) return;
    const retained = usedAssetKey === "" ? [] : usedAssetKey.split(",");
    const timer = window.setTimeout(() => {
      void controller.syncDraft(assetDraftId, retained).catch((error: unknown) => {
        if (assetDraftRef.current === assetDraftId) {
          setErrorMessage(`The media draft could not be synchronized: ${getErrorMessage(error)}`);
        }
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [assetDraftId, controller, usedAssetKey]);

  useEffect(() => {
    if (providedDecks) {
      setLoadedDecks(providedDecks);
      return;
    }
    let current = true;
    void controller.decks()
      .then((decks) => {
        if (current && decks) setLoadedDecks(decks);
      })
      .catch((error: unknown) => {
        if (current) setErrorMessage(`The deck list could not be loaded: ${getErrorMessage(error)}`);
      });
    return () => {
      current = false;
    };
  }, [controller, providedDecks]);

  useEffect(() => {
    let current = true;
    void controller.templateSummaries()
      .then((loaded) => {
        if (current && loaded) setTemplateSummaries(loaded);
      })
      .catch((error: unknown) => {
        if (current) setErrorMessage(`Card templates could not be loaded: ${getErrorMessage(error)}`);
      });
    return () => {
      current = false;
    };
  }, [controller]);

  useEffect(() => {
    if (selectedTemplateKey === "basic" || selectedTemplateKey === "cloze" || isQuestionEditorKind(selectedTemplateKey)) {
      controller.invalidateTemplate();
      setSelectedCustomTemplate(null);
      setIsLoadingTemplate(false);
      if (isQuestionEditorKind(selectedTemplateKey)) {
        setQuestionDrafts((current) => current[selectedTemplateKey]
          ? current
          : { ...current, [selectedTemplateKey]: newQuestionDraft(selectedTemplateKey) });
      }
      return;
    }
    setSelectedCustomTemplate(null);
    setTemplateValues([]);
    setTemplatePreviews([]);
    setIsLoadingTemplate(true);
    void controller.template(selectedTemplateKey)
      .then((template) => {
        if (template) setSelectedCustomTemplate(template);
        setIsLoadingTemplate(false);
      })
      .catch((error: unknown) => {
        setErrorMessage(getErrorMessage(error));
        setIsLoadingTemplate(false);
      });
  }, [controller, selectedTemplateKey]);

  useEffect(() => {
    void controller.tags()
      .then((loaded) => {
        if (loaded) setTagSuggestions(loaded);
      })
      .catch((error: unknown) => {
        console.error("Unable to load tag suggestions:", getErrorMessage(error));
      });
  }, [controller]);

  useEffect(() => {
    if (!selectedCustomTemplate) {
      setTemplateValues([]);
      setTemplatePreviews([]);
      return;
    }
    setTemplateValues((current) => selectedCustomTemplate.fields.map((field) => {
      const existingValue = current.find((entry) => entry.fieldId === field.id);
      return {
        fieldId: field.id,
        doc: existingValue?.doc ?? cloneDocument(EMPTY_RICH_DOCUMENT),
        text: existingValue?.text ?? "",
      };
    }));
    setAssets((current) => {
      const byId = new Map(current.map((asset) => [asset.id, asset]));
      for (const asset of selectedCustomTemplate.assets) byId.set(asset.id, asset);
      return [...byId.values()];
    });
    setTemplatePreviews([]);
  }, [selectedCustomTemplate]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (!previewOpen && canSave) void save(false, false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function addAsset(asset: Asset): void {
    if (!assetIds.has(asset.id)) setAssets((current) => [...current, asset]);
    markDirty();
  }

  function markDirty(): void {
    if (savingRef.current) return;
    setDirty(true);
    setErrorMessage(null);
    setDuplicateMatches([]);
    setTemplatePreviews([]);
    setPreviewPresentations([]);
  }

  function updateQuestionDraft(change: QuestionDraftChange): void {
    if (savingRef.current || !isQuestionEditorKind(selectedTemplateKey)) return;
    const kind = selectedTemplateKey;
    setQuestionDrafts((current) => {
      const previous = current[kind];
      if (!previous) return current;
      const next = typeof change === "function" ? change(previous) : change;
      return next.kind === kind ? { ...current, [kind]: next } : current;
    });
    markDirty();
  }

  function markAssetDraftCommitted(): void {
    assetDraftRef.current = null;
    setAssetDraftId(null);
  }

  async function restartDurableDraft(nextQuestionDrafts: Partial<Record<QuestionEditorKind, QuestionEditorDraft>>): Promise<void> {
    await durableDraft.restart({
      selectedDeckId,
      selectedTemplateKey,
      reverseEnabled: false,
      clozeSource: "",
      clozeMode: "one_at_a_time",
      frontDoc: cloneDocument(EMPTY_RICH_DOCUMENT),
      backDoc: cloneDocument(EMPTY_RICH_DOCUMENT),
      frontText: "",
      backText: "",
      assets: selectedCustomTemplate?.assets ?? [],
      tags: [],
      templateValues: selectedCustomTemplate
        ? selectedCustomTemplate.fields.map((field) => ({
            fieldId: field.id,
            doc: cloneDocument(EMPTY_RICH_DOCUMENT),
            text: "",
          }))
        : [],
      questionDrafts: nextQuestionDrafts,
    });
  }

  async function buildTemplatePreview(): Promise<void> {
    if (!selectedCustomTemplate || !templateValuesComplete || isPreviewingTemplate) return;
    setIsPreviewingTemplate(true);
    setErrorMessage(null);
    try {
      const preview = await controller.preview({
        templateId: selectedCustomTemplate.id,
        templateRevision: selectedCustomTemplate.revision,
        values: templateValues,
      });
      if (preview) setTemplatePreviews(preview);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsPreviewingTemplate(false);
    }
  }

  async function openCardPreview(): Promise<void> {
    if (!canPreview || previewOpen || isSaving || isPreparingPreview) return;
    setErrorMessage(null);
    setPreviewPresentations([]);
    preview.open();
    if (activeQuestionDraft) return;

    setIsPreparingPreview(true);
    try {
      if (selectedCustomTemplate) {
        if (!templateValuesComplete) {
          setErrorMessage("Fill in every template field before previewing generated cards.");
          return;
        }
        const generated = await controller.preview({
          templateId: selectedCustomTemplate.id,
          templateRevision: selectedCustomTemplate.revision,
          values: templateValues,
        });
        if (!generated) return;
        setTemplatePreviews(generated);
        setPreviewPresentations(generated.map((item) => ({
          presentationKey: `template:${item.ruleId}`,
          presentationLabel: item.ruleName,
          cardLabel: item.ruleName,
          frontDoc: item.questionDoc,
          backDoc: item.answerDoc,
          frontText: item.questionText,
          backText: item.answerText,
          assets: item.assets,
        })));
        return;
      }

      if (builtInCardType === "basic") {
        const generated = await controller.previewRecall({
          cardType: "basic",
          reverseEnabled,
          frontDoc,
          backDoc,
        });
        if (generated) {
          setPreviewPresentations(generated.map((item) => ({ ...item, assets })));
        }
        return;
      }

      if (builtInCardType === "cloze") {
        const generated = await controller.previewRecall({
          cardType: "cloze",
          clozeSource,
          clozeMode,
        });
        if (generated) {
          setPreviewPresentations(generated.map((item) => ({ ...item, assets })));
        }
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsPreparingPreview(false);
    }
  }

  async function save(continueAdding: boolean, allowDuplicate: boolean): Promise<void> {
    if (previewOpen) return;
    if (!contentReady) {
      setErrorMessage(activeQuestionDraft
        ? "Complete the required question fields and answer key before saving."
        : selectedCustomTemplate
          ? "Fill in every template field before generating cards."
          : builtInCardType === "cloze"
            ? "Write a statement and mark at least one blank."
            : "Add content to both the question and answer sides.");
      return;
    }
    if (savingRef.current) return;

    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);

    try {
      const editorDraft = await durableDraft.prepareCanonicalCommit();
      if (activeQuestionDraft) {
        const operationId = questionOperationRef.current ?? createOperationId();
        questionOperationRef.current = operationId;
        const receipt = await controller.saveQuestion({
          operationId,
          noteId: questionNote?.noteId ?? null,
          revision: questionNote?.revision ?? null,
          deckId: selectedDeckId,
          definitionJson: JSON.stringify(definitionFromDraft(activeQuestionDraft)),
          tags,
          allowDuplicate,
          assetDraftId: assetDraftId ?? undefined,
          editorDraft,
        });
        questionOperationRef.current = null;
        durableDraft.acceptCanonicalCommit();
        markAssetDraftCommitted();
        setDirty(false);
        setDuplicateMatches([]);
        try {
          await onQuestionSaved?.(receipt.noteId, continueAdding && !existing);
          if (continueAdding && !existing) await resetForAnother([]);
        } catch (error: unknown) {
          setErrorMessage(`The question was saved, but the screen could not finish updating: ${getErrorMessage(error)}`);
        }
        return;
      }

      if (selectedCustomTemplate) {
        const result = await controller.generate({
          deckId: selectedDeckId,
          templateId: selectedCustomTemplate.id,
          templateRevision: selectedCustomTemplate.revision,
          values: templateValues,
          tags,
          assetDraftId: assetDraftId ?? undefined,
          editorDraft,
        });
        durableDraft.acceptCanonicalCommit();
        markAssetDraftCommitted();
        setDirty(false);
        setTemplatePreviews([]);
        await onTemplateGenerated(result, continueAdding, selectedCustomTemplate);
        if (continueAdding) await resetForAnother(selectedCustomTemplate.assets);
        return;
      }

      if (builtInCardType === "basic" && !allowDuplicate && frontText.trim() !== "" && backText.trim() !== "") {
        const duplicates = await controller.duplicates({
          deckId: selectedDeckId,
          frontText,
          backText,
          excludeNoteId: existing && recallNote ? recallNote.noteId : undefined,
        });
        if (duplicates.length > 0) {
          setDuplicateMatches(duplicates);
          return;
        }
      }

      let saved: CardNote;
      if (builtInCardType === "cloze") {
        const input = {
          deckId: selectedDeckId,
          cardType: "cloze" as const,
          clozeSource,
          clozeMode,
          tags,
          allowDuplicate,
          assetDraftId: assetDraftId ?? undefined,
          editorDraft,
        };
        saved = existing && recallNote
          ? await controller.update({ ...input, noteId: recallNote.noteId, revision: recallNote.revision })
          : await controller.create(input);
      } else {
        const input = {
          deckId: selectedDeckId,
          cardType: "basic" as const,
          reverseEnabled,
          frontDoc,
          backDoc,
          tags,
          allowDuplicate,
          assetDraftId: assetDraftId ?? undefined,
          editorDraft,
        };
        saved = existing && recallNote
          ? await controller.update({ ...input, noteId: recallNote.noteId, revision: recallNote.revision })
          : await controller.create(input);
      }

      durableDraft.acceptCanonicalCommit();
      markAssetDraftCommitted();
      setDirty(false);
      setDuplicateMatches([]);
      try {
        await onSaved(saved, continueAdding && !existing);
        if (continueAdding && !existing) await resetForAnother([]);
      } catch (error: unknown) {
        setErrorMessage(`The flashcard was saved, but the screen could not finish updating: ${getErrorMessage(error)}`);
      }
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (activeQuestionDraft && code !== null) questionOperationRef.current = null;
      if (code === "duplicate_card") {
        setDuplicateMatches([{
          noteId: "",
          deckId: selectedDeckId,
          frontText: activeQuestionDraft
            ? questionSummary(activeQuestionDraft)
            : builtInCardType === "cloze" ? clozeSource : frontText,
          backText: activeQuestionDraft ? "Interactive question" : builtInCardType === "cloze" ? "Fill in the blank" : backText,
        }]);
      } else {
        setErrorMessage(getErrorMessage(error));
      }
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  async function resetForAnother(initialAssets: Asset[]): Promise<void> {
    const nextQuestionDrafts = isQuestionEditorKind(selectedTemplateKey)
      ? { [selectedTemplateKey]: newQuestionDraft(selectedTemplateKey) }
      : {};
    setFrontDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setBackDoc(cloneDocument(EMPTY_RICH_DOCUMENT));
    setFrontText("");
    setBackText("");
    setAssets(initialAssets);
    setTags([]);
    setReverseEnabled(false);
    setClozeSource("");
    setClozeMode("one_at_a_time");
    setQuestionDrafts(nextQuestionDrafts);
    setTemplateValues(selectedCustomTemplate
      ? selectedCustomTemplate.fields.map((field) => ({
          fieldId: field.id,
          doc: cloneDocument(EMPTY_RICH_DOCUMENT),
          text: "",
        }))
      : []);
    preview.close();
    setPreviewPresentations([]);
    setIsPreparingPreview(false);
    setEditorGeneration((value) => value + 1);
    try {
      await restartDurableDraft(nextQuestionDrafts);
    } catch (error: unknown) {
      setErrorMessage(`The next media draft could not open: ${getErrorMessage(error)}`);
    }
  }

  function openMoveCopy(): void {
    if (!existing || !recallNote || dirty) return;
    if (onMoveCopy) {
      onMoveCopy();
      return;
    }
    setMoveCopyOpen(true);
  }

  async function cancel(): Promise<void> {
    if (isSaving) return;
    if (dirty) {
      const accepted = await confirm({
        title: "Discard card changes?",
        description: "The unsaved changes to this card will be lost.",
        confirmLabel: "Discard changes",
        destructive: true,
      });
      if (!accepted) return;
    }
    try {
      await durableDraft.discard();
    } catch (error: unknown) {
      setErrorMessage(`The durable draft could not be discarded: ${getErrorMessage(error)}`);
      return;
    }
    assetDraftRef.current = null;
    setAssetDraftId(null);
    onCancel();
  }

  const saveLabel = activeQuestionDraft
    ? "Save question"
    : selectedCustomTemplate
      ? `Generate ${selectedCustomTemplate.rules.length} ${selectedCustomTemplate.rules.length === 1 ? "card" : "cards"}`
      : isLoadingTemplate
        ? "Loading template…"
        : "Save card";

  const cardMenu: OverflowMenuItem[] = [
    ...(existing && recallNote
      ? [{ id: "move-copy", label: dirty ? "Move / copy (save first)" : "Move / copy", disabled: dirty, onSelect: openMoveCopy }]
      : []),
    ...moreActions,
  ];

  return (
    <>
    {previewOpen ? <CardPreviewWorkspace title={selectedDeck.name} onClose={preview.close}>
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {activeQuestionDraft ? <QuestionDraftPreview draft={activeQuestionDraft} assets={assets}
        assetDraftId={assetDraftId} disabled={isSaving} onError={setErrorMessage} /> :
        <RecallDraftPreview presentations={previewPresentations} tags={tags} loading={isPreparingPreview}
          disabled={isSaving} onError={setErrorMessage} />}
    </CardPreviewWorkspace> : null}
    <div ref={preview.editorRef} hidden={previewOpen} inert={previewOpen} onBlurCapture={preview.rememberField}>
    <PageScaffold aria-busy={isSaving || isLoadingTemplate}>
      <header
        data-slot="card-editor-header"
        className="tw:sticky tw:top-0 tw:z-(--ostudy-layer-sticky) tw:mb-4 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-x-4 tw:gap-y-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:px-4 tw:py-2.5 tw:shadow-sm"
      >
        <div className="tw:min-w-0 tw:flex-[1_1_10rem]">
          <span className="tw:block tw:truncate tw:text-caption tw:text-muted-foreground">{deck.name}</span>
          <h1 className="tw:m-0 tw:text-title-small tw:font-semibold">{title}</h1>
        </div>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-end tw:gap-2">
          {closeButton ? null : <Button variant="text" disabled={isSaving} onClick={() => void cancel()}>Cancel</Button>}
          {canPreview ? <Button variant="secondary" disabled={isSaving || isPreparingPreview}
            onClick={() => void openCardPreview()}>Preview card</Button> : null}
          {!existing ? <Button variant="secondary" disabled={!canSave} onClick={() => void save(true, false)}>
            {selectedCustomTemplate ? "Generate + another" : "Save + another"}
          </Button> : null}
          <Button variant="primary" disabled={!canSave} onClick={() => void save(false, false)}>
            {isSaving ? activeQuestionDraft ? "Saving question…" : selectedCustomTemplate ? "Generating…" : "Saving…" : saveLabel}
          </Button>
          {cardMenu.length > 0 ? <OverflowMenu label="Card actions" items={cardMenu} disabled={isSaving} /> : null}
          {closeButton ? <IconButton icon="close" label="Close editor" disabled={isSaving} onClick={() => void cancel()} /> : null}
        </div>
      </header>
      {errorMessage ? <InlineNotice kind="error" className="tw:mb-5">{errorMessage}</InlineNotice> : null}
      {duplicateMatches.length > 0 ? (
        <InlineNotice kind="warning" title="This looks like an exact duplicate" className="tw:mb-5">
          <p>An identical item already exists in {selectedDeck.name}. Review it before creating another copy.</p>
          <div className="tw:grid tw:gap-3">
            {duplicateMatches.filter((match) => match.noteId !== "").slice(0, 3).map((match) => (
              <article className="tw:grid tw:gap-1" key={match.noteId}><strong>{summarize(match.frontText)}</strong><span>{summarize(match.backText)}</span></article>
            ))}
            <div className="tw:flex tw:flex-wrap tw:gap-2">
              <Button variant="secondary" disabled={isSaving} onClick={() => setDuplicateMatches([])}>Keep editing</Button>
              <Button variant="danger" disabled={isSaving} onClick={() => void save(false, true)}>Save duplicate anyway</Button>
            </div>
          </div>
        </InlineNotice>
      ) : null}
      <div className={presentation["card-editor-layout"]}>
        <div className={presentation["card-editor-fields"]}>
          <div className="tw:grid tw:gap-4 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:px-4 tw:py-3 tw:sm:grid-cols-2">
            <FormField label="Card template">
              <NativeSelect value={selectedTemplateKey} disabled={isSaving || existing} onChange={(event) => {
                setSelectedTemplateKey(event.currentTarget.value);
                preview.close();
                setPreviewPresentations([]);
                setIsPreparingPreview(false);
                markDirty();
              }}>
                <optgroup label="Recall cards">
                  <option value="basic">Basic</option>
                  <option value="cloze">Fill in the blank</option>
                </optgroup>
                <optgroup label="Interactive questions">
                  <option value="single_choice">Single-best-answer</option>
                  <option value="select_all">Select all that apply</option>
                  <option value="true_false">True / false</option>
                  <option value="matching">Matching</option>
                  <option value="grouping">Grouping</option>
                  <option value="ordering">Ordering</option>
                  <option value="image_occlusion">Image occlusion</option>
                </optgroup>
                {templateSummaries.length > 0 ? <optgroup label="Reusable templates">{templateSummaries.map((template) => (
                  <option key={template.id} value={template.id}>{template.name}</option>
                ))}</optgroup> : null}
              </NativeSelect>
            </FormField>
            <FormField label="Deck">
              <NativeSelect value={selectedDeckId} disabled={isSaving} onChange={(event) => {
                setSelectedDeckId(event.currentTarget.value);
                markDirty();
              }}>
                {availableDecks.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}
              </NativeSelect>
            </FormField>
          </div>
          {isLoadingTemplate ? <InlineProgress>Loading template…</InlineProgress> : null}
          {activeQuestionDraft ? (
            <InteractiveQuestionEditor key={`${activeQuestionDraft.kind}-${editorGeneration}`} draft={activeQuestionDraft}
              assets={assets} assetDraftId={assetDraftId} disabled={isSaving} onAssetImported={addAsset}
              onChange={updateQuestionDraft} flushDraft={durableDraft.flush} />
          ) : selectedCustomTemplate ? (
            <TemplateEntryForm key={`${selectedCustomTemplate.id}-${editorGeneration}`} template={selectedCustomTemplate}
              values={templateValues} previews={templatePreviews} assets={assets} assetDraftId={assetDraftId}
              disabled={isSaving} isPreviewing={isPreviewingTemplate} onAssetImported={addAsset}
              onValueChange={(fieldId, doc, text) => {
                setTemplateValues((current) => current.map((entry) => entry.fieldId === fieldId ? { ...entry, doc, text } : entry));
                markDirty();
              }} onPreview={() => void buildTemplatePreview()} />
          ) : builtInCardType === "cloze" ? (
            <ClozeEditor source={clozeSource} mode={clozeMode} disabled={isSaving}
              onSourceChange={(source) => { setClozeSource(source); markDirty(); }}
              onModeChange={(mode) => { setClozeMode(mode); markDirty(); }} />
          ) : (
            <BasicCardFields key={`basic-${editorGeneration}`} frontDoc={frontDoc} backDoc={backDoc}
              reverseEnabled={reverseEnabled} assets={assets} assetDraftId={assetDraftId}
              disabled={isSaving} onAssetImported={addAsset}
              onFieldChange={(fieldId, document, plainText) => {
                if (fieldId === "front") { setFrontDoc(document); setFrontText(plainText); }
                else { setBackDoc(document); setBackText(plainText); }
                markDirty();
              }} onReverseChange={(enabled) => { setReverseEnabled(enabled); markDirty(); }} />
          )}
          <TagInput tags={tags} suggestions={tagSuggestions} disabled={isSaving}
            onChange={(nextTags) => { setTags(nextTags); markDirty(); }} />
          {details}
        </div>

      </div>
      {moveCopyOpen && recallNote ? (
        <MoveCopyModal cards={[{ noteId: recallNote.noteId, deckId: recallNote.deckId }]} decks={availableDecks}
          onClose={() => setMoveCopyOpen(false)} onComplete={async () => {
            setMoveCopyOpen(false);
            const refreshed = await getCardNote(recallNote.noteId);
            await onSaved(refreshed, false);
          }} />
      ) : null}
    </PageScaffold>
    </div>
    </>
  );
}

function questionSummary(draft: QuestionEditorDraft): string {
  if (draft.kind === "image_occlusion") return `Image occlusion · ${draft.questions.length} questions`;
  if (draft.kind === "true_false") return draft.statementText;
  return draft.promptText;
}

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}
