# Flashcards workspace

The **Flashcards** area keeps the main authoring and library tools together without placing every tool on the screen at once.

## Workspaces

- **Decks** organizes the study library and opens individual deck details.
- **Templates** creates and edits reusable card structures.
- **Search** finds cards across decks and supports selection and bulk actions.
- **Import/Export** transfers decks, generated-card JSON, and complete-library backups through the system file picker or share sheet.
- **Create Cards** supports both focused single-card authoring and bulk entry.

Each workspace is a real navigation destination. Returning from practice, restoring the application after suspension, and using Android Back preserve the relevant Flashcards destination rather than treating the workspaces as permanently mounted tabs.

## Deck folders

Folders provide one optional organizational level above decks.

- A deck can belong to one folder or remain under **Unfiled decks**.
- Folders appear before unfiled decks.
- Deleting a folder moves its decks to Unfiled. It does not delete cards, schedules, review history, or analytics.
- Archived decks remain separate from the active folder view and keep their existing historical behavior.

Folder names, membership, folder order, and deck order are durable SQLite data. They are included in complete-library database snapshots together with the rest of the study library.

## Moving and ordering

Pointer and touch users can drag a deck by its handle. Dragging is only a convenience: every deck also has a **Move** action and explicit up/down controls, and folders have up/down controls of their own.

Movement is committed as one transaction. The application uses an organization revision to reject stale changes made against an older library view. When a commit fails, the optimistic presentation rolls back and the previous order remains intact.

## Comfortable and compact views

**Comfortable** view shows descriptions and progress more prominently. **Compact** view reduces vertical space so larger libraries are easier to scan. Density changes presentation only; they do not alter folders, order, schedules, or content.

## Search and filters

The ordinary Search view keeps four things visible:

1. the text query;
2. the current deck scope;
3. sorting;
4. an **All filters** action.

Additional filters open in an adaptive sheet. Scope can be chosen by folder or by individual deck. Study-state and tag filters update the results immediately, and active filters reappear above the results as removable chips.

## Contextual help

Question-mark controls open concise help for folders, density, study-state filters, and other unfamiliar concepts. Desktop pointer users also receive a short tooltip. Touch devices use the full help sheet instead of depending on hover.
