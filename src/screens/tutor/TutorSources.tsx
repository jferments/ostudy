import { useId, useState } from "react";
import { scopeMembers, selectedScope, studyScopeKey, type MaterialScope, type ScopeChoice, type StudyScope } from "../../ipc/knowledge";
import { Dialog, DialogActions } from "../../ui/overlays/Dialog";
import { Checkbox } from "../../ui/primitives/FormControls";
import { TextField } from "../../ui/primitives/TextField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Button } from "../../ui/primitives/Button";
import { styles } from "./presentation";

/** Selection is local-only. Applying creates a new scope, never edits a saved chat's permissions. */
export function TutorSources({
  scope,
  choices,
  onApply,
  onClose,
  title = "Sources",
  description = "Choose study material to ground this conversation. No selection means general discussion.",
  note = "Changing sources starts a new chat; previous chats keep their sources. Saved knowledge is reused. Selecting material here sends nothing.",
  applyLabel = "Use sources",
}: {
  scope: StudyScope;
  choices: ScopeChoice[];
  onApply: (scope: StudyScope) => void;
  onClose: () => void;
  title?: string;
  description?: string;
  note?: string;
  applyLabel?: string;
}) {
  const id = useId();
  const [selected, setSelected] = useState<MaterialScope[]>(() => scopeMembers(scope));
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const material = choices.filter((choice) => ["deck", "document", "guide", "collection"].includes(choice.scope.kind));
  const available = [...material];
  for (const member of scopeMembers(scope)) {
    if (!available.some((choice) => studyScopeKey(choice.scope) === studyScopeKey(member))) {
      available.push({ scope: member, title: "Previously selected material (not currently listed)" });
    }
  }
  function toggle(member: MaterialScope, checked: boolean): void {
    const key = studyScopeKey(member);
    const next = selected.filter((entry) => studyScopeKey(entry) !== key);
    if (checked) next.push(member);
    if (next.length > 16) { setError("Select at most 16 sources for one conversation."); return; }
    setSelected(next); setError(null);
  }
  return <Dialog title={title} description={description} onClose={onClose} width="medium">
    <label className={styles.field} htmlFor={`${id}-search`}>Find material
      <TextField id={`${id}-search`} type="search" value={query} onChange={(event) => setQuery(event.target.value)} />
    </label>
    <div className={styles.sourceChoices}>
      {available.filter((choice) => choice.title.toLocaleLowerCase().includes(query.toLocaleLowerCase())).map((choice) => {
        const member = choice.scope as MaterialScope, key = studyScopeKey(member);
        return <label key={key} className={styles.sourceChoice}>
          <Checkbox checked={selected.some((entry) => studyScopeKey(entry) === key)} onCheckedChange={(checked) => toggle(member, checked)} />
          <span>{choice.title}<small>{member.kind === "deck" ? "Flashcards" : member.kind === "guide" ? "Study guide" : member.kind === "collection" ? "Document collection" : "Document"}</small></span>
        </label>;
      })}
      {!available.length ? <p className={styles.muted}>Import a document or create flashcards to add study sources.</p> : null}
    </div>
    <p className={styles.muted}>{selected.length} selected. {note}</p>
    {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    <DialogActions>
      <Button onClick={() => { setSelected([]); setError(null); }}>Clear selection</Button>
      <Button onClick={onClose}>Cancel</Button>
      <Button variant="primary" onClick={() => { onApply(selectedScope(selected)); onClose(); }}>{applyLabel}</Button>
    </DialogActions>
  </Dialog>;
}
