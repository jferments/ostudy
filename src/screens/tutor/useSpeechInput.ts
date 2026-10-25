import { useCallback, useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import {
  getSpeechInputStatus,
  installSpeechInputModel,
  transcribeSpeech,
  type SpeechInputStatus,
} from "../../ipc/speechInput";
import { microphoneAvailable, startRecording, type MicrophoneRecording } from "../../platform/microphone";
import { useConfirm } from "../../ui/overlays/overlayContext";

/** Matches the native limit; a longer recording stops itself here. */
export const MAX_RECORDING_SECONDS = 120;
const DOWNLOAD_POLL_MS = 300;

export type SpeechInputPhase = "idle" | "downloading" | "recording" | "transcribing";

export interface SpeechInput {
  /** False where speech input cannot work, so the microphone is not offered. */
  available: boolean;
  phase: SpeechInputPhase;
  /** Loudness between 0 and 1 while recording. */
  level: number;
  /** Whole seconds recorded so far. */
  seconds: number;
  /** Share of the model downloaded, between 0 and 1. */
  downloaded: number;
  error: string | null;
  /** Starts recording, or stops and transcribes a running recording. */
  toggle: () => void;
  /** Drops a running recording. */
  cancel: () => void;
}

/**
 * The Tutor's microphone. The first use asks before downloading the speech
 * model; after that a tap records and a second tap hands the recording to
 * native recognition, whose text goes to `onText`.
 */
export function useSpeechInput(onText: (text: string) => void): SpeechInput {
  const confirm = useConfirm();
  const [status, setStatus] = useState<SpeechInputStatus | null>(null);
  const [phase, setPhase] = useState<SpeechInputPhase>("idle");
  const [level, setLevel] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const recording = useRef<MicrophoneRecording | null>(null);
  // A tap on stop while the microphone was still starting.
  const stopRequested = useRef(false);
  // Each start or cancel moves this on, so a start that finishes late is dropped.
  const attempt = useRef(0);
  const mounted = useRef(true);
  const onTextRef = useRef(onText);
  useEffect(() => {
    onTextRef.current = onText;
  });

  useEffect(() => {
    mounted.current = true;
    let current = true;
    void getSpeechInputStatus().then(
      (value) => { if (current) setStatus(value); },
      () => undefined,
    );
    return () => {
      current = false;
      mounted.current = false;
      recording.current?.cancel();
      recording.current = null;
    };
  }, []);

  // Download progress is read from native code while the model downloads.
  useEffect(() => {
    if (phase !== "downloading") return;
    const timer = window.setInterval(() => {
      void getSpeechInputStatus().then((value) => { if (mounted.current) setStatus(value); }, () => undefined);
    }, DOWNLOAD_POLL_MS);
    return () => window.clearInterval(timer);
  }, [phase]);

  const stop = useCallback(async () => {
    const active = recording.current;
    if (!active) {
      stopRequested.current = true;
      return;
    }
    recording.current = null;
    setPhase("transcribing");
    setLevel(0);
    try {
      const audio = await active.stop();
      const transcript = await transcribeSpeech(audio.samplesBase64, audio.sampleRate);
      if (!mounted.current) return;
      if (transcript.text) onTextRef.current(transcript.text);
      else setError("Nothing was heard. Try again a little closer to the microphone.");
    } catch (failure) {
      if (mounted.current) setError(getErrorMessage(failure));
    } finally {
      if (mounted.current) setPhase("idle");
    }
  }, []);

  // The timer counts the recording's seconds and ends it at the limit.
  useEffect(() => {
    if (phase !== "recording") return;
    const started = Date.now();
    const timer = window.setInterval(() => {
      const elapsed = Math.floor((Date.now() - started) / 1000);
      setSeconds(elapsed);
      if (elapsed >= MAX_RECORDING_SECONDS) void stop();
    }, 250);
    return () => window.clearInterval(timer);
  }, [phase, stop]);

  const record = useCallback(async () => {
    stopRequested.current = false;
    const mine = ++attempt.current;
    setSeconds(0);
    setPhase("recording");
    try {
      const started = await startRecording((value) => { if (mounted.current) setLevel(value); });
      if (!mounted.current || mine !== attempt.current) {
        started.cancel();
        return;
      }
      recording.current = started;
      if (stopRequested.current) void stop();
    } catch (failure) {
      if (!mounted.current || mine !== attempt.current) return;
      setPhase("idle");
      setError(
        failure instanceof DOMException && failure.name === "NotAllowedError"
          ? "oStudy isn't allowed to use the microphone."
          : failure instanceof DOMException && failure.name === "NotFoundError"
            ? "No microphone was found."
            : getErrorMessage(failure),
      );
    }
  }, [stop]);

  const install = useCallback(async (current: SpeechInputStatus) => {
    const megabytes = Math.round(current.downloadBytes / 1_000_000);
    const accepted = await confirm({
      title: "Download speech input?",
      description: `oStudy turns speech into text on this device with ${current.modelLabel}. The first use downloads ${megabytes} MB.`,
      confirmLabel: "Download",
    });
    if (!accepted || !mounted.current) return;
    setPhase("downloading");
    try {
      const installed = await installSpeechInputModel();
      if (!mounted.current) return;
      setStatus(installed);
      if (installed.installError) setError(installed.installError);
    } catch (failure) {
      if (mounted.current) setError(getErrorMessage(failure));
    } finally {
      if (mounted.current) setPhase("idle");
    }
  }, [confirm]);

  const toggle = useCallback(() => {
    if (phase === "recording") {
      void stop();
      return;
    }
    if (phase !== "idle" || !status) return;
    setError(null);
    if (status.installed) void record();
    else void install(status);
  }, [install, phase, record, status, stop]);

  const cancel = useCallback(() => {
    attempt.current += 1;
    recording.current?.cancel();
    recording.current = null;
    setLevel(0);
    if (phase === "recording") setPhase("idle");
  }, [phase]);

  return {
    available: Boolean(status?.supported) && microphoneAvailable(),
    phase,
    level,
    seconds,
    downloaded: status && status.downloadBytes > 0 ? (status.installingBytes ?? 0) / status.downloadBytes : 0,
    error,
    toggle,
    cancel,
  };
}
