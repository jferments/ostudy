# Editor preview: presentation and mutation boundaries

The editor's full-width preview is an inline authoring workspace, not a new Practice session or modal. `src/screens/CardEditorScreenBase.tsx` retains its controller and hidden/inert editor subtree. `card-editor/useEditorPreview.ts` captures the last text field/DOM selection and actual scrolling ancestors. **Close preview**, the existing editor-owned Android Back handler, and Escape when no overlay/composition owns it restore that state without committing or discarding the draft. Hidden editor media is paused rather than silently continuing behind preview.

`src/screens/practice/RecallCardSurface.tsx` is shared by author previews and `src/screens/practice/ActivePractice.tsx`. It owns only reveal/flip presentation, rich rendering, accessible faces, and active-face playback. Practice keeps session creation, scheduling controls, shortcut ratings, progress, feedback, analytics, and Tutor ownership. The editor never constructs a fake Practice session merely to obtain the real study presentation.

## Native draft projection

Basic and Fill-in-the-blank previews call the pure native `preview_card_draft` command. Rust validates the unsaved rich content and renders presentations through the same `render_projected_presentation` path used by ordinary Practice reads. Reverse Basic cards expose Forward and Reverse. One-at-a-time Cloze exposes each authored blank group as a transient preview choice while retaining one scheduled-card identity; all-together exposes the single combined presentation. These choices do not create cards, schedules, or review history.

Reusable-template preview reuses the existing native `preview_template_cards` renderer. Every generated rule becomes a selectable full-workspace presentation, using its rendered rich documents and managed assets. The smaller field/template and Cloze examples inside their authoring forms remain distinct editing aids; they are not a second implementation of the full card preview.

Objective questions send the unsaved canonical definition to `preview_question_draft`. Rust applies the same validation, deterministic display-order algorithm, public answer-hiding projection, and evaluator used by captured Practice questions. **Check answer** returns transient native feedback through the shared `QuestionBody`; Reset returns to an unanswered local response. The preview command has no database, session, attempt, schedule, review, analytics, or provider dependency.

Image occlusion remains on its stricter native masked-raster route. The preview can choose among the note's authored target questions, but pre-answer rendering never gains the original source-image projection merely to share the objective preview command. Revealing an occlusion in author preview is local authoring behavior and does not create learner evidence.

The former preview aside, split-column recipe, separate Basic preview surface, and duplicated Practice shuffle/readiness helpers are removed. Frontend preview state selects among already validated projections; correctness and private answer relationships remain native.

## Regression and acceptance coverage

`src/screens/CardEditorPreviewWorkspace.test.tsx` covers the mounted unsaved editor, selected-field restoration, Basic reverse, Cloze presentation selection, reusable-template outputs, native Back, Escape/overlay/composition precedence, disabled Save shortcuts, and absence of save/discard callbacks. `src/screens/card-editor/QuestionDraftPreview.test.tsx` covers native objective compilation/check orchestration; `RecallDraftPreview.test.tsx` covers presentation switching and reveal reset. Native tests cover Basic/Cloze projection and objective validation/evaluation without storage.

Visual acceptance compares the same long, media-rich, math-rich, and interactive content in Practice and preview at compact, medium, and wide sizes, with reduced motion and enlarged text. Physical Android Back/media behavior and final visual comparison remain observations that must be reported separately from deterministic/native tests; automated coverage does not imply a physical-device or every-WebView acceptance result.
