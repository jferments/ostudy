# oStudy Flashcards ChatGPT plugin

This directory is the portable, **skills-only** ChatGPT plugin for generating oStudy flashcard import files. It deliberately has no `mcp.json`, remote service, authentication flow, provider credential, or direct access to an oStudy library.

The primary distribution target is **ChatGPT Web through OpenAI's universal Plugins Directory**. The repository-local marketplace remains an optional development aid; it is not required for the public web plugin.

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
assets/
  logo.png
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

## Build the ChatGPT Web submission ZIP

OpenAI's **Skills only** submission path accepts a ZIP containing one portable plugin root. Build the upload artifact with:

```bash
python3 scripts/build-ostudy-flashcards-plugin.py
```

By default this writes:

```text
dist/ostudy-flashcards-0.1.0.zip
```

The builder is dependency-free and deliberately includes only:

- root `plugin.json`;
- `assets/`;
- `skills/`.

It excludes repository-only material such as `.agents/plugins/marketplace.json`, this README, and the submission dossier. It also rejects MCP/app configuration, unsafe ZIP paths, symbolic links, unsupported archive depth/count/size, and skills-only screenshot configuration. Archive member order and metadata are fixed so identical source produces identical ZIP bytes.

Validate inputs without writing a ZIP:

```bash
python3 scripts/build-ostudy-flashcards-plugin.py --check-only
```

## Submit for ChatGPT Web

The web-first publication path is:

1. ensure the OpenAI Platform submitter has **Apps Management: Write** and a verified developer/business identity;
2. make the public website, support, privacy-policy, and terms URLs live;
3. open the OpenAI plugin submission portal and choose **Create plugin → Skills only**;
4. upload the ZIP produced by `scripts/build-ostudy-flashcards-plugin.py`;
5. use `submission/openai-portal.json` as the reviewed source for listing copy, starter prompts, five positive reviewer cases, three negative reviewer cases, and release notes;
6. resolve any package/security-scan findings without weakening the Skill or validator;
7. submit for review;
8. after approval, publish the plugin so it appears in the universal Plugins Directory used by ChatGPT Web.

The submission dossier intentionally lives **outside** the uploaded ZIP. It is portal-entry material, not model/runtime instructions.

Published skill changes are versioned submission snapshots. Updating the repository alone does not change an already-published ChatGPT Web plugin; create and publish a new plugin version.

## ChatGPT Web manual acceptance after publication

Once the submitted version is installable from the ChatGPT Web Plugins Directory:

1. install **oStudy Flashcards**;
2. start a new chat with the plugin enabled;
3. run a topic-only request and confirm it returns a real downloadable `.ostudy.json`;
4. upload a representative study document and request a source-grounded deck;
5. verify an unrelated summarization request does not trigger deck generation;
6. download the produced artifact and run the bundled writer in `--validate-only` mode;
7. import it into an isolated oStudy development library and inspect card quality/type behavior.

Do not use a production school library for the first import acceptance pass.

## Optional repo-local development marketplace

The repository also includes `.agents/plugins/marketplace.json`, exposing this package as **oStudy Flashcards** from the **oStudy Development** repo marketplace on surfaces that support local/repo marketplaces. That path is optional developer infrastructure and is not the ChatGPT-Web distribution mechanism.

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
