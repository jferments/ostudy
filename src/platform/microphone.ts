import workletUrl from "./microphoneCapture.worklet.js?url";

/** The rate speech recognition works at; recordings are brought down to it. */
const SPEECH_SAMPLE_RATE = 16_000;
const LAST_BLOCK_TIMEOUT_MS = 500;

export interface RecordedAudio {
  /** Mono 16-bit little-endian PCM. */
  samplesBase64: string;
  sampleRate: number;
  seconds: number;
}

export interface MicrophoneRecording {
  /** Ends the recording and returns what was heard. */
  stop(): Promise<RecordedAudio>;
  /** Ends the recording and drops it. */
  cancel(): void;
}

export function microphoneAvailable(): boolean {
  return typeof navigator !== "undefined"
    && typeof navigator.mediaDevices?.getUserMedia === "function"
    && typeof AudioContext !== "undefined"
    && typeof AudioWorkletNode !== "undefined";
}

/**
 * Records the microphone through the webview. `onLevel` receives a loudness
 * between 0 and 1 about ten times a second, for a meter. The recording stays
 * in memory and is handed to native code by the caller; nothing is stored.
 */
export async function startRecording(onLevel: (level: number) => void): Promise<MicrophoneRecording> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
  });
  let context: AudioContext;
  try {
    // Recording at the speech rate saves resampling where the webview allows it.
    context = new AudioContext({ sampleRate: SPEECH_SAMPLE_RATE });
  } catch {
    context = new AudioContext();
  }
  const release = () => {
    for (const track of stream.getTracks()) track.stop();
    void context.close().catch(() => undefined);
  };
  let node: AudioWorkletNode;
  try {
    await context.audioWorklet.addModule(workletUrl);
    node = new AudioWorkletNode(context, "ostudy-microphone-capture");
  } catch (error) {
    release();
    throw error;
  }
  const blocks: Float32Array[] = [];
  let finishLast: ((block: Float32Array) => void) | null = null;
  node.port.onmessage = (event: MessageEvent<Float32Array | { last: Float32Array }>) => {
    if (event.data instanceof Float32Array) {
      blocks.push(event.data);
      onLevel(level(event.data));
    } else {
      finishLast?.(event.data.last);
    }
  };
  const source = context.createMediaStreamSource(stream);
  // Some engines only run a node that reaches the output; a silent gain keeps
  // the recording from being played back.
  const silence = context.createGain();
  silence.gain.value = 0;
  source.connect(node);
  node.connect(silence);
  silence.connect(context.destination);

  let ended = false;
  const end = () => {
    ended = true;
    source.disconnect();
    node.disconnect();
    release();
  };
  return {
    async stop() {
      if (ended) throw new Error("The recording has already ended.");
      const last = await new Promise<Float32Array>((resolve) => {
        finishLast = resolve;
        node.port.postMessage("flush");
        window.setTimeout(() => resolve(new Float32Array(0)), LAST_BLOCK_TIMEOUT_MS);
      });
      const rate = context.sampleRate;
      end();
      const samples = resample(join([...blocks, last]), rate, SPEECH_SAMPLE_RATE);
      const sampleRate = rate > SPEECH_SAMPLE_RATE ? SPEECH_SAMPLE_RATE : rate;
      return { samplesBase64: toPcmBase64(samples), sampleRate, seconds: samples.length / sampleRate };
    },
    cancel() {
      if (!ended) end();
    },
  };
}

/** Root-mean-square loudness, scaled so ordinary speech fills most of a meter. */
export function level(block: Float32Array): number {
  let sum = 0;
  for (const sample of block) sum += sample * sample;
  const rms = Math.sqrt(sum / Math.max(1, block.length));
  return Math.min(1, rms * 8);
}

function join(blocks: Float32Array[]): Float32Array {
  const joined = new Float32Array(blocks.reduce((total, block) => total + block.length, 0));
  let offset = 0;
  for (const block of blocks) {
    joined.set(block, offset);
    offset += block.length;
  }
  return joined;
}

/**
 * Brings a recording down to `target` by averaging the samples each output
 * sample covers, which also filters what the lower rate cannot hold. Lower
 * rates are left for native code to raise.
 */
export function resample(samples: Float32Array, from: number, target: number): Float32Array {
  if (from <= target) return samples;
  const ratio = from / target;
  const output = new Float32Array(Math.floor(samples.length / ratio));
  for (let index = 0; index < output.length; index += 1) {
    const start = Math.floor(index * ratio);
    const end = Math.min(samples.length, Math.floor((index + 1) * ratio));
    let sum = 0;
    for (let position = start; position < end; position += 1) sum += samples[position] ?? 0;
    output[index] = sum / Math.max(1, end - start);
  }
  return output;
}

export function toPcmBase64(samples: Float32Array): string {
  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  for (let index = 0; index < samples.length; index += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[index] ?? 0));
    view.setInt16(index * 2, clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff, true);
  }
  let binary = "";
  // Converting in slices keeps each call's argument list small.
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}
