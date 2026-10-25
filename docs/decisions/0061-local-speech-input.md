# ADR 0061: Speech input is recognized on the device

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

The maintainer asked for a microphone in the Tutor's composer: the learner
speaks, the speech becomes text, and the text is sent like a typed message
(2026-10-25). They asked first for local models that match Whisper's quality
with less latency and fewer resources. On a 4-core 2.1 GHz CPU, through
sherpa-onnx with 8-bit weights, a 16.7-second clip took 0.57 s with Moonshine
v2 base, 1.35 s with Whisper base.en and 3.7 s with Whisper large-v3-turbo.
Cloud transcription would cost money per minute and send the learner's voice
to a provider. The same recognition is meant to serve lecture and podcast
transcripts later.

## Decision

- Recognition runs natively through sherpa-onnx `2.0.0` (Apache-2.0) with
  Moonshine v2 base English (MIT). Audio never goes to an AI provider and no
  provider setting is needed.
- The model (141 MB) is not bundled. The first tap on the microphone asks
  before downloading it from a pinned Hugging Face revision. Each file is
  checked against its published size and SHA-256 while it arrives, into a
  staging folder that is renamed into place only when every file matches. It
  lives under the application data folder in `speech-models/`, outside the
  library and its backups.
- That download is the one HTTP client outside the AI gateway. It contacts no
  provider and sends nothing of the learner's, only GET requests over HTTPS
  for the pinned files, so it does not belong in the gateway, whose client
  refuses the redirect Hugging Face answers with. The repository verifiers
  allow exactly `src-tauri/src/speech_input/install.rs` and check that shape.
- The webview records through `getUserMedia` and an AudioWorklet, brings the
  recording down to 16 kHz and hands Rust mono 16-bit PCM. Rust checks the
  rate and caps the recording at 120 seconds. A recording is held only until
  it has been transcribed.
- The transcript goes into the composer for the learner to read and edit; it
  is not sent by itself.
- sherpa-onnx's libraries come from its own release archives, pinned with
  SHA-256 hashes in `scripts/native-deps-manifest.json` and staged by
  `scripts/install-native-deps.sh`. `SHERPA_ONNX_ARCHIVE_DIR` points the
  crate's build script at that staged archive, so a build never downloads one
  itself and fails when it is missing.
- Linux, macOS and Android have speech input. Windows does not yet: the
  published static libraries use the static C runtime, which does not link
  with the rest of the Windows build. The command reports the build as
  unsupported and the microphone is not shown.
- WebKitGTK refuses media capture unless the window enables it, so the Linux
  main window enables media streams and allows a capture request only for
  audio. Android declares `RECORD_AUDIO`; the webview asks for it at runtime.

## Consequences

- No migration and no library data change.
- Builds need `install-native-deps.sh` (or `--pdfium-only`) before Cargo,
  which `pnpm desktop`, the Android scripts and the check scripts already run.
- Spoken Tutor replies, lecture transcripts and diarization build on this
  model folder and engine and record their own decisions.
