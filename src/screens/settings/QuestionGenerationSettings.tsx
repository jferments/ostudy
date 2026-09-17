import { useEffect, useRef, useState } from "react";
import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  getQuestionGenerationPolicy,
  saveQuestionGenerationPolicy,
  type QuestionGenerationSnapshot,
} from "../../ipc/questionGeneration";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Disclosure } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Panel } from "../../ui/primitives/Panel";
import { TextField } from "../../ui/primitives/TextField";
import {
  GENERATION_FAMILIES,
  parseQuestionGenerationDraft,
  recommendedQuestionMix,
  toQuestionGenerationDraft,
  type QuestionGenerationDraft,
} from "./questionGenerationDraft";

interface Props {
  showToast: ShowToast;
  onDirtyChange: (dirty: boolean) => void;
}

export function QuestionGenerationSettings({ showToast, onDirtyChange }: Props) {
  const [saved, setSaved] = useState<QuestionGenerationSnapshot | null>(null);
  const [draft, setDraft] = useState<QuestionGenerationDraft | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadRevision, setLoadRevision] = useState(0);
  const alive = useRef(false);
  const savingRef = useRef(false);
  const dirty = saved !== null && draft !== null
    && JSON.stringify(draft) !== JSON.stringify(toQuestionGenerationDraft(saved.policy));
  const parsed = draft === null ? null : parseQuestionGenerationDraft(draft);

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  useEffect(() => {
    let current = true;
    setLoading(true);
    setError(null);
    void getQuestionGenerationPolicy().then((snapshot) => {
      if (!current) return;
      setSaved(snapshot);
      setDraft(toQuestionGenerationDraft(snapshot.policy));
    }).catch((failure: unknown) => {
      if (current) setError(getErrorMessage(failure));
    }).finally(() => {
      if (current) setLoading(false);
    });
    return () => { current = false; };
  }, [loadRevision]);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  async function save(): Promise<void> {
    if (!saved || !parsed || !dirty || savingRef.current) return;
    savingRef.current = true;
    setSaving(true);
    setError(null);
    try {
      const result = await saveQuestionGenerationPolicy({ revision: saved.revision, policy: parsed });
      if (!alive.current) return;
      setSaved(result);
      setDraft(toQuestionGenerationDraft(result.policy));
      showToast("success", "Question generation preferences saved.");
    } catch (failure: unknown) {
      if (alive.current) setError(getErrorMessage(failure));
    } finally {
      savingRef.current = false;
      if (alive.current) setSaving(false);
    }
  }

  return <Panel className="tw:min-w-0" aria-busy={loading || saving} aria-label="Question generation">
    <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Question generation</h2>
    <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-muted-foreground">
      Choose the formats used for new document-generated cards. Existing cards and running tasks are unchanged.
      Image occlusion remains manually authored.
    </p>
    {loading ? <InlineProgress>Loading question generation preferences…</InlineProgress> : null}
    {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    {!loading && draft ? <fieldset disabled={saving} className="tw:m-0 tw:grid tw:min-w-0 tw:gap-5 tw:border-0 tw:p-0">
      <legend className="tw:sr-only">Generated question formats</legend>
      <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2">
        {GENERATION_FAMILIES.map(({ value, label }) => <label key={value} className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:text-label">
          <Checkbox
            aria-label={label}
            checked={draft.allowedTypes.includes(value)}
            disabled={saving || (draft.allowedTypes.length === 1 && draft.allowedTypes.includes(value))}
            onCheckedChange={(checked) => setDraft((previous) => previous === null ? null : ({
              ...previous,
              allowedTypes: GENERATION_FAMILIES.filter((family) => family.value === value
                ? checked : previous.allowedTypes.includes(family.value)).map((family) => family.value),
            }))}
          />
          <span>{label}</span>
        </label>)}
      </div>
      <div className="tw:flex tw:flex-wrap tw:gap-2">
        <Button type="button" variant="secondary" disabled={saving} onClick={() => setDraft(toQuestionGenerationDraft(recommendedQuestionMix()))}>
          Recommended mix
        </Button>
        <Button type="button" variant="secondary" disabled={saving} onClick={() => setDraft((previous) => previous === null ? null : ({
          ...previous, allowedTypes: ["recall"], weights: { ...previous.weights, recall: "100" },
        }))}>
          Recall only
        </Button>
      </div>
      <Disclosure title="Customize question mix">
        <div className="tw:grid tw:gap-4">
          <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-muted-foreground">
            Relative weights are preferences, not quotas. Source coverage and suitable, unambiguous questions come first.
            A weight of zero removes the preference for that enabled type; uncheck it to prohibit it.
          </p>
          <div className="tw:grid tw:gap-4 tw:sm:grid-cols-2">
            {GENERATION_FAMILIES.map(({ value, label }) => <FormField key={value} label={`${label} weight`}>
              <TextField type="number" inputMode="numeric" min={0} max={1000} step={1}
                value={draft.weights[value]} disabled={saving || !draft.allowedTypes.includes(value)}
                onChange={(event) => {
                  const weight = event.currentTarget.value;
                  setDraft((previous) => previous === null ? null : ({ ...previous, weights: { ...previous.weights, [value]: weight } }));
                }} />
            </FormField>)}
          </div>
          <FormField label="Maximum components per compound question" description="Choose 2–16 options, pairs, items, or steps. Large boards are not used to fill a question-count quota.">
            <TextField type="number" inputMode="numeric" min={2} max={16} step={1}
              value={draft.maxCompoundItems} disabled={saving}
              onChange={(event) => {
                const maxCompoundItems = event.currentTarget.value;
                setDraft((previous) => previous === null ? null : ({ ...previous, maxCompoundItems }));
              }} />
          </FormField>
        </div>
      </Disclosure>
      {!parsed ? <InlineNotice kind="error">Use whole-number weights from 0–1,000, a positive weight for an enabled format, and a component limit from 2–16.</InlineNotice> : null}
      <div className="tw:flex tw:flex-wrap tw:gap-2">
        <Button type="button" variant="primary" disabled={saving || !dirty || !parsed} onClick={() => void save()}>
          {saving ? "Saving…" : "Save question preferences"}
        </Button>
        {dirty && saved ? <Button type="button" variant="secondary" disabled={saving} onClick={() => {
          setDraft(toQuestionGenerationDraft(saved.policy));
          setError(null);
        }}>Discard question changes</Button> : null}
      </div>
    </fieldset> : null}
    {!loading && error ? <Button type="button" variant="secondary" disabled={saving} onClick={() => setLoadRevision((value) => value + 1)}>
      {saved ? "Reload saved question preferences" : "Retry loading preferences"}
    </Button> : null}
  </Panel>;
}
