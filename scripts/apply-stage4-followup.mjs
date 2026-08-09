#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patchFile(relativePath, transform) {
  const url = new URL(`../${relativePath}`, import.meta.url);
  const path = fileURLToPath(url);
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function replaceRequired(source, label, search, replacement) {
  if (!source.includes(search)) {
    throw new Error(`Could not find ${label}`);
  }
  return source.replace(search, replacement);
}

await patchFile("src-tauri/src/ai/document_intelligence.rs", (initial) => {
  if (initial.includes("const FLASHCARD_PROMPT_VERSION: i64 = 4;")) return initial;
  let source = initial;
  source = replaceRequired(
    source,
    "flashcard prompt version",
    "const FLASHCARD_PROMPT_VERSION: i64 = 3;",
    "const FLASHCARD_PROMPT_VERSION: i64 = 4;",
  );
  source = replaceRequired(
    source,
    "flashcard schema version",
    "const FLASHCARD_SCHEMA_VERSION: i64 = 3;",
    "const FLASHCARD_SCHEMA_VERSION: i64 = 4;",
  );
  source = replaceRequired(
    source,
    "basic reverse field",
    `        #[serde(default, rename = "reverseEnabled")]
        reverse_enabled: bool,
`,
    "",
  );
  source = replaceRequired(
    source,
    "basic reverse destructuring",
    `            reverse_enabled,
`,
    "",
  );
  source = replaceRequired(
    source,
    "basic reverse normalization",
    `                "reverseEnabled": reverse_enabled,
`,
    `                "reverseEnabled": false,
`,
  );
  source = replaceRequired(
    source,
    "definition normalization",
    `                "front": format!("What is {}?", term.trim()),
                "back": definition,
                "reverseEnabled": false,
`,
    `                "front": term,
                "back": definition,
                "reverseEnabled": true,
`,
  );
  source = replaceRequired(
    source,
    "basic schema reverse field",
    `            "reverseEnabled": { "type": "boolean" },
`,
    "",
  );
  source = replaceRequired(
    source,
    "direction prompt anchor",
    "Each card should test one coherent learning objective and be self-contained without the PDF. Use a meaningful variety when supported: basic conceptual questions, definition pairs, four-option multiple choice with one best answer and explanation, critical-thinking/application/scenario questions, and cloze fill-in-the-blank. ",
    "Each card should test one coherent learning objective and be self-contained without the PDF. Card direction is strict: use the `definition` type only for a straightforward term/definition pair that remains meaningful in both directions. Ordinary basic question/answer cards, multiple-choice cards, and critical-thinking/application cards are one-way question-to-answer cards and are never reversible. Never put an explanatory answer or descriptive passage on the question side with its question on the answer side. Use a meaningful variety when supported: basic conceptual questions, definition pairs, four-option multiple choice with one best answer and explanation, critical-thinking/application/scenario questions, and cloze fill-in-the-blank. ",
  );
  source = source.replace(`                "reasoning_effort": "none",\n`, "");
  source = replaceRequired(
    source,
    "llama compatibility response",
    `    let response = runtime
        .gateway
        .post_json(
            completion_endpoint(&task.profile)?,
            bearer,
            &body,
            task.profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await
        .map_err(gateway_to_command)?;
`,
    `    let first_response = runtime
        .gateway
        .post_json(
            completion_endpoint(&task.profile)?,
            bearer,
            &body,
            task.profile.request_timeout_ms,
            MAX_COMPLETION_BYTES,
        )
        .await;
    let response = match first_response {
        Ok(response) => Ok(response),
        Err(first_error)
            if task.profile.provider_kind == ProviderKind::LlamaCpp
                && first_error.safe.code == "ai_http_bad_request" =>
        {
            // Older llama.cpp / llama-server builds can support OpenAI-style
            // streaming and JSON output while rejecting newer schema/templating
            // request extensions. Stay on the exact configured provider/model,
            // fall back once to plain JSON-object output, and retain strict local
            // deserialization/normalization as the authority.
            let mut compatibility_body = body.clone();
            if let Some(object) = compatibility_body.as_object_mut() {
                object.remove("chat_template_kwargs");
                object.insert(
                    "response_format".to_string(),
                    json!({ "type": "json_object" }),
                );
            }
            let schema_text =
                serde_json::to_string(request.schema).map_err(CommandError::serialization)?;
            if let Some(system_content) = compatibility_body.pointer_mut("/messages/0/content") {
                let original = system_content.as_str().unwrap_or_default().to_string();
                *system_content = Value::String(format!(
                    "{original}\\n\\nCompatibility mode: return exactly one JSON object matching this schema. Do not include Markdown fences or commentary. Schema:\\n{schema_text}"
                ));
            }
            runtime
                .gateway
                .post_json(
                    completion_endpoint(&task.profile)?,
                    bearer,
                    &compatibility_body,
                    task.profile.request_timeout_ms,
                    MAX_COMPLETION_BYTES,
                )
                .await
        }
        Err(error) => Err(error),
    }
    .map_err(gateway_to_command)?;
`,
  );
  source = replaceRequired(
    source,
    "prompt version test",
    `        assert!(prompt.contains("Returning zero cards is preferable"));
        assert!(prompt.contains("schema version: 3"));
`,
    `        assert!(prompt.contains("Returning zero cards is preferable"));
        assert!(prompt.contains("never reversible"));
        assert!(prompt.contains("question side"));
        assert!(prompt.contains("schema version: 4"));
`,
  );
  source = replaceRequired(
    source,
    "schema direction test anchor",
    `        assert!(flashcards.contains("cloze"));
        assert!(outline.contains("learningObjectives"));
`,
    `        assert!(flashcards.contains("cloze"));
        assert!(!flashcards.contains("reverseEnabled"));
        assert!(outline.contains("learningObjectives"));
`,
  );
  source = replaceRequired(
    source,
    "cloze test anchor",
    `    #[test]
    fn cloze_blanks_are_constructed_by_rust() {
`,
    `    #[test]
    fn only_definition_cards_normalize_as_reversible() {
        let basic = normalize_generated_card(
            0,
            GeneratedStudyCard::Basic {
                front: "What causes hypoxic pulmonary vasoconstriction?".to_string(),
                back: "Low alveolar oxygen triggers local pulmonary arteriolar constriction.".to_string(),
                tags: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(basic["reverseEnabled"], false);
        assert!(basic["front"].as_str().unwrap().starts_with("What causes"));

        let definition = normalize_generated_card(
            1,
            GeneratedStudyCard::Definition {
                term: "Shunt".to_string(),
                definition: "Perfusion without ventilation.".to_string(),
                tags: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(definition["front"], "Shunt");
        assert_eq!(definition["back"], "Perfusion without ventilation.");
        assert_eq!(definition["reverseEnabled"], true);
    }

    #[test]
    fn cloze_blanks_are_constructed_by_rust() {
`,
  );
  return source;
});

await patchFile("src/screens/PracticeScreen.tsx", (initial) => {
  if (initial.includes("registerPracticeExitHandler")) return initial;
  let source = initial;
  source = replaceRequired(
    source,
    "practice exit import",
    `import { PracticeController } from "../controllers/practiceController";
`,
    `import { PracticeController } from "../controllers/practiceController";
import { registerPracticeExitHandler } from "../controllers/practiceExitGuard";
`,
  );
  source = replaceRequired(
    source,
    "practice exit registration anchor",
    `  const confirm = useConfirm();
  const experience = useExperience();

  useEffect(() => () => controller.invalidate(), [controller]);
`,
    `  const confirm = useConfirm();
  const experience = useExperience();

  useEffect(() => {
    if (!session || completed) return registerPracticeExitHandler(null);
    const activeSessionId = session.id;
    return registerPracticeExitHandler(async () => {
      if (pendingReviewRef.current) {
        setErrorMessage(
          "Finish or safely retry the pending rating before leaving this practice session.",
        );
        return false;
      }
      if (endingRef.current) return false;
      endingRef.current = true;
      setIsEnding(true);
      setErrorMessage(null);
      try {
        await controller.complete(activeSessionId);
        return true;
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
        return false;
      } finally {
        endingRef.current = false;
        setIsEnding(false);
      }
    });
  }, [completed, controller, session]);

  useEffect(() => () => controller.invalidate(), [controller]);
`,
  );
  return source;
});

await patchFile("src-tauri/src/document_more.rs", (initial) => {
  let source = initial;
  source = source.replace(`use std::{collections::HashSet, fs};\n`, `use std::fs;\n`);
  source = source.replace(
    `                        pages_json.len(),\n                        rusqlite::types::Type::Text,`,
    `                        3,\n                        rusqlite::types::Type::Text,`,
  );
  source = source.replace(
    `                        types_json.len(),\n                        rusqlite::types::Type::Text,`,
    `                        4,\n                        rusqlite::types::Type::Text,`,
  );
  source = source.replace(
    `            [..] if ids.is_empty() => Err(CommandError::new(`,
    `            [] => Err(CommandError::new(`,
  );
  source = source.replace(
    `        assert_eq!(selected.iter().collect::<HashSet<_>>().len(), 6);`,
    `        assert_eq!(selected.iter().collect::<std::collections::HashSet<_>>().len(), 6);`,
  );
  return source;
});
