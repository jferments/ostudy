# ADR 0062: Free voices read aloud on the device

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

Lessons, lectures, podcasts and Tutor replies were read aloud only through
OpenRouter (Drama 3 or Kokoro), so listening cost money and needed an account.
Set up AI tells learners that what they make themselves is free and that AI
costs money only when it reads, writes or speaks. The maintainer asked for free
voices that run locally, naming Kokoro and KittenTTS (2026-10-26), and for a
short sample of two voices from each model in the repository so the learner
can hear them and choose which one to install (2026-10-27).

sherpa-onnx `2.0.0`, already linked for speech input (ADR 0061), runs both
models, but 2.0 removed eSpeak NG. Its English Kokoro and KittenTTS configs
then speak only words in a lexicon file and skip the rest, which loses exactly
the technical words study material is full of ("basolateral" in a physiology
test sentence). sherpa-onnx accepts phonemes as Unicode code points instead of
text, so the phonemes can come from elsewhere. eSpeak NG itself is GPL-3.0.

## Decision

- Two voice models are offered, neither bundled: **Kokoro** v1.0, 8-bit
  (Apache-2.0, 142 MB, 14 American and British English voices offered), and
  **KittenTTS** nano v0.2, 16-bit (Apache-2.0, 24 MB, 8 voices). Each is
  pinned to a Hugging Face revision of the sherpa-onnx conversions, with every
  file's size and SHA-256.
- Text becomes phonemes with loqui-g2p `0.4.0` (MIT), a Rust port of misaki,
  Kokoro's own English G2P, with misaki's lexicons and a small embedded neural
  model for words they do not hold. KittenTTS was trained on eSpeak's
  phonemes, so misaki's one-letter diphthongs are spelled as eSpeak writes
  them for it. Phonemes are cut into sentences of at most 400 characters
  before they reach the model. Both models then read a physiology sentence
  that Moonshine transcribed back with every technical word right; the
  repository test does the same when the models are present.
- A device voice is a speech model in the same catalog as the provider ones
  (`device/kokoro-v1.0-int8`, `device/kitten-nano-v0.2`), chosen in the Voice
  row as **This device (free)**. It needs no connection, is not accounted as
  an AI request, and its pieces are kept like any other narration: managed
  WAV assets keyed by text, engine `device`, model, voice and format `wav`.
  Lesson voices, podcast hosts and Tutor replies go through the same path.
- **Free voices on this device** plays a sample of each model, about ten
  seconds of two of its voices, from `public/voices/device/`. The samples were
  made with these models and phonemes. **Download and use** downloads the
  model through the speech input installer, now shared and tracking each
  model's progress, into `speech-models/<id>` under the application data
  folder, outside the library and its backups. That installer stays the one
  HTTP client outside the AI gateway.
- On a 4-core cloud CPU, Kokoro took about 11 s to speak a 9-second sentence
  and KittenTTS about 3 s, so device voices speak pieces of at most 600
  characters, which start playing sooner.
- Windows has no device voices, for the reason it has no speech input.

## Consequences

- No migration and no schema change. Existing rows are read as before; a
  saved device voice is one `speech_settings` row with no connection, which
  an older build does not know and replaces with its default voice.
- A lesson keeps the voice it was first read in. A lesson first read through
  OpenRouter keeps that voice while a connection exists, so choosing a device
  voice changes lessons not yet read; a lesson whose provider voice has no
  connection left takes the chosen voice.
- WAV is larger than the providers' MP3: about 2.9 MB a minute at 24 kHz.
  Encoding to a smaller format is left for later.
- The device voices read English only; British voices use misaki's British
  lexicon.
