#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

const oldSignature = "fn parse_flashcards(value: Value) -> Result<Vec<Value>, CommandError> {";
const newSignature = "fn parse_flashcards(value: Value, maximum_cards: usize) -> Result<Vec<Value>, CommandError> {";
if (!source.includes(newSignature)) {
  if (!source.includes(oldSignature)) throw new Error("Could not find parse_flashcards signature");
  source = source.replace(oldSignature, newSignature);
}

const oldLimit = `    if output.cards.len() > MAX_WINDOW_CARDS {
        return Err(output_error(format!(
            "The model returned more than {MAX_WINDOW_CARDS} cards for one semantic source window."
        )));
    }`;
const newLimit = `    let maximum_cards = maximum_cards.max(1).min(MAX_WINDOW_CARDS);
    if output.cards.len() > maximum_cards {
        return Err(output_error(format!(
            "The model returned more than {maximum_cards} cards for this semantic source window."
        )));
    }`;
if (!source.includes(newLimit)) {
  if (!source.includes(oldLimit)) throw new Error("Could not find parse_flashcards card limit");
  source = source.replace(oldLimit, newLimit);
}

if (!source.includes("dynamic_generation_card_limit_is_enforced")) {
  const marker = "    #[test]\n    fn schemas_cover_all_generation_forms()";
  const index = source.indexOf(marker);
  if (index < 0) throw new Error("Could not find document intelligence test insertion point");
  const test = `    #[test]
    fn dynamic_generation_card_limit_is_enforced() {
        let cards = (0..3)
            .map(|index| {
                json!({
                    "type": "basic",
                    "front": format!("Question {index}"),
                    "back": format!("Answer {index}"),
                    "tags": []
                })
            })
            .collect::<Vec<_>>();
        let error = parse_flashcards(json!({ "cards": cards }), 2).unwrap_err();
        assert_eq!(error.code, "ai_document_output_invalid");
        assert!(error.message.contains("more than 2 cards"));
    }

`;
  source = `${source.slice(0, index)}${test}${source.slice(index)}`;
}

if (source !== original) await writeFile(path, source, "utf8");
