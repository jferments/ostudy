from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:160]!r}")
    write(path, text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# PDF v6: before raster OCR, give every OCR candidate a second native-text
# extraction through the already-bound PDFium runtime. This is deliberately
# engine-diverse: oxidize-pdf remains the semantic extractor; PDFium is a
# candidate-page native-text rescue; Tesseract is the last resort only.
# ---------------------------------------------------------------------------
pdf_path = "src-tauri/src/pdf_backend.rs"
pdf = read(pdf_path)
if pdf.count("oxidize-pdf-4.2.2-rag-native-first-ocr-v5") != 2:
    raise SystemExit("expected v5 PDF backend id exactly twice")
pdf = pdf.replace(
    "oxidize-pdf-4.2.2-rag-native-first-ocr-v5",
    "oxidize-pdf-4.2.2-rag-native-first-ocr-v6",
)
pdf = pdf.replace(
    "fn native_page_requires_ocr(text: &str, assessment: &TextQualityAssessment) -> bool {",
    "pub(crate) fn native_page_requires_ocr(text: &str, assessment: &TextQualityAssessment) -> bool {",
    1,
)
pdf = pdf.replace(
    "        || ('\\u{e000}'..='\\u{f8ff}').contains(&character)\n",
    "        || is_private_use(character)\n",
    1,
)
insert = '''
    #[test]
    fn alternative_native_text_can_rescue_a_primary_extractor_false_positive() {
        let primary = format!(
            "{} readable respiratory physiology text with enough remaining words to look locally damaged",
            "�\\u{0001}".repeat(28)
        );
        let primary_assessment = assess_text_quality(&primary);
        assert!(primary_assessment.suspicious);
        assert!(native_page_requires_ocr(&primary, &primary_assessment));

        let pdfium_native = concat!(
            "A textbook page can contain tables, figures, and readable explanatory prose. ",
            "A malformed embedded font may leave bron\\u{fffe}chial or pulmo\\u{fffe}nary placeholders ",
            "without making the native text layer unusable."
        );
        let rescue_assessment = assess_text_quality(pdfium_native);
        assert!(!native_page_requires_ocr(pdfium_native, &rescue_assessment));
    }

'''
marker = '''    #[test]
    fn recovered_ocr_text_replaces_corrupt_page_and_semantic_content() {
'''
if pdf.count(marker) != 1:
    raise SystemExit("PDF test insertion marker missing")
pdf = pdf.replace(marker, insert + marker, 1)
write(pdf_path, pdf)

render_path = "src-tauri/src/pdf_render_backend.rs"
render = read(render_path)
marker = '''pub(crate) fn render_pages<E, F>(
'''
helper = r'''pub(crate) fn extract_native_text_pages(
    runtime: &PdfRenderRuntime,
    source_path: &std::path::Path,
    page_numbers: &[usize],
) -> Result<Vec<(usize, String)>, PdfRenderError> {
    if runtime.backend_id != active_backend_id() {
        return Err(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "The PDF renderer runtime does not match the active renderer backend.",
        ));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let document = runtime
            .pdfium
            .load_pdf_from_file(source_path, None)
            .map_err(|error| {
                PdfRenderError::new(PdfRenderErrorKind::Open, None, error.to_string())
            })?;
        let total_pages = page_count(&document)?;
        let mut requested = page_numbers.to_vec();
        requested.sort_unstable();
        requested.dedup();
        for page_number in &requested {
            if *page_number == 0 || *page_number > total_pages {
                return Err(PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(*page_number),
                    "The native-text fallback requested a page outside the PDF page range.",
                ));
            }
        }

        let mut extracted = Vec::with_capacity(requested.len());
        for page_number in requested {
            let index = PdfPageIndex::try_from(page_number - 1).map_err(|_| {
                PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(page_number),
                    "The PDF page index exceeded the renderer's supported range.",
                )
            })?;
            let page = document.pages().get(index).map_err(|error| {
                PdfRenderError::new(
                    PdfRenderErrorKind::RenderPage,
                    Some(page_number),
                    error.to_string(),
                )
            })?;
            match page.text() {
                Ok(text) => extracted.push((page_number, text.all())),
                Err(error) => crate::logging::info(
                    "pdfium_native_text_page_unavailable",
                    format!(
                        "page={page_number} detail={}",
                        bounded_detail(&error.to_string(), 300)
                    ),
                ),
            }
        }
        Ok(extracted)
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (source_path, page_numbers);
        Err(PdfRenderError::new(
            PdfRenderErrorKind::Bind,
            None,
            "This mobile build does not yet include the platform PDF native-text adapter.",
        ))
    }
}

'''
if render.count(marker) != 1:
    raise SystemExit("render_pages marker missing")
render = render.replace(marker, helper + marker, 1)
write(render_path, render)

ingestion_path = "src-tauri/src/document_ingestion.rs"
ingestion = read(ingestion_path)
old = '''    validate_pdf_semantic_provenance(&semantic)?;
    let recovery_plan = pdf_backend::plan_text_recovery(&semantic);
    for candidate in &recovery_plan.ocr_pages {
        crate::logging::info(
            "document_ocr_fallback_detected",
            format!(
                "document={document_id} page={} required={} reasons={}",
                candidate.page_number,
                candidate.required,
                candidate.reasons.join(",")
            ),
        );
    }
    for page in &mut semantic.pages {
        page.text = normalize_page_text(&page.text);
    }
'''
new = '''    validate_pdf_semantic_provenance(&semantic)?;
    let mut recovery_plan = pdf_backend::plan_text_recovery(&semantic);

    // OCR is the expensive last resort. If oxidize-pdf marks a page as an OCR
    // candidate, ask the already-bound PDFium engine for that page's native text
    // before rasterizing anything at OCR resolution. Different PDF engines can
    // interpret incomplete ToUnicode/custom-font maps differently; a usable
    // second native extraction is strictly preferable to replacing readable text
    // with Tesseract output.
    let candidate_pages = recovery_plan
        .ocr_pages
        .iter()
        .map(|candidate| candidate.page_number)
        .collect::<Vec<_>>();
    let mut native_rescued_pages = BTreeSet::new();
    if !candidate_pages.is_empty() {
        match pdf_render_backend::extract_native_text_pages(
            render_runtime,
            source_path,
            &candidate_pages,
        ) {
            Ok(alternatives) => {
                for (page_number, alternative) in alternatives {
                    let assessment = pdf_backend::assess_text_quality(&alternative);
                    if alternative.trim().is_empty()
                        || pdf_backend::native_page_requires_ocr(&alternative, &assessment)
                    {
                        continue;
                    }
                    if let Some(page) = semantic
                        .pages
                        .iter_mut()
                        .find(|page| page.page_number == page_number)
                    {
                        page.text = alternative;
                        semantic
                            .native_text_failed_pages
                            .retain(|failed| *failed != page_number);
                        native_rescued_pages.insert(page_number);
                        crate::logging::info(
                            "document_pdfium_native_text_rescued",
                            format!("document={document_id} page={page_number}"),
                        );
                    }
                }
            }
            Err(error) => crate::logging::info(
                "document_pdfium_native_text_rescue_unavailable",
                format!("document={document_id} detail={}", bounded_diagnostic(&error.to_string(), 300)),
            ),
        }
    }
    if !native_rescued_pages.is_empty() {
        recovery_plan = pdf_backend::plan_text_recovery(&semantic);
        let still_ocr = recovery_plan
            .ocr_pages
            .iter()
            .map(|candidate| candidate.page_number)
            .collect::<BTreeSet<_>>();
        let mut fallback = recovery_plan
            .native_semantic_fallback_pages
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        fallback.extend(
            native_rescued_pages
                .iter()
                .copied()
                .filter(|page| !still_ocr.contains(page)),
        );
        recovery_plan.native_semantic_fallback_pages = fallback.into_iter().collect();
    }

    for candidate in &recovery_plan.ocr_pages {
        crate::logging::info(
            "document_ocr_fallback_detected",
            format!(
                "document={document_id} page={} required={} reasons={}",
                candidate.page_number,
                candidate.required,
                candidate.reasons.join(",")
            ),
        );
    }
    for page in &mut semantic.pages {
        page.text = normalize_page_text(&page.text);
    }
'''
if ingestion.count(old) != 1:
    raise SystemExit("ingestion recovery-plan block missing")
ingestion = ingestion.replace(old, new, 1)
ingestion = ingestion.replace(
    "        || ('\\u{e000}'..='\\u{f8ff}').contains(&character)\n",
    "        || matches!(character as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)\n",
    1,
)
write(ingestion_path, ingestion)

pdf_test_path = "src-tauri/src/document_pdf_tests.rs"
pdf_test = read(pdf_test_path)
pdf_test = pdf_test.replace(
    '        "oxidize-pdf-4.2.2-rag-native-first-ocr-v5"',
    '        "oxidize-pdf-4.2.2-rag-native-first-ocr-v6"',
    1,
)
old = '''    let runtime = PdfRenderRuntime::new(None).expect("provisioned PDFium should initialize");
    let mut rendered = 0_usize;
'''
new = '''    let runtime = PdfRenderRuntime::new(None).expect("provisioned PDFium should initialize");
    let pdfium_text = pdf_render_backend::extract_native_text_pages(&runtime, &source, &[1])
        .expect("PDFium should expose native text before OCR is considered");
    assert_eq!(pdfium_text.len(), 1);
    assert_eq!(pdfium_text[0].0, 1);
    assert!(pdfium_text[0].1.chars().filter(|character| character.is_alphabetic()).count() >= 10);

    let mut rendered = 0_usize;
'''
if pdf_test.count(old) != 1:
    raise SystemExit("real PDF fixture runtime marker missing")
pdf_test = pdf_test.replace(old, new, 1)
write(pdf_test_path, pdf_test)

# ---------------------------------------------------------------------------
# Deck-launched practice: the Decks button is an explicit full-deck practice
# action, not a hidden entrance into the daily scheduled-workload limiter.
# Practice landing keeps scheduled/daily-limit semantics.
# Also remember where an external practice launch came from so End session can
# return there, including reopening an exact deck detail view.
# ---------------------------------------------------------------------------
controller_path = "src/controllers/useAppController.ts"
controller = read(controller_path)
old = '''export interface PracticeLaunchRequest {
  requestId: number;
  deckIds: string[];
  tagIds: string[];
  cardIds: string[];
  autoStart: boolean;
}
'''
new = '''export interface PracticeReturnTarget {
  section: Exclude<AppSection, "practice">;
  deckId?: string;
}

export interface DeckOpenRequest {
  requestId: number;
  deckId: string;
}

export interface PracticeLaunchRequest {
  requestId: number;
  deckIds: string[];
  tagIds: string[];
  cardIds: string[];
  autoStart: boolean;
  mode: "scheduled" | "extra";
  returnTarget: PracticeReturnTarget | null;
}
'''
if controller.count(old) != 1:
    raise SystemExit("PracticeLaunchRequest marker missing")
controller = controller.replace(old, new, 1)
old = '''  const [practiceLaunch, setPracticeLaunch] = useState<PracticeLaunchRequest | null>(null);
  const practiceRequestSequence = useRef(1);
  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);
'''
new = '''  const [practiceLaunch, setPracticeLaunch] = useState<PracticeLaunchRequest | null>(null);
  const practiceRequestSequence = useRef(1);
  const [deckOpenRequest, setDeckOpenRequest] = useState<DeckOpenRequest | null>(null);
  const deckOpenRequestSequence = useRef(1);
  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);
'''
if controller.count(old) != 1:
    raise SystemExit("practice launch state marker missing")
controller = controller.replace(old, new, 1)
controller = controller.replace(
    '  const consumePracticeLaunch = useCallback(() => setPracticeLaunch(null), []);\n',
    '  const consumePracticeLaunch = useCallback(() => setPracticeLaunch(null), []);\n  const consumeDeckOpenRequest = useCallback(() => setDeckOpenRequest(null), []);\n',
    1,
)
old = '''  function nextPracticeRequest(
    deckIds: string[],
    tagIds: string[],
    cardIds: string[],
    autoStart: boolean,
  ): void {
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds, tagIds, cardIds, autoStart });
  }
'''
new = '''  function nextPracticeRequest(
    deckIds: string[],
    tagIds: string[],
    cardIds: string[],
    autoStart: boolean,
    mode: "scheduled" | "extra",
    returnTarget: PracticeReturnTarget | null,
  ): void {
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds, tagIds, cardIds, autoStart, mode, returnTarget });
  }
'''
if controller.count(old) != 1:
    raise SystemExit("nextPracticeRequest marker missing")
controller = controller.replace(old, new, 1)
controller = controller.replace(
    '    if (next === "practice") nextPracticeRequest([], [], [], false);\n',
    '    if (next === "practice") nextPracticeRequest([], [], [], false, "scheduled", null);\n',
    1,
)
old = '''  async function practiceDeck(deckId: string): Promise<void> {
    await practiceScope([deckId], []);
  }

  async function practiceScope(deckIds: string[], tagIds: string[]): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    nextPracticeRequest(deckIds, tagIds, [], true);
    setSection("practice");
  }

  async function practiceCards(cardIds: string[]): Promise<void> {
    if (cardIds.length === 0 || !(await canLeaveCurrentSection())) return;
    nextPracticeRequest([], [], cardIds, true);
    setSection("practice");
  }
'''
new = '''  async function practiceDeck(
    deckId: string,
    origin: "library" | "detail" = "library",
  ): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    nextPracticeRequest(
      [deckId],
      [],
      [],
      true,
      "extra",
      origin === "detail" ? { section: "decks", deckId } : { section: "decks" },
    );
    setSection("practice");
  }

  async function practiceScope(deckIds: string[], tagIds: string[]): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    const returnTarget: PracticeReturnTarget | null =
      section === "practice" ? null : { section };
    nextPracticeRequest(deckIds, tagIds, [], true, "scheduled", returnTarget);
    setSection("practice");
  }

  async function practiceCards(cardIds: string[]): Promise<void> {
    if (cardIds.length === 0 || !(await canLeaveCurrentSection())) return;
    const returnTarget: PracticeReturnTarget | null =
      section === "practice" ? null : { section };
    nextPracticeRequest([], [], cardIds, true, "scheduled", returnTarget);
    setSection("practice");
  }

  function returnFromPractice(target: PracticeReturnTarget): void {
    if (target.section === "decks" && target.deckId) {
      const requestId = deckOpenRequestSequence.current;
      deckOpenRequestSequence.current += 1;
      setDeckOpenRequest({ requestId, deckId: target.deckId });
    }
    if (target.section === "decks" || target.section === "cards") {
      void refreshDecks().catch((error: unknown) => showToast("error", getErrorMessage(error)));
    }
    setSection(target.section);
  }
'''
if controller.count(old) != 1:
    raise SystemExit("practice launch functions marker missing")
controller = controller.replace(old, new, 1)
old = '''    practiceLaunch,
    analyticsLaunch,
'''
new = '''    practiceLaunch,
    deckOpenRequest,
    analyticsLaunch,
'''
if controller.count(old) != 1:
    raise SystemExit("controller return state marker missing")
controller = controller.replace(old, new, 1)
old = '''    consumePracticeLaunch,
    consumeAnalyticsLaunch,
'''
new = '''    consumePracticeLaunch,
    consumeDeckOpenRequest,
    consumeAnalyticsLaunch,
'''
if controller.count(old) != 1:
    raise SystemExit("controller consume marker missing")
controller = controller.replace(old, new, 1)
old = '''    practiceCards,
    viewSessionAnalytics,
'''
new = '''    practiceCards,
    returnFromPractice,
    viewSessionAnalytics,
'''
if controller.count(old) != 1:
    raise SystemExit("controller actions marker missing")
controller = controller.replace(old, new, 1)
write(controller_path, controller)

app_path = "src/App.tsx"
app = read(app_path)
app = app.replace(
    '''  AnalyticsLaunchRequest,\n  AppSection,\n  PracticeLaunchRequest,\n''',
    '''  AnalyticsLaunchRequest,\n  AppSection,\n  DeckOpenRequest,\n  PracticeLaunchRequest,\n  PracticeReturnTarget,\n''',
    1,
)
old = '''                onPracticeDeck={controller.practiceDeck}
                onPracticeCards={controller.practiceCards}
                onManageTemplates={() => controller.navigate("templates")}
'''
new = '''                onPracticeDeck={controller.practiceDeck}
                onPracticeCards={controller.practiceCards}
                openDeckRequest={controller.deckOpenRequest}
                onOpenDeckRequestConsumed={controller.consumeDeckOpenRequest}
                onManageTemplates={() => controller.navigate("templates")}
'''
if app.count(old) != 1:
    raise SystemExit("App DecksWorkspace props marker missing")
app = app.replace(old, new, 1)
old = '''                onLaunchConsumed={controller.consumePracticeLaunch}
                onViewAnalytics={controller.viewSessionAnalytics}
                showToast={controller.showToast}
'''
new = '''                onLaunchConsumed={controller.consumePracticeLaunch}
                onViewAnalytics={controller.viewSessionAnalytics}
                onEndEarly={controller.returnFromPractice}
                showToast={controller.showToast}
'''
if app.count(old) != 1:
    raise SystemExit("App PracticeScreen props marker missing")
app = app.replace(old, new, 1)
write(app_path, app)

workspace_path = "src/screens/DecksWorkspace.tsx"
workspace = read(workspace_path)
workspace = workspace.replace(
    'import { useState } from "react";\n\n',
    'import { useState } from "react";\n\nimport type { DeckOpenRequest } from "../App";\n',
    1,
)
workspace = workspace.replace(
    '  onPracticeDeck: (deckId: string) => void | Promise<void>;\n',
    '  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;\n  openDeckRequest: DeckOpenRequest | null;\n  onOpenDeckRequestConsumed: () => void;\n',
    1,
)
workspace = workspace.replace(
    '''  onPracticeDeck,
  onPracticeCards,
  onManageTemplates,
''',
    '''  onPracticeDeck,
  onPracticeCards,
  openDeckRequest,
  onOpenDeckRequestConsumed,
  onManageTemplates,
''',
    1,
)
workspace = workspace.replace(
    '''        onPracticeDeck={onPracticeDeck}
        onManageTemplates={onManageTemplates}
''',
    '''        onPracticeDeck={onPracticeDeck}
        openDeckRequest={openDeckRequest}
        onOpenDeckRequestConsumed={onOpenDeckRequestConsumed}
        onManageTemplates={onManageTemplates}
''',
    1,
)
write(workspace_path, workspace)

decks_path = "src/screens/DecksScreen.tsx"
decks = read(decks_path)
decks = decks.replace(
    'import type { FormEvent } from "react";\n\n',
    'import type { FormEvent } from "react";\n\nimport type { DeckOpenRequest } from "../App";\n',
    1,
)
decks = decks.replace(
    '  onPracticeDeck: (deckId: string) => void | Promise<void>;\n',
    '  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;\n  openDeckRequest?: DeckOpenRequest | null;\n  onOpenDeckRequestConsumed?: () => void;\n',
    1,
)
decks = decks.replace(
    '''  refreshDecks,
  onPracticeDeck,
  onManageTemplates,
''',
    '''  refreshDecks,
  onPracticeDeck,
  openDeckRequest = null,
  onOpenDeckRequestConsumed,
  onManageTemplates,
''',
    1,
)
old = '''  const controller = controllerRef.current;
  const confirm = useConfirm();
'''
new = '''  const controller = controllerRef.current;
  const handledOpenRequestRef = useRef<number | null>(null);
  const confirm = useConfirm();
'''
if decks.count(old) != 1:
    raise SystemExit("Decks controller marker missing")
decks = decks.replace(old, new, 1)
marker = '''  async function refreshDeckListAfterMutation(): Promise<void> {
'''
effect = '''  useEffect(() => {
    if (!openDeckRequest || handledOpenRequestRef.current === openDeckRequest.requestId) return;
    handledOpenRequestRef.current = openDeckRequest.requestId;
    void openDeck(openDeckRequest.deckId).finally(() => onOpenDeckRequestConsumed?.());
  }, [openDeckRequest?.requestId]);

'''
if decks.count(marker) != 1:
    raise SystemExit("Decks open request insertion marker missing")
decks = decks.replace(marker, effect + marker, 1)
decks = decks.replace(
    'onClick={() => void onPracticeDeck(deck.id)}',
    'onClick={() => void onPracticeDeck(deck.id, "library")}',
    1,
)
decks = decks.replace(
    'onClick={() => void onPracticeDeck(selectedDeck.id)}',
    'onClick={() => void onPracticeDeck(selectedDeck.id, "detail")}',
    1,
)
old = '''function practiceTitle(deck: DeckSummary): string {
  if (deck.archivedAt !== null) return "Unarchive this deck before practicing";
  if (deck.generatedCardCount === 0) return "Add a flashcard before practicing this deck";
  if (deck.studyableDueCount === 0) return "Start an extra session; no cards are ready now";
  return `${deck.studyableDueCount} ${
    deck.studyableDueCount === 1 ? "card is" : "cards are"
  } ready now`;
}
'''
new = '''function practiceTitle(deck: DeckSummary): string {
  if (deck.archivedAt !== null) return "Unarchive this deck before practicing";
  if (deck.generatedCardCount === 0) return "Add a flashcard before practicing this deck";
  return `Practice all ${deck.generatedCardCount} ${
    deck.generatedCardCount === 1 ? "card" : "cards"
  } in this deck`;
}
'''
if decks.count(old) != 1:
    raise SystemExit("practiceTitle block missing")
decks = decks.replace(old, new, 1)
write(decks_path, decks)

practice_path = "src/screens/PracticeScreen.tsx"
practice = read(practice_path)
practice = practice.replace(
    'import type { PracticeLaunchRequest } from "../App";\n',
    'import type { PracticeLaunchRequest, PracticeReturnTarget } from "../App";\n',
    1,
)
practice = practice.replace(
    '  onViewAnalytics: (sessionId: string, deckIds: string[]) => void;\n',
    '  onViewAnalytics: (sessionId: string, deckIds: string[]) => void;\n  onEndEarly: (target: PracticeReturnTarget) => void;\n',
    1,
)
practice = practice.replace(
    '''  onLaunchConsumed,
  onViewAnalytics,
  showToast,
''',
    '''  onLaunchConsumed,
  onViewAnalytics,
  onEndEarly,
  showToast,
''',
    1,
)
old = '''  const [recentDecks, setRecentDecks] = useState<RecentDeckSummary[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
'''
new = '''  const [recentDecks, setRecentDecks] = useState<RecentDeckSummary[]>([]);
  const [returnTarget, setReturnTarget] = useState<PracticeReturnTarget | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
'''
if practice.count(old) != 1:
    raise SystemExit("Practice state marker missing")
practice = practice.replace(old, new, 1)
old = '''    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
'''
new = '''    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setReturnTarget(launchRequest.returnTarget);
    setSession(null);
'''
if practice.count(old) != 1:
    raise SystemExit("Practice launch effect marker missing")
practice = practice.replace(old, new, 1)
practice = practice.replace(
    '''        settings.practiceShuffle,
        "scheduled",
      );
''',
    '''        settings.practiceShuffle,
        launchRequest.mode,
      );
''',
    1,
)
old = '''    try {
      await controller.complete(session.id);
      resetToSetup();
    } catch (error: unknown) {
'''
new = '''    try {
      await controller.complete(session.id);
      if (returnTarget) {
        onEndEarly(returnTarget);
        return;
      }
      resetToSetup();
    } catch (error: unknown) {
'''
if practice.count(old) != 1:
    raise SystemExit("endSession block missing")
practice = practice.replace(old, new, 1)
old = '''    setTemporaryCardIds([]);
    setEditingCard(null);
    setSelectedTagIds(new Set());
'''
new = '''    setTemporaryCardIds([]);
    setEditingCard(null);
    setReturnTarget(null);
    setSelectedTagIds(new Set());
'''
if practice.count(old) != 1:
    raise SystemExit("resetToSetup marker missing")
practice = practice.replace(old, new, 1)
write(practice_path, practice)

# ---------------------------------------------------------------------------
# AI settings: Default task profile was literally the same persisted task
# default as the top Default AI model control. Remove the duplicate block.
# ---------------------------------------------------------------------------
ai_path = "src/screens/settings/AiLlmSettings.tsx"
ai = read(ai_path)
duplicate = '''
        <section className={styles.settingsCard}>
          <h2>Default task profile</h2>
          <label className={styles.stackedField}>
            <span>Flashcard generation</span>
            <select
              value={controller.defaults.flashcardGenerationProfileId ?? ""}
              disabled={controller.mutating}
              onChange={(event) =>
                void controller.setFlashcardDefault(event.currentTarget.value || null)
              }
            >
              <option value="">Choose each time</option>
              {controller.profiles
                .filter((profile) => profile.configurationStatus !== "planned")
                .map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.displayName}
                    {profile.configurationStatus === "ready" ? "" : " — incomplete"}
                  </option>
                ))}
            </select>
          </label>
          <p className={styles.securityNote}>
            There is no automatic fallback. A failed local profile will never silently send study material to a cloud provider.
          </p>
        </section>
'''
if ai.count(duplicate) != 1:
    raise SystemExit("duplicate Default task profile block missing")
ai = ai.replace(duplicate, "", 1)
write(ai_path, ai)

# ---------------------------------------------------------------------------
# Frontend regressions.
# ---------------------------------------------------------------------------
decks_test_path = "src/screens/DecksScreen.test.tsx"
decks_test = read(decks_test_path)
decks_test = decks_test.replace(
    '    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id);\n',
    '    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id, "detail");\n',
    1,
)
write(decks_test_path, decks_test)

settings_test_path = "src/screens/SettingsScreen.test.tsx"
settings_test = read(settings_test_path)
needle = '''    expect(screen.queryByRole("option", { name: /planned/u })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
'''
replacement = '''    expect(screen.queryByRole("option", { name: /planned/u })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Default task profile" })).toBeNull();
    expect(screen.getAllByRole("combobox", { name: "Flashcard generation" })).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
'''
if settings_test.count(needle) != 1:
    raise SystemExit("Settings AI test marker missing")
settings_test = settings_test.replace(needle, replacement, 1)
write(settings_test_path, settings_test)

practice_test_path = "src/screens/PracticeScreen.test.tsx"
practice_test = read(practice_test_path)
practice_test = practice_test.replace(
    '''    onLaunchConsumed: vi.fn(),
    onViewAnalytics: vi.fn(),
    showToast: vi.fn(),
''',
    '''    onLaunchConsumed: vi.fn(),
    onViewAnalytics: vi.fn(),
    onEndEarly: vi.fn(),
    showToast: vi.fn(),
''',
    1,
)
# Existing custom-card launch request gets the new required launch metadata.
practice_test = practice_test.replace(
    '''        cardIds: [card.cardId],
        autoStart: true,
      },
''',
    '''        cardIds: [card.cardId],
        autoStart: true,
        mode: "scheduled",
        returnTarget: { section: "cards" },
      },
''',
    1,
)
marker = '''  it("reuses one practice-start operation after an ambiguous transport failure", async () => {
'''
new_tests = '''  it("auto-starts a deck-origin launch as explicit extra practice instead of the daily limit", async () => {
    renderPractice({
      launchRequest: {
        requestId: 9,
        deckIds: [deck.id],
        tagIds: [],
        cardIds: [],
        autoStart: true,
        mode: "extra",
        returnTarget: { section: "decks", deckId: deck.id },
      },
    });
    await waitFor(() =>
      expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [deck.id],
        tagIds: [],
        shuffle: true,
        mode: "extra",
      }),
    );
  });

  it("returns to an external launch origin when End session terminates early", async () => {
    const onEndEarly = vi.fn();
    const returnTarget = { section: "decks" as const, deckId: deck.id };
    renderPractice({
      launchRequest: {
        requestId: 10,
        deckIds: [deck.id],
        tagIds: [],
        cardIds: [],
        autoStart: true,
        mode: "extra",
        returnTarget,
      },
      onEndEarly,
    });
    fireEvent.click(await screen.findByRole("button", { name: "End session" }));
    await waitFor(() => {
      expect(practiceIpc.completePracticeSession).toHaveBeenCalledWith(session.id);
      expect(onEndEarly).toHaveBeenCalledWith(returnTarget);
    });
  });

'''
if practice_test.count(marker) != 1:
    raise SystemExit("Practice test insertion marker missing")
practice_test = practice_test.replace(marker, new_tests + marker, 1)
write(practice_test_path, practice_test)

app_test_path = "src/App.test.tsx"
app_test = read(app_test_path)
app_test = app_test.replace(
    'import type { AnalyticsLaunchRequest, PracticeLaunchRequest } from "./App";\n',
    'import type { AnalyticsLaunchRequest, DeckOpenRequest, PracticeLaunchRequest, PracticeReturnTarget } from "./App";\n',
    1,
)
old = '''    onPracticeDeck,
    onDirtyChange,
  }: {
    decks: decksIpc.DeckSummary[];
    onPracticeDeck: (deckId: string) => void;
    onDirtyChange: (dirty: boolean) => void;
  }) => (
'''
new = '''    onPracticeDeck,
    openDeckRequest,
    onDirtyChange,
  }: {
    decks: decksIpc.DeckSummary[];
    onPracticeDeck: (deckId: string, origin: "library" | "detail") => void;
    openDeckRequest: DeckOpenRequest | null;
    onDirtyChange: (dirty: boolean) => void;
  }) => (
'''
if app_test.count(old) != 1:
    raise SystemExit("App DecksWorkspace mock signature missing")
app_test = app_test.replace(old, new, 1)
app_test = app_test.replace(
    '''      <span>{decks.map((deck) => deck.name).join(",")}</span>
      <button type="button" onClick={() => onDirtyChange(true)}>
''',
    '''      <span>{decks.map((deck) => deck.name).join(",")}</span>
      <output aria-label="Deck open request">{openDeckRequest?.deckId ?? "none"}</output>
      <button type="button" onClick={() => onDirtyChange(true)}>
''',
    1,
)
app_test = app_test.replace(
    '      <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "")}>\n',
    '      <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "", "detail")}>\n',
    1,
)
old = '''    launchRequest,
    onViewAnalytics,
  }: {
    launchRequest: PracticeLaunchRequest | null;
    onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
  }) => (
'''
new = '''    launchRequest,
    onViewAnalytics,
    onEndEarly,
  }: {
    launchRequest: PracticeLaunchRequest | null;
    onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
    onEndEarly: (target: PracticeReturnTarget) => void;
  }) => (
'''
if app_test.count(old) != 1:
    raise SystemExit("App PracticeScreen mock signature missing")
app_test = app_test.replace(old, new, 1)
old = '''          ? `${launchRequest.deckIds.join(",")}|${launchRequest.cardIds.join(",")}|${String(
              launchRequest.autoStart,
            )}`
'''
new = '''          ? `${launchRequest.deckIds.join(",")}|${launchRequest.cardIds.join(",")}|${String(
              launchRequest.autoStart,
            )}|${launchRequest.mode}|${launchRequest.returnTarget?.section ?? "none"}|${
              launchRequest.returnTarget?.deckId ?? ""
            }`
'''
if app_test.count(old) != 1:
    raise SystemExit("App practice launch output marker missing")
app_test = app_test.replace(old, new, 1)
app_test = app_test.replace(
    '''      <button
        type="button"
        onClick={() => onViewAnalytics("session-1", ["deck-from-practice"])}
      >
        Open session analytics
      </button>
''',
    '''      <button
        type="button"
        onClick={() => onViewAnalytics("session-1", ["deck-from-practice"])}
      >
        Open session analytics
      </button>
      <button
        type="button"
        onClick={() => {
          if (launchRequest?.returnTarget) onEndEarly(launchRequest.returnTarget);
        }}
      >
        End early mock
      </button>
''',
    1,
)
app_test = app_test.replace(
    '    expect(screen.getByLabelText("Practice launch").textContent).toBe(`${deck.id}||true`);\n',
    '    expect(screen.getByLabelText("Practice launch").textContent).toBe(`${deck.id}||true|extra|decks|${deck.id}`);\n',
    1,
)
marker = '''  it("carries a completed practice session into week-scoped analytics", async () => {
'''
new_test = '''  it("returns an early-ended deck-detail practice session to that exact deck", async () => {
    const deck = makeDeck({ name: "Respiratory physiology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    await screen.findByRole("heading", { name: "Practice mock" });
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Deck open request").textContent).toBe(deck.id);
  });

'''
if app_test.count(marker) != 1:
    raise SystemExit("App test insertion marker missing")
app_test = app_test.replace(marker, new_test + marker, 1)
write(app_test_path, app_test)

# ---------------------------------------------------------------------------
# Architecture documentation.
# ---------------------------------------------------------------------------
doc_path = "docs/architecture/ai-and-document-processing.md"
doc = read(doc_path)
old = '''Good raw page text with bad semantic structure uses page-text semantic fallback instead of OCR. The quality system deliberately has two levels: localized malformed glyph mappings invalidate structured semantics and are removed from the native page-text fallback, while only page-wide unusability is allowed to invoke OCR. **Readable native text is never replaced merely because it is short, image-heavy, table-heavy, or contains a minority of unmapped scientific-symbol/font glyphs.** This prevents image-heavy textbook pages from crossing an OCR threshold simply because they contain less prose. OCR is reserved for native extraction failures, genuinely empty native text where recognition is needed, whole-page substitution/obfuscation, or corruption that dominates the visible text layer. Blank/image-only pages may remain visual evidence when no text is recoverable. Recovered text passes the same quality boundary again; ostudy does not hard-code a substitution alphabet for one PDF/font.
'''
new = '''Good raw page text with bad semantic structure uses page-text semantic fallback instead of OCR. The quality system deliberately has two levels: localized malformed glyph mappings invalidate structured semantics and are removed from the native page-text fallback, while only page-wide unusability may become an OCR candidate. **Readable native text is never replaced merely because it is short, image-heavy, table-heavy, or contains a minority of unmapped scientific-symbol/font glyphs.** Before any candidate is rasterized for Tesseract, ostudy asks PDFium for a second, independent native-text extraction of that page. If PDFium produces usable text, that native text is cleaned and used for semantic fallback instead. OCR is therefore reserved for pages where the primary native extraction and the PDFium native-text rescue both fail to provide usable text, including genuine image-only pages, extraction failures, or whole-page obfuscation/corruption. Blank/image-only pages may remain visual evidence when no text is recoverable. Recovered text passes the same quality boundary again; ostudy does not hard-code a substitution alphabet for one PDF/font.
'''
if doc.count(old) != 1:
    raise SystemExit("AI architecture recovery paragraph missing")
doc = doc.replace(old, new, 1)
write(doc_path, doc)

overview_path = "docs/architecture/overview.md"
overview = read(overview_path)
needle = '''Document processing is divided into narrow capabilities rather than one PDF-specific monolith. Parser, renderer, OCR, provider, and authoring types do not cross each other's boundaries. A model receives bounded context and returns strict task data; native validation and the existing authoring transaction remain authoritative.
'''
replacement = '''Document processing is divided into narrow capabilities rather than one PDF-specific monolith. Parser, renderer, OCR, provider, and authoring types do not cross each other's boundaries. Candidate OCR pages receive an engine-diverse native-text rescue through PDFium before raster OCR is allowed, so a damaged font map in one parser cannot by itself trigger Tesseract. A model receives bounded context and returns strict task data; native validation and the existing authoring transaction remain authoritative.
'''
if overview.count(needle) != 1:
    raise SystemExit("overview document processing paragraph missing")
overview = overview.replace(needle, replacement, 1)
write(overview_path, overview)

print("PR41 follow-up v7 changes applied")
