import { useEffect, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import type { LessonSummary } from "../../ipc/lessons";
import {
  PODCAST_CHARACTERS_PER_MINUTE,
  PODCAST_MAX_LESSONS,
  PODCAST_MINUTES,
  writePodcast,
  type Podcast,
} from "../../ipc/podcasts";
import { type SpeechSettings } from "../../ipc/speech";
import { loadVoice } from "./voiceReady";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Slider } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";

/** The hosts, in the order of their voices; the native writer uses the same names. */
export const PODCAST_HOSTS = ["Ada", "Ben"] as const;
/** Each host's voice until the learner picks another, by kind of model. */
const DEFAULT_VOICES = {
  cloned: ["builtin:calm-older-woman", "builtin:relaxed-man"],
  preset: ["af_heart", "am_michael"],
} as const;
/** Lessons picked when the dialog opens. */
const FIRST_PICKED = 3;

interface VoiceChoice {
  voice: string;
  name: string;
}

/** The voices a host can speak in with the speech model in use. */
function voiceChoices(settings: SpeechSettings): VoiceChoice[] {
  const model = settings.models.find((candidate) => candidate.id === settings.modelId);
  if (!model) return [];
  if (!model.clonesVoices) return model.voices.map((voice) => ({ voice, name: voice }));
  return settings.voices
    .filter((voice) => voice.state !== "tried" && voice.modelId === model.id)
    .map((voice) => ({ voice: voice.voice, name: voice.name }));
}

function defaultVoices(settings: SpeechSettings, choices: VoiceChoice[]): [string, string] {
  const model = settings.models.find((candidate) => candidate.id === settings.modelId);
  const wanted = model?.clonesVoices ? DEFAULT_VOICES.cloned : DEFAULT_VOICES.preset;
  const pick = (voice: string, fallback: number) =>
    choices.some((choice) => choice.voice === voice) ? voice : (choices[fallback]?.voice ?? choices[0]?.voice ?? "");
  return [pick(wanted[0], 0), pick(wanted[1], 1)];
}

/**
 * Make a podcast: pick the lessons, the length and each host's voice. Writing
 * the conversation is one request to the AI model; the voices read it as it
 * plays.
 */
export function MakePodcastDialog({
  subjectId,
  lessons,
  onMade,
  onClose,
}: {
  subjectId: string;
  /** The subject's written lessons, in course order. */
  lessons: LessonSummary[];
  onMade: (podcast: Podcast) => void;
  onClose: () => void;
}) {
  const [picked, setPicked] = useState<string[]>(() => lessons.slice(0, FIRST_PICKED).map((lesson) => lesson.id));
  const [minutes, setMinutes] = useState<number>(PODCAST_MINUTES.default);
  const [settings, setSettings] = useState<SpeechSettings | null>(null);
  const [ready, setReady] = useState(true);
  const [voices, setVoices] = useState<[string, string] | null>(null);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    loadVoice().then(
      ({ speech: value, ready: speaks }) => {
        if (!live) return;
        setSettings(value);
        setReady(speaks);
        setVoices(defaultVoices(value, voiceChoices(value)));
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const choices = settings ? voiceChoices(settings) : [];
  const model = settings?.models.find((candidate) => candidate.id === settings.modelId);
  const toggle = (lessonId: string, on: boolean) =>
    setPicked((current) =>
      // Kept in course order, whatever order they were ticked in.
      lessons.map((lesson) => lesson.id).filter((id) => (id === lessonId ? on : current.includes(id))),
    );
  const tooMany = picked.length > PODCAST_MAX_LESSONS;

  const make = () => {
    setWriting(true);
    setError(null);
    writePodcast(subjectId, picked, minutes, voices).then(onMade, (failure: unknown) => {
      setError(getErrorMessage(failure));
      setWriting(false);
    });
  };

  return (
    <Dialog
      title="Make a podcast"
      description="Two hosts talk through the lessons you pick, using only what those lessons say."
      busy={writing}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        if (!writing && picked.length > 0 && !tooMany) make();
      }}
      actions={
        <>
          <Button className="tw:rounded-full" disabled={writing} onClick={onClose}>
            Cancel
          </Button>
          <Button
            type="submit"
            variant="primary"
            className="tw:rounded-full"
            disabled={writing || picked.length === 0 || tooMany}
          >
            <Icon name="sparkles" size={16} /> {writing ? "Writing the podcast…" : "Make podcast"}
          </Button>
        </>
      }
    >
      <fieldset className="tw:m-0 tw:grid tw:gap-1 tw:border-0 tw:p-0">
        <legend className="tw:mb-1 tw:p-0 tw:text-label tw:font-medium">Lessons</legend>
        {lessons.map((lesson, index) => (
          <label key={lesson.id} className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3">
            <Checkbox
              checked={picked.includes(lesson.id)}
              disabled={writing}
              onCheckedChange={(on) => toggle(lesson.id, on)}
            />
            <span className="tw:min-w-0 tw:flex-1 tw:break-words">
              {index + 1} · {lesson.title}
            </span>
          </label>
        ))}
        {tooMany ? (
          <InlineNotice kind="warning">A podcast talks through at most {PODCAST_MAX_LESSONS} lessons.</InlineNotice>
        ) : null}
      </fieldset>
      <div className="tw:grid tw:gap-2">
        <div className="tw:flex tw:items-baseline tw:justify-between tw:text-label tw:font-medium">
          <span>Length</span>
          <span className="tw:tabular-nums">{minutes} min</span>
        </div>
        <Slider
          label="Length in minutes"
          value={minutes}
          min={PODCAST_MINUTES.min}
          max={PODCAST_MINUTES.max}
          disabled={writing}
          onValueChange={setMinutes}
        />
        <div className="tw:flex tw:justify-between tw:text-small tw:text-muted-foreground">
          <span>{PODCAST_MINUTES.min}</span>
          <span>{PODCAST_MINUTES.max}</span>
        </div>
      </div>
      {settings && voices && choices.length > 0 ? (
        <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2">
          {PODCAST_HOSTS.map((host, position) => (
            <label key={host} className="tw:grid tw:min-w-0 tw:gap-1 tw:text-label tw:font-medium">
              <span>{host}'s voice</span>
              <NativeSelect
                value={voices[position]}
                disabled={writing}
                onChange={(event) => {
                  const next: [string, string] = [...voices];
                  next[position] = event.currentTarget.value;
                  setVoices(next);
                }}
              >
                {choices.map((choice) => (
                  <option key={choice.voice} value={choice.voice}>
                    {choice.name}
                  </option>
                ))}
              </NativeSelect>
            </label>
          ))}
        </div>
      ) : null}
      {settings && !ready ? (
        <InlineNotice kind="info">
          {model?.onDevice
            ? `The hosts speak with ${model.label}. Download it in Settings → AI, under Voice, to listen.`
            : "The hosts speak through OpenRouter. Add an OpenRouter connection in Settings, or choose a free voice on this device, to listen."}
        </InlineNotice>
      ) : null}
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
        One request to your AI model writes the conversation, then {model?.label ?? "the voice"} reads about{" "}
        {(minutes * PODCAST_CHARACTERS_PER_MINUTE).toLocaleString()} characters as you listen
        {model?.onDevice ? ", for free" : ""}. The audio is kept, so replays are free.
      </p>
      {error ? <InlineNotice kind="error">The podcast was not written: {error}</InlineNotice> : null}
    </Dialog>
  );
}
