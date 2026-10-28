import { useEffect, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import {
  getDeviceVoices,
  getSpeechSettings,
  saveSpeechSettings,
  type DeviceVoices,
  type SpeechModel,
  type SpeechSettings,
} from "../../ipc/speech";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { Button } from "../../ui/primitives/Button";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TASK_GRID, TaskOffRow } from "./DefaultAiModelSettings";
import { DeviceVoicesDialog } from "./DeviceVoicesDialog";
import { SpeechVoicesDialog } from "./SpeechVoicesDialog";
import { SwitchRow } from "./SettingsControls";

const fieldClass = "tw:grid tw:min-w-0 tw:gap-2 tw:text-label tw:font-medium tw:text-foreground";
/** The Service choice that speaks on this device. */
const ON_DEVICE = "device";

/**
 * The Voice row of "Default for each task": the connection, model and voice
 * that read lessons aloud. "This device" speaks for free with a voice model
 * downloaded once (the Free voices dialog); an OpenRouter connection speaks
 * with its models. A model with preset voices offers its list; a model that
 * copies voices offers the voices that come with ostudy and the learner's
 * saved ones, managed in the Lesson voices dialog. It reloads when the AI
 * connections change, since only an OpenRouter connection can speak online.
 */
export function LessonVoiceSettings({
  connectionsKey,
  onAddOpenRouter,
  lectureAudio,
}: {
  connectionsKey: unknown;
  /** Offered when no OpenRouter connection can speak. */
  onAddOpenRouter?: () => void;
  /** Whether new lessons get their lecture audio made, shown once a voice is chosen. */
  lectureAudio?: { checked: boolean; onChange: (checked: boolean) => void };
}) {
  const wide = useLayoutMode().layoutMode === "wide";
  const lane = useRequestLane();
  const [settings, setSettings] = useState<SpeechSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [voices, setVoices] = useState<"closed" | "open" | "try">("closed");
  const [deviceDialog, setDeviceDialog] = useState(false);
  const [device, setDevice] = useState<DeviceVoices | null>(null);

  useEffect(() => {
    void latestResult(lane, getSpeechSettings).then(
      (value) => {
        if (value !== undefined) setSettings(value);
      },
      (failure: unknown) => setError(getErrorMessage(failure)),
    );
  }, [lane, connectionsKey]);

  // Which voices on the device are downloaded; read again when the dialog closes.
  useEffect(() => {
    if (deviceDialog) return;
    let live = true;
    void getDeviceVoices().then(
      (value) => { if (live) setDevice(value); },
      () => undefined,
    );
    return () => { live = false; };
  }, [deviceDialog]);

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
  const onDevice = model?.onDevice === true;
  const deviceSupported = device?.supported === true;
  if (settings.connections.length === 0 && onAddOpenRouter && !deviceSupported && !onDevice) {
    return <TaskOffRow task="voice" label="Voice" wide={wide} onAddOpenRouter={onAddOpenRouter} />;
  }
  const installed = (candidate: SpeechModel) =>
    device?.models.some((status) => status.modelId === candidate.id && status.installed) === true;
  const shownModels = settings.models.filter((candidate) => candidate.onDevice === onDevice);
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

  /** The model and voice an OpenRouter connection starts with: Drama 3 and its first voice. */
  const online = (): [string, string] => {
    const remote = settings.models.filter((candidate) => !candidate.onDevice);
    const next = remote.find((candidate) => candidate.clonesVoices) ?? remote[0];
    if (!next) return [settings.modelId, settings.voice];
    const own = settings.voices.find((voice) => voice.state !== "tried" && voice.modelId === next.id);
    return [next.id, next.clonesVoices ? (own?.voice ?? "") : next.defaultVoice];
  };
  const chooseModel = (next: SpeechModel) => {
    if (next.onDevice && !installed(next)) {
      setDeviceDialog(true);
      return;
    }
    // A model that copies voices starts with its first voice, the default.
    const own = settings.voices.find((voice) => voice.state !== "tried" && voice.modelId === next.id);
    save(settings.profileId, next.id, next.clonesVoices ? (own?.voice ?? "") : next.defaultVoice);
  };

  const connectionSelect = (
    <NativeSelect
      aria-label="Lesson voice connection"
      value={onDevice ? ON_DEVICE : (settings.profileId ?? "")}
      disabled={saving}
      onChange={(event) => {
        const value = event.currentTarget.value;
        if (value === ON_DEVICE) {
          const ready = settings.models.find((candidate) => candidate.onDevice && installed(candidate));
          if (ready) chooseModel(ready);
          else setDeviceDialog(true);
        } else if (onDevice) {
          save(value || null, ...online());
        } else {
          save(value || null, settings.modelId, settings.voice);
        }
      }}
    >
      {deviceSupported || onDevice ? <option value={ON_DEVICE}>This device (free)</option> : null}
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
        if (next) chooseModel(next);
      }}
    >
      {shownModels.map((candidate) => (
        <option key={candidate.id} value={candidate.id}>
          {candidate.onDevice && !installed(candidate) ? `${candidate.label} (not downloaded)` : candidate.label}
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
        ...(deviceSupported || onDevice
          ? [{ id: "device", label: "Free voices on this device…", onSelect: () => setDeviceDialog(true) }]
          : []),
        ...(settings.connections.length === 0 && onAddOpenRouter
          ? [{ id: "openrouter", label: "Add OpenRouter…", onSelect: onAddOpenRouter }]
          : []),
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
          disabled: !settings.profileId && !onDevice,
          onSelect: () => (onDevice ? save(null, ...online()) : save(null, settings.modelId, settings.voice)),
        },
      ]}
    />
  );
  const feedback = (
    <>
      {lectureAudio && (settings.profileId || (onDevice && model && installed(model))) ? (
        <SwitchRow
          label="Make lecture audio with new lessons"
          checked={lectureAudio.checked}
          onChange={lectureAudio.onChange}
        />
      ) : null}
      {error ? <InlineNotice kind="error">The lesson voice was not saved: {error}</InlineNotice> : null}
      {deviceDialog ? (
        <DeviceVoicesDialog settings={settings} onChanged={setSettings} onClose={() => setDeviceDialog(false)} />
      ) : null}
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
