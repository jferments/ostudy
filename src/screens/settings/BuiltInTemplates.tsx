import { useEffect, useRef, useState } from "react";

import { useAiFeatures } from "../../controllers/AiFeaturesProvider";
import { getErrorMessage } from "../../ipc/common";
import {
  getQuestionGenerationPolicy,
  saveQuestionGenerationPolicy,
  type QuestionFamily,
  type QuestionGenerationSnapshot,
} from "../../ipc/questionGeneration";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Slider, Switch } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { TextField } from "../../ui/primitives/TextField";
import {
  GENERATION_FAMILIES,
  parseQuestionGenerationDraft,
  recommendedQuestionMix,
  toQuestionGenerationDraft,
  type QuestionGenerationDraft,
} from "./questionGenerationDraft";

const FAMILY_MARKS: Record<QuestionFamily, string> = {
  recall: "Aa",
  single_choice: "1",
  select_all: "✓✓",
  true_false: "T/F",
  matching: "⇄",
  grouping: "▦",
  ordering: "1-2",
};

/**
 * The card formats ostudy ships with. While AI is on, a switch says whether
 * AI may write that format from documents; it saves at once. The weights live
 * in the Question mix sheet.
 */
export function BuiltInTemplates() {
  const ai = useAiFeatures().enabled;
  const [saved, setSaved] = useState<QuestionGenerationSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadRevision, setLoadRevision] = useState(0);
  const [mixOpen, setMixOpen] = useState(false);
  const alive = useRef(false);
  const savingRef = useRef(false);

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    void getQuestionGenerationPolicy().then((snapshot) => {
      if (current) setSaved(snapshot);
    }).catch((failure: unknown) => {
      if (current) setError(getErrorMessage(failure));
    }).finally(() => {
      if (current) setLoading(false);
    });
    return () => { current = false; };
  }, [loadRevision]);

  async function save(draft: QuestionGenerationDraft): Promise<boolean> {
    const policy = parseQuestionGenerationDraft(draft);
    if (!saved || !policy || savingRef.current) return false;
    savingRef.current = true;
    setSaving(true);
    setError(null);
    try {
      const result = await saveQuestionGenerationPolicy({ revision: saved.revision, policy });
      if (alive.current) setSaved(result);
      return true;
    } catch (failure: unknown) {
      if (alive.current) setError(getErrorMessage(failure));
      return false;
    } finally {
      savingRef.current = false;
      if (alive.current) setSaving(false);
    }
  }

  function toggle(family: QuestionFamily, checked: boolean): void {
    if (!saved) return;
    const draft = toQuestionGenerationDraft(saved.policy);
    draft.allowedTypes = GENERATION_FAMILIES
      .filter(({ value }) => value === family ? checked : draft.allowedTypes.includes(value))
      .map(({ value }) => value);
    // A format switched on with no weight would never be chosen.
    if (checked && Number(draft.weights[family]) === 0) draft.weights[family] = "10";
    void save(draft);
  }

  const allowed = saved?.policy.allowedTypes ?? [];
  return (
    <Panel title="Built-in templates" aria-busy={loading || saving}
      actions={<>
        <HelpTrigger topicId="templates.builtIn" label="Help with built-in templates" />
        {ai ? <Button variant="secondary" disabled={!saved || saving} onClick={() => setMixOpen(true)}>Question mix</Button> : null}
      </>}>
      {loading ? <InlineProgress>Loading built-in templates…</InlineProgress> : null}
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      {!loading && error ? <Button variant="secondary" className="tw:justify-self-start" onClick={() => setLoadRevision((value) => value + 1)}>
        {saved ? "Reload" : "Try again"}
      </Button> : null}
      {saved ? <>
        {ai ? <span aria-hidden="true" className="tw:-mb-3 tw:justify-self-end tw:text-caption tw:text-muted-foreground">AI writes</span> : null}
        <ListRows>
          {GENERATION_FAMILIES.map(({ value, label }) => {
            const on = allowed.includes(value);
            return <ListRow key={value}
              leading={<ListRowMedia label={FAMILY_MARKS[value]} className="tw:normal-case" />}
              title={label}
              trailing={ai ? <Switch aria-label={`AI writes ${label}`} checked={on}
                disabled={saving || (on && allowed.length === 1)}
                onCheckedChange={(checked) => toggle(value, checked)} /> : undefined} />;
          })}
          <ListRow
            leading={<ListRowMedia icon="image" />} title="Image occlusion" />
        </ListRows>
      </> : null}
      {mixOpen && saved ? <QuestionMixSheet saved={saved} saving={saving}
        onClose={() => setMixOpen(false)}
        onSave={async (draft) => { if (await save(draft)) setMixOpen(false); }} /> : null}
    </Panel>
  );
}

function QuestionMixSheet({ saved, saving, onClose, onSave }: {
  saved: QuestionGenerationSnapshot;
  saving: boolean;
  onClose: () => void;
  onSave: (draft: QuestionGenerationDraft) => Promise<void>;
}) {
  const [draft, setDraft] = useState(() => toQuestionGenerationDraft(saved.policy));
  const parsed = parseQuestionGenerationDraft(draft);
  const recommended = toQuestionGenerationDraft(recommendedQuestionMix());
  const isRecommended = JSON.stringify(draft) === JSON.stringify(recommended);
  const isRecallOnly = draft.allowedTypes.length === 1 && draft.allowedTypes[0] === "recall";
  const sliderMax = Math.max(100, ...GENERATION_FAMILIES.map(({ value }) => Number(draft.weights[value]) || 0));

  return (
    <Sheet title="Question mix" width="small" busy={saving} onClose={onClose}
      actions={<>
        <Button variant="secondary" disabled={saving} onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={saving || !parsed} onClick={() => void onSave(draft)}>{saving ? "Saving…" : "Done"}</Button>
      </>}>
      <div className="tw:grid tw:gap-5">
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
          <Button variant={isRecommended ? "primary" : "secondary"} aria-pressed={isRecommended}
            onClick={() => setDraft(recommended)}>Recommended</Button>
          <Button variant={isRecallOnly ? "primary" : "secondary"} aria-pressed={isRecallOnly}
            onClick={() => setDraft((previous) => ({ ...previous, allowedTypes: ["recall"], weights: { ...previous.weights, recall: previous.weights.recall === "0" ? "100" : previous.weights.recall } }))}>
            Recall only
          </Button>
          <span className="tw:ms-auto"><HelpTrigger topicId="templates.questionMix" label="Help with the question mix" /></span>
        </div>
        <ListRows>
          {GENERATION_FAMILIES.map(({ value, label }) => {
            const on = draft.allowedTypes.includes(value);
            return <ListRow key={value} density="dense" className="tw:min-h-14 tw:items-center" title={label}
              trailing={<>
                <div className="tw:w-36">
                  <Slider label={`${label} weight`} min={0} max={sliderMax} value={Number(draft.weights[value]) || 0} disabled={!on}
                    onValueChange={(weight) => setDraft((previous) => ({ ...previous, weights: { ...previous.weights, [value]: String(weight) } }))} />
                </div>
                <span className="tw:w-9 tw:text-right tw:font-mono tw:text-small tw:tabular-nums tw:text-muted-foreground">{on ? draft.weights[value] : "Off"}</span>
                <Switch aria-label={`Use ${label}`} checked={on} disabled={on && draft.allowedTypes.length === 1}
                  onCheckedChange={(checked) => setDraft((previous) => ({
                    ...previous,
                    allowedTypes: GENERATION_FAMILIES.filter((family) => family.value === value ? checked : previous.allowedTypes.includes(family.value)).map((family) => family.value),
                    weights: checked && previous.weights[value] === "0" ? { ...previous.weights, [value]: "10" } : previous.weights,
                  }))} />
              </>} />;
          })}
        </ListRows>
        <label className="tw:flex tw:min-h-12 tw:items-center tw:justify-between tw:gap-6 tw:text-body tw:font-medium">
          <span>Most parts in one question</span>
          <TextField type="number" inputMode="numeric" min={2} max={16} step={1} className="tw:w-20 tw:text-right"
            value={draft.maxCompoundItems}
            onChange={(event) => {
              const maxCompoundItems = event.currentTarget.value;
              setDraft((previous) => ({ ...previous, maxCompoundItems }));
            }} />
        </label>
        {!parsed ? <InlineNotice kind="error">Use 2 to 16 parts in one question.</InlineNotice> : null}
      </div>
    </Sheet>
  );
}
