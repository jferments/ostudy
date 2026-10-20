import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import {
  keepSpeechVoice,
  removeSpeechVoice,
  renameSpeechVoice,
  trySpeechVoice,
  type SpeechSettings,
  type SpeechVoice,
} from "../../ipc/speech";
import { createManagedMediaObjectUrl, revokeManagedMediaObjectUrl } from "../../media/managedPlayback";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { Badge } from "../../ui/primitives/Badge";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { TextField } from "../../ui/primitives/TextField";

/**
 * The learner's own lesson voices for a model that copies a voice: listen to
 * each, choose one, rename or remove it, and try new ones until one is worth
 * keeping. Lessons keep the voice they were first read in.
 */
export function SpeechVoicesDialog({
  settings,
  tryFirst,
  onChanged,
  onClose,
}: {
  settings: SpeechSettings;
  /** Start trying a voice as the dialog opens. */
  tryFirst: boolean;
  onChanged: (settings: SpeechSettings) => void;
  onClose: () => void;
}) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const url = useRef<string | null>(null);
  const [playing, setPlaying] = useState<string | null>(null);
  const [busy, setBusy] = useState<"trying" | "saving" | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [description, setDescription] = useState("");
  const model = settings.models.find((candidate) => candidate.id === settings.modelId);
  const saved = settings.voices.filter((voice) => voice.state !== "tried" && voice.modelId === settings.modelId);
  const tried = settings.voices.find((voice) => voice.state === "tried" && voice.modelId === settings.modelId);

  const stop = () => {
    const audio = audioRef.current;
    if (audio) {
      audio.onended = null;
      audio.pause();
    }
    revokeManagedMediaObjectUrl(url.current);
    url.current = null;
    setPlaying(null);
  };

  const play = (voice: SpeechVoice) => {
    stop();
    setPlaying(voice.id);
    // A voice that comes with ostudy plays from the app's own files.
    const source =
      voice.assetId === null
        ? Promise.resolve({ src: builtInSample(voice.id), owned: false })
        : createManagedMediaObjectUrl(voice.assetId, "audio").then((src) => ({ src, owned: true }));
    source.then(
      ({ src: made, owned }) => {
        const audio = audioRef.current;
        if (!audio) {
          if (owned) revokeManagedMediaObjectUrl(made);
          return;
        }
        url.current = owned ? made : null;
        audio.src = made;
        audio.onended = stop;
        audio.play().catch((failure: unknown) => {
          stop();
          setError(getErrorMessage(failure));
        });
      },
      (failure: unknown) => {
        setPlaying(null);
        setError(getErrorMessage(failure));
      },
    );
  };

  const change = (
    action: () => Promise<SpeechSettings>,
    kind: "trying" | "saving",
    after?: (next: SpeechSettings) => void,
  ) => {
    setBusy(kind);
    setError(null);
    action()
      .then(
        (next) => {
          onChanged(next);
          after?.(next);
        },
        (failure: unknown) => setError(getErrorMessage(failure)),
      )
      .finally(() => setBusy(null));
  };

  const tryAnother = () => {
    stop();
    change(
      () => trySpeechVoice(description.trim() || null),
      "trying",
      (next) => {
        const made = next.voices.find((voice) => voice.state === "tried");
        if (made) play(made);
      },
    );
  };

  const started = useRef(false);
  useEffect(() => {
    if (tryFirst && !started.current) {
      started.current = true;
      tryAnother();
    }
    return stop;
    // Runs once, as the dialog opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const playButton = (voice: SpeechVoice) => (
    <Button
      size="small"
      variant="ghost"
      aria-label={playing === voice.id ? `Stop ${voice.name}` : `Listen to ${voice.name}`}
      onClick={() => (playing === voice.id ? stop() : play(voice))}
    >
      <Icon name={playing === voice.id ? "stop" : "listen"} size={16} />
    </Button>
  );

  return (
    <Dialog
      title="Lesson voices"
      description={`Lessons are read in the voice you choose here, and each lesson keeps the voice it was first read in. ${model?.label ?? "This model"} makes a new voice every time you try one.`}
      width="small"
      busy={busy !== null}
      onClose={onClose}
      actions={
        <Button variant="primary" className="tw:rounded-full" onClick={onClose}>
          Done
        </Button>
      }
    >
      <audio ref={audioRef} hidden />
      {saved.length > 0 ? (
        <ul role="list" aria-label="Your voices" className="tw:m-0 tw:grid tw:list-none tw:gap-1 tw:p-0">
          {saved.map((voice) => {
            const inUse = voice.voice === settings.voice;
            return (
              <li key={voice.id} className="tw:flex tw:min-h-11 tw:min-w-0 tw:items-center tw:gap-2">
                {playButton(voice)}
                {renaming?.id === voice.id ? (
                  <form
                    className="tw:flex tw:min-w-0 tw:flex-1 tw:items-center tw:gap-2"
                    onSubmit={(event) => {
                      event.preventDefault();
                      change(
                        () => renameSpeechVoice(voice.id, renaming.name),
                        "saving",
                        () => setRenaming(null),
                      );
                    }}
                  >
                    <TextField
                      aria-label={`Name for ${voice.name}`}
                      autoFocus
                      maxLength={60}
                      value={renaming.name}
                      onChange={(event) =>
                        setRenaming({
                          id: voice.id,
                          name: event.currentTarget.value,
                        })
                      }
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.stopPropagation();
                          setRenaming(null);
                        }
                      }}
                    />
                    <Button size="small" type="submit" disabled={busy !== null || !renaming.name.trim()}>
                      Save
                    </Button>
                  </form>
                ) : (
                  <span className="tw:min-w-0 tw:flex-1 tw:truncate">{voice.name}</span>
                )}
                {inUse ? (
                  <Badge tone="success">In use</Badge>
                ) : (
                  <Button
                    size="small"
                    disabled={busy !== null}
                    onClick={() => change(() => keepSpeechVoice(voice.id), "saving")}
                  >
                    Use
                  </Button>
                )}
                {voice.state === "builtIn" ? null : (
                  <OverflowMenu
                    label={`More for ${voice.name}`}
                    disabled={busy !== null}
                    items={[
                      {
                        id: "rename",
                        label: "Rename…",
                        onSelect: () => setRenaming({ id: voice.id, name: voice.name }),
                      },
                      {
                        id: "remove",
                        label: "Remove",
                        disabled: inUse,
                        onSelect: () => change(() => removeSpeechVoice(voice.id), "saving"),
                      },
                    ]}
                  />
                )}
              </li>
            );
          })}
        </ul>
      ) : null}
      {tried ? (
        <div className="tw:flex tw:min-h-11 tw:min-w-0 tw:items-center tw:gap-2 tw:rounded-md tw:bg-muted tw:px-2 tw:py-1">
          {playButton(tried)}
          <span className="tw:min-w-0 tw:flex-1 tw:truncate">{tried.name}</span>
          <Button
            size="small"
            variant="primary"
            disabled={busy !== null}
            onClick={() => change(() => keepSpeechVoice(tried.id), "saving")}
          >
            Keep this voice
          </Button>
        </div>
      ) : null}
      <form
        className="tw:grid tw:gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          if (busy === null) tryAnother();
        }}
      >
        <label className="tw:grid tw:min-w-0 tw:gap-1 tw:text-label tw:font-medium">
          <span>Describe the voice (optional)</span>
          <TextField
            maxLength={120}
            placeholder="For example: a calm older woman with a British accent"
            value={description}
            onChange={(event) => setDescription(event.currentTarget.value.replace(/[[\]]/gu, ""))}
          />
        </label>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
          <Button type="submit" disabled={busy !== null}>
            <Icon name="sparkles" size={16} /> {busy === "trying" ? "Making a voice…" : "Try another voice"}
          </Button>
          <span className="tw:text-small tw:text-muted-foreground">
            Each try is one short sample, well under a cent.
          </span>
        </div>
      </form>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    </Dialog>
  );
}

/** Where a voice that comes with ostudy keeps its sample. */
export function builtInSample(id: string): string {
  return `${import.meta.env.BASE_URL}voices/${id}.mp3`;
}
