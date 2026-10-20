import { useId, useState } from "react";
import { sameStudyScope, scopeMembers, selectedScope, studyScopeKey, type MaterialScope, type ScopeChoice, type StudyScope } from "../../ipc/knowledge";
import { Dialog } from "../../ui/overlays/Dialog";
import { Checkbox } from "../../ui/primitives/FormControls";
import { TextField } from "../../ui/primitives/TextField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Button } from "../../ui/primitives/Button";
import { styles } from "./presentation";

/** The title of a chat's chosen material, or null when it has none (general knowledge only). */
export function sourcesLabel(scope: StudyScope, choices: ScopeChoice[]): string | null {
  if (scope.kind === "general") return null;
  if (scope.kind === "library") return "Library";
  const members = scopeMembers(scope);
  if (members.length > 1) return `${members.length} sources`;
  const member = members[0];
  if (!member) return null;
  return choices.find((choice) => sameStudyScope(choice.scope, member))?.title ?? "Selected source";
}

/** Selection is local-only. Applying creates a new scope, never edits a saved chat's permissions. */
export function TutorSources({
  scope,
  choices,
  onApply,
  onClose,
  title = "Sources",
  applyLabel = "Use sources",
}: {
  scope: StudyScope;
  choices: ScopeChoice[];
  onApply: (scope: StudyScope) => void;
  onClose: () => void;
  title?: string;
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
  return <Dialog title={title} onClose={onClose} width="medium"
    actions={<>
      <Button className="tw:mr-auto" onClick={() => { setSelected([]); setError(null); }}>Clear selection</Button>
      <Button onClick={onClose}>Cancel</Button>
      <Button variant="primary" onClick={() => { onApply(selectedScope(selected)); onClose(); }}>{applyLabel}</Button>
    </>}>
    <label className={styles.field} htmlFor={`${id}-search`}>Find material
      <TextField id={`${id}-search`} type="search" value={query} onChange={(event) => setQuery(event.target.value)} />
    </label>
    <div className={styles.sourceChoices}>
      {available.filter((choice) => choice.title.toLocaleLowerCase().includes(query.toLocaleLowerCase())).map((choice) => {
        const member = choice.scope as MaterialScope, key = studyScopeKey(member);
        return <label key={key} className={styles.sourceChoice}>
          <Checkbox checked={selected.some((entry) => studyScopeKey(entry) === key)} onCheckedChange={(checked) => toggle(member, checked)} />
          <span>{choice.title}<small>{member.kind === "deck" ? "Flashcards" : member.kind === "guide" ? "Study guide" : member.kind === "collection" ? "Subject documents" : "Document"}</small></span>
        </label>;
      })}
      {!available.length ? <p className={styles.muted}>No study material yet.</p> : null}
    </div>
    <p className={styles.muted}>{selected.length} selected</p>
    {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
  </Dialog>;
}
