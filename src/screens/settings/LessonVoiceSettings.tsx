import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getSpeechSettings, saveSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Panel } from "../../ui/primitives/Panel";

const fieldClass = "tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground";

/**
 * The voice that reads lessons aloud. It reloads when the AI connections
 * change, since only an OpenRouter connection can speak.
 */
export function LessonVoiceSettings({ connectionsKey }: { connectionsKey: unknown }) {
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

  if (!settings) {
    return error ? (
      <Panel headingLevel={2} title="Lesson voice">
        <InlineNotice kind="error">The lesson voice could not load: {error}</InlineNotice>
      </Panel>
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

  return (
    <Panel headingLevel={2} title="Lesson voice">
      <div className="tw:grid tw:min-w-0 tw:gap-4 tw:xl:grid-cols-3">
        <label className={fieldClass}>
          <span>Connection</span>
          <NativeSelect
            aria-label="Lesson voice connection"
            value={settings.profileId ?? ""}
            disabled={saving}
            onChange={(event) =>
              save(event.currentTarget.value || null, settings.modelId, settings.voice)
            }
          >
            <option value="">{settings.connections.length ? "No connection selected" : "No OpenRouter connection"}</option>
            {settings.connections.map((connection) => (
              <option key={connection.id} value={connection.id}>
                {connection.name}
              </option>
            ))}
          </NativeSelect>
        </label>
        <label className={fieldClass}>
          <span>Voice model</span>
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
        </label>
        {model && model.voices.length > 0 ? (
          <label className={fieldClass}>
            <span>Voice</span>
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
          </label>
        ) : null}
      </div>
      {error ? <InlineNotice kind="error">The lesson voice was not saved: {error}</InlineNotice> : null}
    </Panel>
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
