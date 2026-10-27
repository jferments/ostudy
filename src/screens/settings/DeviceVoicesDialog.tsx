import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import {
  getDeviceVoices,
  installDeviceVoice,
  saveSpeechSettings,
  type DeviceVoices,
  type DeviceVoiceStatus,
  type SpeechSettings,
} from "../../ipc/speech";
import { playbackErrorMessage } from "../../media/managedPlayback";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Badge } from "../../ui/primitives/Badge";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";

/** How often a running download's progress is read. */
export const DOWNLOAD_PROGRESS_INTERVAL_MS = 500;

/**
 * The free voices that read aloud on this device. Each model has a sample of
 * two of its voices to listen to before choosing; choosing one downloads it
 * once and makes it the lesson voice.
 */
export function DeviceVoicesDialog({
  settings,
  onChanged,
  onClose,
}: {
  settings: SpeechSettings;
  onChanged: (settings: SpeechSettings) => void;
  onClose: () => void;
}) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [voices, setVoices] = useState<DeviceVoices | null>(null);
  const [playing, setPlaying] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const current = useRef(settings);
  current.current = settings;

  const downloading = voices?.models.some((model) => model.installingBytes !== null) ?? false;
  useEffect(() => {
    let live = true;
    const read = () =>
      void getDeviceVoices().then(
        (next) => { if (live) setVoices(next); },
        (failure: unknown) => { if (live) setError(getErrorMessage(failure)); },
      );
    read();
    const timer = downloading ? window.setInterval(read, DOWNLOAD_PROGRESS_INTERVAL_MS) : undefined;
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [downloading]);

  const stop = () => {
    const audio = audioRef.current;
    if (audio) {
      audio.onended = null;
      audio.pause();
    }
    setPlaying(null);
  };
  useEffect(() => stop, []);

  const play = (model: DeviceVoiceStatus) => {
    const audio = audioRef.current;
    if (!audio) return;
    stop();
    setPlaying(model.modelId);
    audio.src = deviceSample(model.sample);
    audio.onended = stop;
    audio.play().catch((failure: unknown) => {
      stop();
      setError(playbackErrorMessage(failure));
    });
  };

  const use = (model: DeviceVoiceStatus) => {
    const speech = current.current.models.find((candidate) => candidate.id === model.modelId);
    if (!speech) return;
    setBusy(model.modelId);
    setError(null);
    const chosen = model.installed
      ? Promise.resolve()
      : installDeviceVoice(model.modelId).then(setVoices);
    chosen
      .then(() => {
        const latest = current.current;
        return saveSpeechSettings(latest.profileId, speech.id, speech.defaultVoice, latest.revision);
      })
      .then(onChanged, (failure: unknown) => {
        setError(getErrorMessage(failure));
        void getDeviceVoices().then(setVoices, () => undefined);
      })
      .finally(() => setBusy(null));
    // The download's own progress shows while it runs.
    window.setTimeout(() => void getDeviceVoices().then(setVoices, () => undefined), 100);
  };

  return (
    <Dialog
      title="Free voices on this device"
      description="These voices read lessons, podcasts and Tutor replies aloud on this device. They are free, need no account, and nothing you study leaves the device. Listen to the sample first; the voices download once."
      width="small"
      onClose={onClose}
      actions={
        <Button variant="primary" className="tw:rounded-full" onClick={onClose}>
          Done
        </Button>
      }
    >
      <audio ref={audioRef} hidden />
      {voices && !voices.supported ? (
        <InlineNotice kind="info">Voices on this device are not available in this build yet.</InlineNotice>
      ) : null}
      {voices?.supported ? (
        <ul role="list" aria-label="Voices on this device" className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0">
          {voices.models.map((model) => {
            const inUse = settings.modelId === model.modelId;
            const received = model.installingBytes;
            return (
              <li
                key={model.modelId}
                className="tw:grid tw:min-w-0 tw:gap-2 tw:rounded-md tw:border tw:border-solid tw:border-border tw:p-3"
              >
                <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2">
                  <Button
                    size="small"
                    variant="ghost"
                    aria-label={playing === model.modelId ? `Stop the ${model.label} sample` : `Listen to ${model.label}`}
                    onClick={() => (playing === model.modelId ? stop() : play(model))}
                  >
                    <Icon name={playing === model.modelId ? "stop" : "listen"} size={16} />
                  </Button>
                  <span className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
                    <strong className="tw:font-semibold">{model.label}</strong>
                    <span className="tw:text-small tw:text-muted-foreground">
                      {model.installed ? "Downloaded" : `${megabytes(model.downloadBytes)} download`}
                    </span>
                  </span>
                  {inUse && model.installed ? (
                    <Badge tone="success">In use</Badge>
                  ) : (
                    <Button
                      size="small"
                      variant={model.installed ? "secondary" : "primary"}
                      disabled={busy !== null || received !== null}
                      onClick={() => use(model)}
                    >
                      {model.installed ? "Use" : "Download and use"}
                    </Button>
                  )}
                </div>
                <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{model.summary}</p>
                {received !== null ? (
                  <Progress
                    value={received}
                    max={model.downloadBytes}
                    label={`Downloading ${model.label}`}
                    valueText={`${megabytes(received)} of ${megabytes(model.downloadBytes)}`}
                    text={<span>Downloading: {megabytes(received)} of {megabytes(model.downloadBytes)}</span>}
                  />
                ) : null}
                {model.installError && received === null && busy !== model.modelId ? (
                  <InlineNotice kind="error">{model.installError}</InlineNotice>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : null}
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    </Dialog>
  );
}

/** Where the app keeps a device voice's sample. */
export function deviceSample(sample: string): string {
  return `${import.meta.env.BASE_URL}${sample.replace(/^\//u, "")}`;
}

function megabytes(bytes: number): string {
  return `${Math.max(1, Math.round(bytes / 1_000_000))} MB`;
}
