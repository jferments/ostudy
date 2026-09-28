# oStudy Flashcards ChatGPT plugin

This directory is the portable, **skills-only** ChatGPT plugin for generating oStudy flashcard import files. It deliberately has no `mcp.json`, remote service, authentication flow, provider credential, or direct access to an oStudy library.

## What it does

The `generate-ostudy-flashcards` skill can use:

- a topic or natural-language prompt;
- text already present in the conversation; or
- documents/files that ChatGPT can already read in the current conversation.

ChatGPT authors the learning material. The bundled dependency-free Python writer then validates the generated v2 transport structure and writes one UTF-8 `.ostudy.json` artifact. The user imports that file through **Flashcards → Decks → Import deck from file**.

The native Rust importer remains authoritative. This package never bypasses oStudy validation, scheduling, duplicate handling, or transactional persistence.

## Package layout

```text
plugin.json
skills/
  generate-ostudy-flashcards/
    SKILL.md
    references/
      authoring-guide.md
      ostudy-flashcards-v2.schema.json
      all-card-types.ostudy.json
    scripts/
      write_ostudy.py
```

The reference schema is an exact bundled copy of `docs/ostudy-flashcards-v2.schema.json`; repository tests prevent the two copies from drifting.

## Local ChatGPT desktop testing

The repository includes `.agents/plugins/marketplace.json`, which exposes this package as **oStudy Flashcards** from the `oStudy Development` repo marketplace. On a supported ChatGPT desktop/Work or Codex plugin surface:

1. check out the branch/commit being tested;
2. restart the ChatGPT desktop app so it refreshes the repo marketplace;
3. open the Plugins Directory and choose the **oStudy Development** source;
4. install **oStudy Flashcards**;
5. start a new chat and invoke the plugin directly or ask for an oStudy deck.

Local/repo marketplaces are development distribution, not publication to the universal public Plugins Directory. Public distribution uses OpenAI's plugin submission flow with the plugin root in this directory.

## Direct writer testing

Validate the committed all-types fixture:

```bash
python3 integrations/ostudy-flashcards/skills/generate-ostudy-flashcards/scripts/write_ostudy.py \
  --input integrations/ostudy-flashcards/skills/generate-ostudy-flashcards/references/all-card-types.ostudy.json \
  --validate-only
```

Write a normalized artifact from a generated draft:

```bash
python3 integrations/ostudy-flashcards/skills/generate-ostudy-flashcards/scripts/write_ostudy.py \
  --input /path/to/draft.json \
  --output /tmp/example.ostudy.json
```

A validation failure exits nonzero and does not intentionally publish a replacement output.

## Compatibility

This plugin writes `ostudy-flashcards` **format version 2**. It can generate Basic, Cloze, single-choice, select-all, true/false, matching, grouping, and ordering material. Generated JSON does not carry image-occlusion assets, schedules, review history, internal IDs, learner analytics, credentials, or library settings.

See `docs/reference/generated-json-import.md` for the native import contract.
