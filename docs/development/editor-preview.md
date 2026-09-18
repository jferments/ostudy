# Editor preview: presentation and mutation boundaries

The editor's full-width preview is an inline workspace, not a new Practice session or modal. `src/screens/CardEditorScreenBase.tsx` retains its controller and hidden/inert editor subtree. `card-editor/useEditorPreview.ts` captures the last text field/DOM selection and actual scrolling ancestors. Close and the existing editor-owned Android Back handler restore that state without committing/discarding the draft. Escape respects an active overlay and composition. Hidden editor media is paused, not silently resumed.

`src/screens/practice/RecallCardSurface.tsx` is shared by author previews and `src/screens/practice/ActivePractice.tsx` (the latter is under `practice/`, not directly under `screens/`). It owns only reveal/flip presentation, the rich renderer, accessible faces and active-face playback. Practice keeps its session effects, scheduling controls, shortcut ratings, progress and feedback ownership. No fake session is constructed to obtain card styling.

Objective preview still uses `QuestionBody` and its current draft projection without grading. Image preview retains native masked derivatives and the existing draft asset lease. The former preview aside, split-column recipe and separate Basic preview surface are removed. Cloze and template field examples remain live, distinct controls and have not been deleted as unused.

The next LK-01 increment adds native draft compilation/checking and brings Cloze/template presentation selection into the full workspace. This first increment must not be reported as all of LK-01, LK-02 or LK-03 complete. The current-format conversion and shared-knowledge work are specified in the [foundation plan](shared-knowledge-lessons/01-foundations.md).

## Regression coverage

`src/screens/CardEditorPreviewWorkspace.test.tsx` exercises mounted draft preservation, selected-field restoration, reverse content, disabled Save shortcuts, native Back, Escape/overlay/composition handling, and the absence of save/discard callbacks. Existing Practice suites continue covering session/rating behavior through the extracted shared surface. TypeScript syntax checks are auxiliary; exact-head repository/static/fast and relevant platform results belong in the PR, not a timeless assertion here.

Visual acceptance compares the same long, media-rich and math-rich cards in Practice and preview at compact/medium/wide sizes, with reduced motion and enlarged text. Physical Android Back, media focus and actual visual parity remain separately observed gates. Tests do not claim every browser or physical device was exercised.
