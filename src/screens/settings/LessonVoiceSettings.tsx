import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getSpeechSettings, saveSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { Button } from "../../ui/primitives/Button";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TASK_GRID, TaskOffRow } from "./DefaultAiModelSettings";
import { SpeechVoicesDialog } from "./SpeechVoicesDialog";

const fieldClass = "tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground";

/**
 * The Voice row of "Default for each task": the connection, model and voice
 * that read lessons aloud. A model with preset voices offers its list; a model
 * that copies voices offers the voices that come with ostudy and the learner's
 * saved ones, managed in the Lesson
 * voices dialog. It reloads when the AI connections change, since only an
 * OpenRouter connection can speak.
 */
export function LessonVoiceSettings({
  connectionsKey,
  onAddOpenRouter,
}: {
  connectionsKey: unknown;
  /** Offered when no OpenRouter connection can speak. */
  onAddOpenRouter?: () => void;
}) {
  const wide = useLayoutMode().layoutMode === "wide";
  const lane = useRequestLane();
  const [settings, setSettings] = useState<SpeechSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [voices, setVoices] = useState<"closed" | "open" | "try">("closed");

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

  if (settings.connections.length === 0 && onAddOpenRouter) {
    return <TaskOffRow task="voice" label="Voice" wide={wide} onAddOpenRouter={onAddOpenRouter} />;
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
        if (!next) return;
        // A model that copies voices starts with its first voice, the default.
        const own = settings.voices.find((voice) => voice.state !== "tried" && voice.modelId === next.id);
        save(settings.profileId, next.id, next.clonesVoices ? (own?.voice ?? "") : next.defaultVoice);
      }}
    >
      {settings.models.map((candidate) => (
        <option key={candidate.id} value={candidate.id}>
          {candidate.label}
        </option>
      ))}
    </NativeSelect>
  );
  const saved = settings.voices.filter((voice) => voice.state !== "tried" && voice.modelId === settings.modelId);
  const clones = model?.clonesVoices === true;
  const voiceSelect = clones ? (
    saved.length > 0 ? (
      <NativeSelect
        aria-label="Lesson voice"
        value={settings.voice}
        disabled={saving}
        onChange={(event) => save(settings.profileId, settings.modelId, event.currentTarget.value)}
      >
        {saved.some((voice) => voice.voice === settings.voice) ? null : <option value="">Choose a voice</option>}
        {saved.map((voice) => (
          <option key={voice.id} value={voice.voice}>
            {voice.name}
          </option>
        ))}
      </NativeSelect>
    ) : (
      <Button disabled={saving || !settings.profileId} onClick={() => setVoices("try")}>
        Try a voice
      </Button>
    )
  ) : model && model.voices.length > 0 ? (
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
        ...(clones
          ? [
              {
                id: "voices",
                label: "Lesson voices…",
                disabled: !settings.profileId,
                onSelect: () => setVoices("open"),
              },
            ]
          : []),
        {
          id: "clear",
          label: "Clear",
          disabled: !settings.profileId,
          onSelect: () => save(null, settings.modelId, settings.voice),
        },
      ]}
    />
  );
  const feedback = (
    <>
      {error ? <InlineNotice kind="error">The lesson voice was not saved: {error}</InlineNotice> : null}
      {voices !== "closed" ? (
        <SpeechVoicesDialog
          settings={settings}
          tryFirst={voices === "try"}
          onChanged={setSettings}
          onClose={() => setVoices("closed")}
        />
      ) : null}
    </>
  );

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
      {clones && saved.length === 0 ? (
        <div className={fieldClass}>
          <span>Voice</span>
          {voiceSelect}
        </div>
      ) : voiceSelect ? (
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
