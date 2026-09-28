---
name: generate-ostudy-flashcards
description: Generate a validated downloadable oStudy .ostudy.json flashcard deck from a topic, supplied text, or documents/files already available in the current ChatGPT conversation. Use when the user asks for oStudy flashcards, an oStudy deck, or an importable oStudy flashcard file.
---

# Generate oStudy flashcards

Create one importable `.ostudy.json` file. This skill is intentionally skills-only: do not look for, require, or call an MCP server, remote oStudy service, external flashcard service, or second model provider.

## Source selection

Use the material the user actually supplied:

- For a topic-only request, use the model's ordinary knowledge and follow any requested scope, level, and card count.
- For supplied text or attached/current-conversation documents, read those sources with the normal ChatGPT file/document capabilities and ground the deck in them. Do not send the source to another service.
- If the user gives both sources and supplemental instructions, use the sources as the factual grounding and the instructions to choose emphasis, difficulty, and format.
- Do not invent source citations or imply that an unattached source was reviewed.

For source-grounded decks, cover the important concepts and relationships across the source instead of overweighting easily extracted isolated facts. Preserve qualifications, units, exceptions, mechanisms, and clinically or technically meaningful distinctions.

## Choose the deck destination

Default to:

```json
{"mode":"create","name":"<useful human-readable deck name>","description":"<brief scope description>"}
```

Use `append` only when the user explicitly asks to append and gives the exact existing oStudy deck name. An append file cannot verify the user's local library; omit the description in append mode and state after generation that oStudy will require exactly one active deck with that name.

## Author cards

Always generate format version 2 so Basic, Cloze, and all text-native interactive question types are available. Read `references/authoring-guide.md` before authoring a substantial deck. Consult `references/ostudy-flashcards-v2.schema.json` when exact field names or bounds are needed, and use `references/all-card-types.ostudy.json` as shape examples.

Quality rules:

1. Test a meaningful retrievable idea, discrimination, relationship, mechanism, sequence, or classification.
2. Prefer conceptual coverage and useful application over trivia copied from isolated sentences.
3. Make the prompt sufficiently specific that the intended answer is unambiguous.
4. Keep answers concise enough for active recall while retaining the explanation needed to understand why.
5. Avoid near-duplicate cards unless the second card tests a genuinely different retrieval route.
6. Use reverse Basic cards only when both directions are independently useful and unambiguous.
7. Use Cloze when sentence context improves retrieval; do not turn every sentence into deletion trivia.
8. Use single-choice for one best answer with plausible distractors; use select-all only when the set itself is worth learning.
9. Use matching, grouping, and ordering only when the relationship or sequence is intrinsically study-worthy.
10. For interactive questions, include concise rationales/explanations when they improve learning. Do not make the correct answer obvious through wording, length, or category leakage.
11. Tags should be sparse, reusable topic labels, without a leading `#`.

For large requests, generate in manageable internal batches but assemble one final JSON document. Never bypass oStudy's 10,000-source-card or 16 MiB import limits.

## Build and validate the artifact

Create a draft JSON object matching the v2 contract, then run the bundled deterministic writer. Resolve this skill's directory from the path supplied when the skill is loaded; do not assume the current working directory.

Use:

```bash
python3 <skill-directory>/scripts/write_ostudy.py \
  --input <draft-json-path> \
  --output <deck-name>.ostudy.json
```

The writer performs strict shape, bounds, references, card-type, Cloze, tag, and conservative restricted-Markdown checks; normalizes safe whitespace; serializes UTF-8 JSON; re-opens and validates the written artifact; and refuses to leave a partial output on failure.

If validation fails:

1. read the reported JSON path and reason;
2. correct the draft rather than weakening the validator;
3. run the writer again;
4. do not return an unvalidated replacement assembled by hand.

oStudy's Rust importer remains the final authority for its restricted Markdown and durable import semantics. The bundled writer is a fail-closed preflight, and repository conformance tests keep its examples aligned with the native importer.

## Return the result

Return the validated `.ostudy.json` as a downloadable file/attachment. In the accompanying response, briefly state:

- deck name;
- total source-card count;
- card-type mix;
- whether it is `create` or `append`;
- that the user can import it from **Flashcards → Decks → Import deck from file**.

Do not paste the complete JSON unless the user asks. Do not claim the deck was imported into oStudy; this plugin only creates the file.
