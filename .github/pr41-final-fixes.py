from pathlib import Path
import re


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def replace_regex(path: str, pattern: str, replacement: str, flags=0) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one regex match, found {count}: {pattern!r}")
    write(path, updated)


# ---------------------------------------------------------------------------
# PDF extraction: keep readable native text even when custom symbol fonts expose
# isolated C0 control codes. This exact pattern occurs in ordinary textbook PDFs
# where glyphs such as micro, approximate, less-than, or alpha are encoded by a
# custom Type1C font without a complete ToUnicode map. OCR is reserved for dense
# corruption, extraction failure, or genuinely empty native text.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/pdf_backend.rs",
    '    "oxidize-pdf-4.2.2-rag-native-first-ocr-v3"\n',
    '    "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\n',
)
replace_once(
    "src-tauri/src/pdf_backend.rs",
    '''    if invalid_controls > 0 {
        reasons.push("invalid_control_characters");
    }
''',
    '''    // Custom embedded PDF fonts sometimes map isolated scientific-symbol
    // glyphs to C0 control codes when no complete ToUnicode map is available.
    // A few such glyphs inside otherwise readable native text are lossy, but
    // they are not evidence that the whole page should be replaced by OCR.
    // Treat controls as corruption only when they are both numerous and dense.
    if invalid_controls >= 8
        && invalid_controls.saturating_mul(100) >= nonspace.max(1).saturating_mul(5)
    {
        reasons.push("invalid_control_characters");
    }
''',
)
replace_once(
    "src-tauri/src/pdf_backend.rs",
    '            "oxidize-pdf-4.2.2-rag-native-first-ocr-v3"\n',
    '            "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\n',
)
replace_once(
    "src-tauri/src/pdf_backend.rs",
    '''    #[test]
    fn isolated_replacement_glyph_does_not_force_ocr_on_readable_native_text() {
''',
    '''    #[test]
    fn sparse_custom_font_control_glyphs_do_not_force_ocr() {
        let assessment = assess_text_quality(
            "The active blood-gas barrier is around 0.3 \\u{001e}m thick and the capillary is under 3 \\u{001e}m in scale.",
        );
        assert!(!assessment.suspicious);

        let document = base_document(
            "The active blood-gas barrier is around 0.3 \\u{001e}m thick.",
            "",
        );
        let plan = plan_text_recovery(&document);
        assert!(plan.ocr_pages.is_empty());
        assert_eq!(plan.native_semantic_fallback_pages, Vec::<usize>::new());
    }

    #[test]
    fn dense_control_character_corruption_still_requires_recovery() {
        let corrupt = "A\\u{0001}B\\u{0002}C\\u{0003}D\\u{0004}E\\u{0005}F\\u{0006}G\\u{0007}H\\u{0008}I\\u{0009}J\\u{000b}K\\u{000c}L\\u{000e}M\\u{000f}N";
        let assessment = assess_text_quality(corrupt);
        assert!(assessment.suspicious);
        assert!(assessment.reasons.contains(&"invalid_control_characters"));
    }

    #[test]
    fn isolated_replacement_glyph_does_not_force_ocr_on_readable_native_text() {
''',
)
replace_once(
    "src-tauri/src/document_pdf_tests.rs",
    '        "oxidize-pdf-4.2.2-rag-quality-ocr-v2"\n',
    '        "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\n',
)

# ---------------------------------------------------------------------------
# AI profile UI: one creation affordance, provider choice inside the connection
# form, and remove implementation-detail prose from everyday settings.
# ---------------------------------------------------------------------------
replace_once(
    "src/screens/settings/AiLlmSettings.tsx",
    '''            <h2>Provider profiles</h2>
''',
    '''            <h2>AI provider profiles</h2>
''',
)
replace_regex(
    "src/screens/settings/AiLlmSettings.tsx",
    r'''        <div className=\{styles\.addProviderGrid\}>\n          \{availableProviders\.map\(\(provider\) => \(\n            <button[\s\S]*?          \)\)\}\n        </div>''',
    '''        <div className={styles.addProviderGrid}>
          <button
            type="button"
            className="secondary-button"
            disabled={controller.mutating || availableProviders.length === 0}
            onClick={() => {
              const preferred =
                availableProviders.find((provider) => provider.providerKind === "llama_cpp") ??
                availableProviders[0];
              if (preferred) void beginCreate(preferred.providerKind);
            }}
          >
            New profile
          </button>
        </div>''',
)
replace_once(
    "src/screens/settings/AiLlmSettings.tsx",
    '''            <ProfileEditor
              controller={controller}
              providerSettingsDirty={providerSettingsDirty}
              onDelete={() => void deleteProfile()}
            />
''',
    '''            <ProfileEditor
              controller={controller}
              providerSettingsDirty={providerSettingsDirty}
              onProviderChange={(providerKind) => void beginCreate(providerKind)}
              onDelete={() => void deleteProfile()}
            />
''',
)
replace_once(
    "src/screens/settings/AiLlmSettings.tsx",
    '''function ProfileEditor({
  controller,
  providerSettingsDirty,
  onDelete,
}: {
  controller: AiProfilesController;
  providerSettingsDirty: boolean;
  onDelete: () => void;
}) {
''',
    '''function ProfileEditor({
  controller,
  providerSettingsDirty,
  onProviderChange,
  onDelete,
}: {
  controller: AiProfilesController;
  providerSettingsDirty: boolean;
  onProviderChange: (providerKind: AiProviderKind) => void;
  onDelete: () => void;
}) {
''',
)
replace_once(
    "src/screens/settings/AiLlmSettings.tsx",
    '''        <label className={styles.stackedField}>
          <span>Provider</span>
          <input value={provider?.displayName ?? draft.providerKind} readOnly />
        </label>
''',
    '''        <label className={styles.stackedField}>
          <span>Provider</span>
          <select
            aria-label="Provider type"
            value={draft.providerKind}
            disabled={saved !== null || controller.mutating}
            onChange={(event) => onProviderChange(event.currentTarget.value as AiProviderKind)}
          >
            {controller.catalog.map((entry) => (
              <option
                key={entry.providerKind}
                value={entry.providerKind}
                disabled={entry.implementationStatus !== "available"}
              >
                {entry.displayName}
                {entry.implementationStatus === "available" ? "" : " — planned"}
              </option>
            ))}
          </select>
        </label>
''',
)
replace_regex(
    "src/screens/settings/AiLlmSettings.tsx",
    r'''\n      \{isLlama \? \(\n        <p className=\{styles\.securityNote\}>\n          llama\.cpp applies the chat template embedded with the loaded model\.[\s\S]*?      \) : \(\n        <p className=\{styles\.cloudNotice\}>\n          DeepSeek is a cloud provider\. ostudy uses DeepSeek's fixed official API endpoint; the credential is entered only after this profile has a saved identity\.\n        </p>\n      \)\}\n''',
    '''
      {!isLlama ? (
        <p className={styles.cloudNotice}>
          DeepSeek is a cloud provider. ostudy uses DeepSeek's fixed official API endpoint.
        </p>
      ) : null}
''',
)
replace_regex(
    "src/screens/settings/AiLlmSettings.tsx",
    r'''\n      <p className=\{styles\.securityNote\}>\n        The credential never enters SQLite,[\s\S]*?      </p>''',
    '',
)
replace_regex(
    "src/screens/settings/AiLlmSettings.tsx",
    r'''\n          \{profile\.providerKind === "llama_cpp" \? \(\n            <p className=\{styles\.note\}>\n              llama\.cpp normally reports the loaded GGUF file path[\s\S]*?            </p>\n          \) : null\}''',
    '',
)

# ---------------------------------------------------------------------------
# Practice: prevent the same first card from recurring across consecutive
# equivalent shuffled sessions, while never crossing FSRS priority boundaries.
# Also add a native refresh boundary so editing a card during practice updates
# the current session snapshot and removes a now-deleted reverse variant.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/practice_selection.rs",
    '''pub struct PracticeCardWindowInput {
    pub session_id: String,
    pub start_position: i64,
    pub limit: i64,
}
''',
    '''pub struct PracticeCardWindowInput {
    pub session_id: String,
    pub start_position: i64,
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RefreshPracticeAfterCardEditInput {
    pub session_id: String,
    pub note_id: String,
    pub start_position: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeEditRefreshDto {
    pub card_count: i64,
    pub start_position: i64,
    pub completed: bool,
    pub window: PracticeCardWindowDto,
}
''',
)
replace_once(
    "src-tauri/src/practice_selection.rs",
    '''#[tauri::command]
pub fn get_practice_card_window(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PracticeCardWindowInput,
) -> Result<PracticeCardWindowDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    database.with_connection(|connection| {
        load_practice_window(
            connection,
            &paths,
            &input.session_id,
            input.start_position,
            input.limit,
        )
    })
}
''',
    '''#[tauri::command]
pub fn get_practice_card_window(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PracticeCardWindowInput,
) -> Result<PracticeCardWindowDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    database.with_connection(|connection| {
        load_practice_window(
            connection,
            &paths,
            &input.session_id,
            input.start_position,
            input.limit,
        )
    })
}

#[tauri::command]
pub fn refresh_practice_after_card_edit(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: RefreshPracticeAfterCardEditInput,
) -> Result<PracticeEditRefreshDto, CommandError> {
    validate_entity_id(&input.session_id, "practice session")?;
    validate_entity_id(&input.note_id, "note")?;
    if input.start_position < 0 {
        return Err(CommandError::new(
            "invalid_practice_position",
            "The current practice position is invalid.",
        ));
    }
    database.with_connection(|connection| {
        refresh_practice_after_card_edit_in_connection(connection, &paths, input)
    })
}
''',
)
replace_once(
    "src-tauri/src/practice_selection.rs",
    '''    let session_id = generate_id(connection)?;
    let mut prepared = prepare_new_session_cards(connection, &planned, time)?;
    if shuffle {
        prepared.sort_by_key(|card| {
            let due_date = time
                .local_date_for_timestamp(card.due_at)
                .unwrap_or(time.local_date);
            (
                due_date,
                !card.needs_more_practice,
                card.state == 0,
                shuffle_key(&session_id, &card.card_id),
            )
        });
    }
''',
    '''    let previous_first = if shuffle {
        previous_first_card_for_request(connection, &request_fingerprint)?
    } else {
        None
    };
    let session_id = generate_id(connection)?;
    let mut prepared = prepare_new_session_cards(connection, &planned, time)?;
    if shuffle {
        prepared.sort_by_key(|card| {
            let due_date = time
                .local_date_for_timestamp(card.due_at)
                .unwrap_or(time.local_date);
            (
                due_date,
                !card.needs_more_practice,
                card.state == 0,
                shuffle_key(&session_id, &card.card_id),
            )
        });
        avoid_repeating_previous_first(&mut prepared, previous_first.as_deref(), time);
    }
''',
)
replace_once(
    "src-tauri/src/practice_selection.rs",
    '''fn first_window_from_prepared(
''',
    '''fn previous_first_card_for_request(
    connection: &Connection,
    request_fingerprint: &str,
) -> Result<Option<String>, CommandError> {
    connection
        .query_row(
            "SELECT psc.card_id
             FROM practice_sessions ps
             JOIN practice_session_cards psc
               ON psc.session_id = ps.id AND psc.position = 0
             WHERE ps.start_request_fingerprint = ?1
             ORDER BY ps.started_at DESC, ps.id DESC
             LIMIT 1",
            [request_fingerprint],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

fn avoid_repeating_previous_first(
    prepared: &mut [PreparedPracticeCard],
    previous_first: Option<&str>,
    time: &StudyTimeContext,
) {
    let Some(previous_first) = previous_first else {
        return;
    };
    let Some(first) = prepared.first() else {
        return;
    };
    if first.card_id != previous_first {
        return;
    }
    let due_date = time
        .local_date_for_timestamp(first.due_at)
        .unwrap_or(time.local_date);
    let weak = first.needs_more_practice;
    let is_new = first.state == 0;
    if let Some(index) = prepared.iter().enumerate().skip(1).find_map(|(index, card)| {
        let candidate_date = time
            .local_date_for_timestamp(card.due_at)
            .unwrap_or(time.local_date);
        (candidate_date == due_date
            && card.needs_more_practice == weak
            && (card.state == 0) == is_new)
            .then_some(index)
    }) {
        prepared.swap(0, index);
    }
}

fn refresh_practice_after_card_edit_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: RefreshPracticeAfterCardEditInput,
) -> Result<PracticeEditRefreshDto, CommandError> {
    let session_open: Option<i64> = connection
        .query_row(
            "SELECT card_count FROM practice_sessions
             WHERE id = ?1 AND completed_at IS NULL",
            [input.session_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    if session_open.is_none() {
        return Err(CommandError::new(
            "practice_session_not_found",
            "That active practice session no longer exists.",
        ));
    }

    let existing_rows = {
        let mut statement = connection
            .prepare(
                "SELECT psc.card_id
                 FROM practice_session_cards psc
                 JOIN practice_session_card_snapshots snap
                   ON snap.session_id = psc.session_id AND snap.card_id = psc.card_id
                 WHERE psc.session_id = ?1 AND snap.note_id = ?2 AND psc.rated_at IS NULL
                 ORDER BY psc.position",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map(params![input.session_id, input.note_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    if existing_rows.is_empty() {
        return Err(CommandError::new(
            "practice_card_already_completed",
            "That card is no longer an unreviewed item in this practice session.",
        ));
    }

    let time = StudyTimeContext::capture()?;
    let active_planned = {
        let mut statement = connection
            .prepare(
                "SELECT c.deck_id, c.id
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 JOIN decks d ON d.id = c.deck_id
                 WHERE c.note_id = ?1
                   AND c.deleted_at IS NULL
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND d.archived_at IS NULL",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([input.note_id.as_str()], |row| {
                Ok(PlannedCard {
                    deck_id: row.get(0)?,
                    card_id: row.get(1)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let prepared = prepare_new_session_cards(connection, &active_planned, &time)?;
    let prepared_by_id = prepared
        .into_iter()
        .map(|card| (card.card_id.clone(), card))
        .collect::<BTreeMap<_, _>>();

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    for card_id in &existing_rows {
        transaction
            .execute(
                "DELETE FROM practice_session_card_tag_snapshots
                 WHERE session_id = ?1 AND card_id = ?2",
                params![input.session_id, card_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM practice_session_card_snapshots
                 WHERE session_id = ?1 AND card_id = ?2",
                params![input.session_id, card_id],
            )
            .map_err(CommandError::database)?;
        if let Some(card) = prepared_by_id.get(card_id) {
            insert_session_snapshot(&transaction, &input.session_id, card, time.now_utc_millis)?;
        } else {
            transaction
                .execute(
                    "DELETE FROM practice_session_cards
                     WHERE session_id = ?1 AND card_id = ?2 AND rated_at IS NULL",
                    params![input.session_id, card_id],
                )
                .map_err(CommandError::database)?;
        }
    }

    let remaining = {
        let mut statement = transaction
            .prepare(
                "SELECT card_id FROM practice_session_cards
                 WHERE session_id = ?1 ORDER BY position",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([input.session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    transaction
        .execute(
            "UPDATE practice_session_cards SET position = position + ?1 WHERE session_id = ?2",
            params![MAX_SESSION_CARDS as i64 + 1, input.session_id],
        )
        .map_err(CommandError::database)?;
    for (position, card_id) in remaining.iter().enumerate() {
        transaction
            .execute(
                "UPDATE practice_session_cards SET position = ?1
                 WHERE session_id = ?2 AND card_id = ?3",
                params![
                    i64::try_from(position).unwrap_or(i64::MAX),
                    input.session_id,
                    card_id,
                ],
            )
            .map_err(CommandError::database)?;
    }
    let card_count = i64::try_from(remaining.len()).unwrap_or(i64::MAX);
    let rated_count: i64 = transaction
        .query_row(
            "SELECT rated_count FROM practice_sessions WHERE id = ?1",
            [input.session_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let completed = card_count == 0 || rated_count >= card_count;
    transaction
        .execute(
            "UPDATE practice_sessions
             SET card_count = ?1,
                 completed_at = CASE WHEN ?2 = 1 THEN COALESCE(completed_at, ?3) ELSE completed_at END
             WHERE id = ?4",
            params![
                card_count,
                i64::from(completed),
                time.now_utc_millis,
                input.session_id,
            ],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;

    if completed {
        return Ok(PracticeEditRefreshDto {
            card_count,
            start_position: input.start_position.min(card_count.max(0)),
            completed: true,
            window: PracticeCardWindowDto {
                start_position: input.start_position.min(card_count.max(0)),
                cards: Vec::new(),
                has_more: false,
            },
        });
    }
    let start_position = input.start_position.min(card_count.saturating_sub(1));
    let window = load_practice_window(
        connection,
        paths,
        &input.session_id,
        start_position,
        PRACTICE_WINDOW_MAX,
    )?;
    Ok(PracticeEditRefreshDto {
        card_count,
        start_position,
        completed: false,
        window,
    })
}

fn first_window_from_prepared(
''',
)
replace_once(
    "src-tauri/src/lib.rs",
    '''            practice_selection::get_practice_card_window,
''',
    '''            practice_selection::get_practice_card_window,
            practice_selection::refresh_practice_after_card_edit,
''',
)

# IPC/controller contract for the refresh.
replace_once(
    "src/ipc/practice.ts",
    '''export interface PracticeCardWindowInput {
  sessionId: string;
  startPosition: number;
  limit: number;
}
''',
    '''export interface PracticeCardWindowInput {
  sessionId: string;
  startPosition: number;
  limit: number;
}

export interface RefreshPracticeAfterCardEditInput {
  sessionId: string;
  noteId: string;
  startPosition: number;
}

export interface PracticeEditRefresh {
  cardCount: number;
  startPosition: number;
  completed: boolean;
  window: PracticeCardWindow;
}
''',
)
replace_once(
    "src/ipc/practice.ts",
    '''export async function getPracticeCardWindow(
  input: PracticeCardWindowInput,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", { input });
}
''',
    '''export async function getPracticeCardWindow(
  input: PracticeCardWindowInput,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", { input });
}

export async function refreshPracticeAfterCardEdit(
  input: RefreshPracticeAfterCardEditInput,
): Promise<PracticeEditRefresh> {
  return invoke<PracticeEditRefresh>("refresh_practice_after_card_edit", { input });
}
''',
)
replace_once(
    "src/controllers/practiceController.ts",
    '''  markPracticeCardViewed,
''',
    '''  markPracticeCardViewed,
  refreshPracticeAfterCardEdit,
''',
)
replace_once(
    "src/controllers/practiceController.ts",
    '''  type PracticeSession,
''',
    '''  type PracticeEditRefresh,
  type PracticeSession,
''',
)
replace_once(
    "src/controllers/practiceController.ts",
    '''  type ReviewPreviewSet,
''',
    '''  type RefreshPracticeAfterCardEditInput,
  type ReviewPreviewSet,
''',
)
replace_once(
    "src/controllers/practiceController.ts",
    '''  window(input: PracticeCardWindowInput): Promise<PracticeCardWindow | undefined> {
    return latestResult(this.#window, () => getPracticeCardWindow(input));
  }
''',
    '''  window(input: PracticeCardWindowInput): Promise<PracticeCardWindow | undefined> {
    return latestResult(this.#window, () => getPracticeCardWindow(input));
  }

  refreshAfterEdit(input: RefreshPracticeAfterCardEditInput): Promise<PracticeEditRefresh> {
    return refreshPracticeAfterCardEdit(input);
  }
''',
)

# Practice UI: reuse the canonical card editor, then refresh the durable session.
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''import type { PracticeLaunchRequest } from "../App";
''',
    '''import type { PracticeLaunchRequest } from "../App";
import { getCardNote, type CardNote } from "../ipc/cards";
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''import { ActivePractice } from "./practice/ActivePractice";
''',
    '''import { CardEditorScreen } from "./CardEditorScreen";
import { ActivePractice } from "./practice/ActivePractice";
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''  const [isManaging, setIsManaging] = useState(false);
''',
    '''  const [isManaging, setIsManaging] = useState(false);
  const [isLoadingEditor, setIsLoadingEditor] = useState(false);
  const [editingCard, setEditingCard] = useState<CardNote | null>(null);
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''  async function manageCurrent(
''',
    '''  async function editCurrentCard(): Promise<void> {
    if (!session || !current || isLoadingEditor || pendingReviewRef.current) return;
    setIsLoadingEditor(true);
    setErrorMessage(null);
    try {
      setEditingCard(await getCardNote(current.presentation.noteId));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingEditor(false);
    }
  }

  async function finishCardEdit(saved: CardNote): Promise<void> {
    if (!session) return;
    const refreshed = await controller.refreshAfterEdit({
      sessionId: session.id,
      noteId: saved.noteId,
      startPosition: position,
    });
    setSession((currentSession) =>
      currentSession ? { ...currentSession, cardCount: refreshed.cardCount } : currentSession,
    );
    setEditingCard(null);
    setPosition(refreshed.startPosition);
    setSessionCards((cards) => [
      ...cards.slice(0, refreshed.startPosition),
      ...refreshed.window.cards,
    ]);
    setRevealed(false);
    setPreviewSet(null);
    questionShownAtRef.current = Date.now();
    if (refreshed.completed) {
      setCompleted(true);
      experience.emit({ type: "session_completed" });
    }
    showToast("success", "Card updated. Practice session refreshed.");
  }

  async function manageCurrent(
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''  if (session && completed) {
''',
    '''  if (session && editingCard) {
    const editorDeck = activeDecks.find((deck) => deck.id === editingCard.deckId);
    if (editorDeck) {
      return (
        <CardEditorScreen
          deck={editorDeck}
          decks={[editorDeck]}
          card={editingCard}
          onCancel={() => setEditingCard(null)}
          onSaved={async (saved) => finishCardEdit(saved)}
          onTemplateGenerated={async () => undefined}
          onDirtyChange={() => undefined}
          onMoveCopy={() => undefined}
        />
      );
    }
  }

  if (session && completed) {
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''        isManaging={isManaging || isLoadingWindow}
''',
    '''        isManaging={isManaging || isLoadingWindow || isLoadingEditor}
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''        onScheduleAction={(action, intent) => void manageCurrent(action, intent)}
''',
    '''        onEdit={() => void editCurrentCard()}
        onScheduleAction={(action, intent) => void manageCurrent(action, intent)}
''',
)
replace_once(
    "src/screens/PracticeScreen.tsx",
    '''    setTemporaryCardIds([]);
    setSelectedTagIds(new Set());
''',
    '''    setTemporaryCardIds([]);
    setEditingCard(null);
    setSelectedTagIds(new Set());
''',
)

replace_once(
    "src/screens/practice/ActivePractice.tsx",
    '''  onScore: (score: ConfidenceScore) => void;
''',
    '''  onScore: (score: ConfidenceScore) => void;
  onEdit: () => void;
''',
)
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    '''  onScore,
  onScheduleAction,
''',
    '''  onScore,
  onEdit,
  onScheduleAction,
''',
)
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    '''        <div className="practice-header-actions">
          <Menu
''',
    '''        <div className="practice-header-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={busy}
            onClick={onEdit}
          >
            Edit card
          </button>
          <Menu
''',
)

# ---------------------------------------------------------------------------
# Regression tests for the reported failures and new behavior.
# ---------------------------------------------------------------------------
replace_once(
    "src/screens/practice/ActivePractice.test.tsx",
    '''    onScore: vi.fn(),
    onScheduleAction: vi.fn(),
''',
    '''    onScore: vi.fn(),
    onEdit: vi.fn(),
    onScheduleAction: vi.fn(),
''',
)
replace_once(
    "src/screens/practice/ActivePractice.test.tsx",
    '''  it("routes shared menu schedule controls to the exact-card action callback", () => {
''',
    '''  it("opens the card editor from an active session", () => {
    const { props } = renderActive();
    fireEvent.click(screen.getByRole("button", { name: "Edit card" }));
    expect(props.onEdit).toHaveBeenCalledOnce();
  });

  it("routes shared menu schedule controls to the exact-card action callback", () => {
''',
)

# Add refresh IPC mock; keep the rest of PracticeScreen's behavior unchanged.
replace_once(
    "src/screens/PracticeScreen.test.tsx",
    '''  markPracticeCardViewed: vi.fn(),
''',
    '''  markPracticeCardViewed: vi.fn(),
  refreshPracticeAfterCardEdit: vi.fn(),
''',
)

# Settings regression test for the simplified profile UI.
replace_once(
    "src/screens/SettingsScreen.test.tsx",
    '''  it("backs up provider metadata while explicitly excluding credentials", async () => {
''',
    '''  it("uses one New profile action and selects provider type inside the connection form", async () => {
    renderSettings();
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
    expect(await screen.findByRole("heading", { name: "AI provider profiles" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "New profile" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Add llama\.cpp/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Add DeepSeek/u })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    const provider = await screen.findByRole("combobox", { name: "Provider type" });
    expect((provider as HTMLSelectElement).value).toBe("llama_cpp");
    fireEvent.change(provider, { target: { value: "deepseek" } });
    expect((provider as HTMLSelectElement).value).toBe("deepseek");
    expect(screen.queryByText(/chat template embedded with the loaded model/u)).toBeNull();
    expect(screen.queryByText(/credential never enters SQLite/u)).toBeNull();
    expect(screen.queryByText(/normally reports the loaded GGUF file path/u)).toBeNull();
  });

  it("backs up provider metadata while explicitly excluding credentials", async () => {
''',
)

# Native practice tests: consecutive equal-priority shuffled sessions should not
# start with the same card, and an edit can remove a reverse variant while
# refreshing the surviving snapshot.
append_marker = '''#[test]
fn practice_start_persists_snapshot_and_returns_a_bounded_first_window() {'''
replace_once(
    "src-tauri/src/practice_selection/operation_tests.rs",
    append_marker,
    '''#[test]
fn consecutive_shuffled_sessions_avoid_repeating_the_same_equal_priority_first_card() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Shuffle fairness".to_string(),
                    description: None,
                },
            )?
            .id;
            for _ in 0..6 {
                create_card(connection, &deck_id)?;
            }
            let first = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                "1".repeat(32),
                vec![deck_id.clone()],
                Vec::new(),
                true,
                "extra",
            )?;
            let second = start_selected_practice_in_connection(
                connection,
                &paths.paths,
                "2".repeat(32),
                vec![deck_id],
                Vec::new(),
                true,
                "extra",
            )?;
            assert_ne!(
                first.first_window.cards[0].presentation.id,
                second.first_window.cards[0].presentation.id
            );
            Ok(())
        })
        .expect("equivalent shuffled sessions should rotate their first card");
}

#[test]
fn practice_edit_refresh_removes_deleted_reverse_variant_and_updates_snapshot() {
    let paths = TestPaths::new();
    let database = Database::open_in_memory().expect("database should open");
    database
        .with_connection(|connection| {
            let deck_id = create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: "Edit during practice".to_string(),
                    description: None,
                },
            )?
            .id;
            let (note_id, forward_id) = create_card(connection, &deck_id)?;
            let reverse_id = generate_id(connection)?;
            connection.execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'reverse', 0, 10, 10)",
                params![reverse_id, note_id, deck_id],
            ).map_err(CommandError::database)?;
            insert_new_schedule(connection, &reverse_id, 10)?;
            connection.execute(
                "UPDATE notes SET reverse_enabled = 1 WHERE id = ?1",
                [note_id.as_str()],
            ).map_err(CommandError::database)?;

            let session = start_card_practice_in_connection(
                connection,
                &paths.paths,
                "3".repeat(32),
                vec![forward_id.clone(), reverse_id.clone()],
                false,
            )?;
            let edited = validate_content(
                paragraph("Updated question"),
                paragraph("Updated answer"),
                "Updated question".to_string(),
                "Updated answer".to_string(),
            )?;
            connection.execute(
                "UPDATE notes
                 SET front_doc = ?1, back_doc = ?2, front_text = ?3, back_text = ?4,
                     reverse_enabled = 0, revision = revision + 1, updated_at = 20
                 WHERE id = ?5",
                params![
                    edited.front_doc_json,
                    edited.back_doc_json,
                    edited.front_text,
                    edited.back_text,
                    note_id,
                ],
            ).map_err(CommandError::database)?;
            connection.execute(
                "UPDATE cards SET deleted_at = 20, updated_at = 20 WHERE id = ?1",
                [reverse_id.as_str()],
            ).map_err(CommandError::database)?;

            let refreshed = refresh_practice_after_card_edit_in_connection(
                connection,
                &paths.paths,
                RefreshPracticeAfterCardEditInput {
                    session_id: session.id,
                    note_id,
                    start_position: 0,
                },
            )?;
            assert_eq!(refreshed.card_count, 1);
            assert!(!refreshed.completed);
            assert_eq!(refreshed.window.cards.len(), 1);
            assert_eq!(refreshed.window.cards[0].presentation.id, forward_id);
            assert_eq!(refreshed.window.cards[0].presentation.front_text, "Updated question");
            Ok(())
        })
        .expect("practice session should refresh after editing its note");
}

#[test]
fn practice_start_persists_snapshot_and_returns_a_bounded_first_window() {''',
)

print("PR41 final fixes applied")
