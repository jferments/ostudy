import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getSpeechSettings, saveSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TASK_GRID } from "./DefaultAiModelSettings";

const fieldClass = "tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground";

/**
 * The Voice row of "Default for each task": the connection, model and, for
 * models with preset voices, the voice that reads lessons aloud. It reloads
 * when the AI connections change, since only an OpenRouter connection can
 * speak.
 */
export function LessonVoiceSettings({ connectionsKey }: { connectionsKey: unknown }) {
  const wide = useLayoutMode().layoutMode === "wide";
  const lane = useRequestLane();
  const [settings, setSettings] = useState<SpeechSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void latestResult(lane, getSpeechSettings).then(
      (value) => {
        if (value !== undefined) setSettings(value);
      },
      (failure: unknown) => setError(getErrorMessage(failure)),
    );
  }, [lane, connectionsKey]);

  const rowClass = wide
    ? "tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-2.5"
    : "tw:grid tw:min-w-0 tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-4";

  if (!settings) {
    return error ? (
      <div role="listitem" data-task="voice" className={rowClass}>
        <InlineNotice kind="error">The lesson voice could not load: {error}</InlineNotice>
      </div>
    ) : null;
  }

  const model = settings.models.find((candidate) => candidate.id === settings.modelId);
  const save = (profileId: string | null, modelId: string, voice: string) => {
    setSaving(true);
    setError(null);
    saveSpeechSettings(profileId, modelId, voice, settings.revision)
      .then(setSettings, (failure: unknown) => {
        setError(getErrorMessage(failure));
        // Show what is saved now, whoever changed it.
        return getSpeechSettings().then(setSettings, () => undefined);
      })
      .finally(() => setSaving(false));
  };

  const connectionSelect = (
    <NativeSelect
      aria-label="Lesson voice connection"
      value={settings.profileId ?? ""}
      disabled={saving}
      onChange={(event) => save(event.currentTarget.value || null, settings.modelId, settings.voice)}
    >
      <option value="">{settings.connections.length ? "Choose a service" : "No OpenRouter service"}</option>
      {settings.connections.map((connection) => (
        <option key={connection.id} value={connection.id}>
          {connection.name}
        </option>
      ))}
    </NativeSelect>
  );
  const modelSelect = (
    <NativeSelect
      aria-label="Lesson voice model"
      value={settings.modelId}
      disabled={saving}
      onChange={(event) => {
        const next = settings.models.find((candidate) => candidate.id === event.currentTarget.value);
        if (next) save(settings.profileId, next.id, next.defaultVoice);
      }}
    >
      {settings.models.map((candidate) => (
        <option key={candidate.id} value={candidate.id}>
          {candidate.label}
        </option>
      ))}
    </NativeSelect>
  );
  const voiceSelect =
    model && model.voices.length > 0 ? (
      <NativeSelect
        aria-label="Lesson voice"
        value={settings.voice}
        disabled={saving}
        onChange={(event) => save(settings.profileId, settings.modelId, event.currentTarget.value)}
      >
        {model.voices.map((voice) => (
          <option key={voice} value={voice}>
            {voiceLabel(voice)}
          </option>
        ))}
      </NativeSelect>
    ) : null;
  const menu = (
    <OverflowMenu
      label="More for Voice"
      disabled={saving}
      items={[
        {
          id: "clear",
          label: "Clear",
          disabled: !settings.profileId,
          onSelect: () => save(null, settings.modelId, settings.voice),
        },
      ]}
    />
  );
  const feedback = error ? <InlineNotice kind="error">The lesson voice was not saved: {error}</InlineNotice> : null;

  return wide ? (
    <div role="listitem" data-task="voice" className={rowClass}>
      <div className={TASK_GRID}>
        <span className="tw:font-medium">Voice</span>
        {connectionSelect}
        {modelSelect}
        {voiceSelect ?? <span />}
        {menu}
      </div>
      {feedback}
    </div>
  ) : (
    <div role="listitem" data-task="voice" className={rowClass}>
      <div className="tw:flex tw:items-center tw:gap-2">
        <strong className="tw:flex-1 tw:font-medium">Voice</strong>
        {menu}
      </div>
      <label className={fieldClass}>
        <span>Service</span>
        {connectionSelect}
      </label>
      <label className={fieldClass}>
        <span>Model</span>
        {modelSelect}
      </label>
      {voiceSelect ? (
        <label className={fieldClass}>
          <span>Voice</span>
          {voiceSelect}
        </label>
      ) : null}
      {feedback}
    </div>
  );
}

const ACCENTS: Record<string, string> = { a: "American", b: "British" };

/** Kokoro names voices like `af_heart`: accent, then gender, then a name. */
export function voiceLabel(voice: string): string {
  const match = /^([ab])([fm])_(\w+)$/u.exec(voice);
  if (!match) return voice;
  const [, accent, gender, name] = match as unknown as [string, string, string, string];
  const title = name.charAt(0).toUpperCase() + name.slice(1);
  return `${title} (${ACCENTS[accent]} ${gender === "f" ? "female" : "male"})`;
}
