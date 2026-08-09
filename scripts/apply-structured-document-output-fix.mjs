#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patchFile(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function replaceRequired(source, label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

await patchFile("src-tauri/src/ai/gateway.rs", (initial) => {
  if (initial.includes("tool_arguments: String")) return initial;
  let source = initial;
  source = replaceRequired(
    source,
    "SSE accumulator fields",
    `struct SseChatAccumulator {
    pending: Vec<u8>,
    content: String,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    done: bool,
}`,
    `struct SseChatAccumulator {
    pending: Vec<u8>,
    content: String,
    tool_name: Option<String>,
    tool_arguments: String,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    done: bool,
}`,
  );
  source = replaceRequired(
    source,
    "SSE accumulator initialization",
    `            pending: Vec::new(),
            content: String::new(),
            wire_bytes: 0,`,
    `            pending: Vec::new(),
            content: String::new(),
            tool_name: None,
            tool_arguments: String::new(),
            wire_bytes: 0,`,
  );
  source = replaceRequired(
    source,
    "stream empty response check",
    `        if stream.content.trim().is_empty() {
            return Err(error(
                "ai_empty_response",
                "The model returned no final document-task content.",
                true,
                None,
            ));
        }
        Ok(json!({
            "choices": [{
                "message": { "content": stream.content }
            }]
        }))`,
    `        if stream.content.trim().is_empty() && stream.tool_arguments.trim().is_empty() {
            return Err(error(
                "ai_empty_response",
                "The model returned no final document-task content.",
                true,
                None,
            ));
        }
        let message = if stream.tool_arguments.trim().is_empty() {
            json!({ "content": stream.content })
        } else {
            json!({
                "content": if stream.content.trim().is_empty() { Value::Null } else { Value::String(stream.content) },
                "tool_calls": [{
                    "type": "function",
                    "function": {
                        "name": stream.tool_name.unwrap_or_default(),
                        "arguments": stream.tool_arguments
                    }
                }]
            })
        };
        Ok(json!({ "choices": [{ "message": message }] }))`,
  );
  source = replaceRequired(
    source,
    "SSE content accumulation",
    `        if let Some(content) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {
                return Err(error(
                    "ai_response_too_large",
                    "The AI provider returned more generated content than ostudy accepts.",
                    false,
                    None,
                ));
            }
            self.content.push_str(content);
        }
        Ok(())`,
    `        let delta = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"));
        if let Some(content) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {
                return Err(error(
                    "ai_response_too_large",
                    "The AI provider returned more generated content than ostudy accepts.",
                    false,
                    None,
                ));
            }
            self.content.push_str(content);
        }
        if let Some(tool_call) = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array)
            .and_then(|calls| calls.first())
        {
            if let Some(name) = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
            {
                match self.tool_name.as_deref() {
                    Some(existing) if existing != name => {
                        return Err(error(
                            "ai_provider_protocol_error",
                            "The AI provider changed structured-output tool names mid-stream.",
                            false,
                            None,
                        ));
                    }
                    None => self.tool_name = Some(name.to_string()),
                    _ => {}
                }
            }
            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(Value::as_str)
            {
                if self
                    .content
                    .len()
                    .saturating_add(self.tool_arguments.len())
                    .saturating_add(arguments.len())
                    > self.maximum_content_bytes
                {
                    return Err(error(
                        "ai_response_too_large",
                        "The AI provider returned more structured output than ostudy accepts.",
                        false,
                        None,
                    ));
                }
                self.tool_arguments.push_str(arguments);
            }
        }
        Ok(())`,
  );
  source = replaceRequired(
    source,
    "gateway SSE test anchor",
    `    #[test]
    fn sse_accumulator_bounds_generated_content() {`,
    `    #[test]
    fn sse_accumulator_collects_streamed_tool_arguments() {
        let mut stream = SseChatAccumulator::new(2048, 8192);
        stream
            .push_bytes(
                br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"submit_ostudy_document_result","arguments":"{\\"cards\\":["}}]}}]}\n"#,
            )
            .unwrap();
        stream
            .push_bytes(
                br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"]}"}}]}}]}\n\ndata: [DONE]\n\n"#,
            )
            .unwrap();
        stream.finish().unwrap();
        assert_eq!(
            stream.tool_name.as_deref(),
            Some("submit_ostudy_document_result")
        );
        assert_eq!(stream.tool_arguments, "{\\\"cards\\\":[]}");
        assert!(stream.done);
    }

    #[test]
    fn sse_accumulator_bounds_generated_content() {`,
  );
  return source;
});

await patchFile("src-tauri/src/ai/document_intelligence.rs", (initial) => {
  if (initial.includes("DEEPSEEK_STRICT_TOOL_NAME")) return initial;
  let source = initial;
  source = replaceRequired(
    source,
    "completion byte constant",
    `const MAX_COMPLETION_BYTES: usize = 2 * 1024 * 1024;`,
    `const MAX_COMPLETION_BYTES: usize = 2 * 1024 * 1024;
const DEEPSEEK_STRICT_TOOL_NAME: &str = "submit_ostudy_document_result";`,
  );

  const requestStart = source.indexOf("async fn request_json(");
  const requestEnd = source.indexOf("\nfn parse_outline", requestStart);
  if (requestStart < 0 || requestEnd < 0) throw new Error("Could not locate request_json block");
  const replacement = `async fn request_json(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    request: JsonTaskRequest<'_>,
) -> Result<Value, CommandError> {
    let credential = runtime.secrets.resolve(
        &task.profile.id,
        task.profile.auth_mode,
        task.credential_storage,
    )?;
    let bearer = credential
        .as_ref()
        .map(|secret| secret.as_utf8())
        .transpose()?;

    let (url, body) = match task.profile.provider_kind {
        ProviderKind::LlamaCpp => {
            let mut parts = vec![json!({ "type": "text", "text": request.user_text })];
            for image in request.images.iter().take(MAX_INLINE_IMAGES) {
                parts.push(json!({
                    "type": "text",
                    "text": format!(
                        "Rendered PDF page {} follows. Use it only as source evidence for diagrams, tables, equations, and layout-dependent or image-only content.",
                        image.page_number
                    )
                }));
                parts.push(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!(
                            "data:image/png;base64,{}",
                            STANDARD.encode(&image.bytes)
                        )
                    }
                }));
            }
            (
                completion_endpoint(&task.profile)?,
                json!({
                    "model": task.profile.model_id,
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": parts }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "chat_template_kwargs": { "enable_thinking": false },
                    "response_format": {
                        "type": "json_object",
                        "schema": request.schema
                    }
                }),
            )
        }
        ProviderKind::Deepseek => {
            let strict_schema = deepseek_strict_schema(request.schema);
            (
                deepseek_strict_completion_endpoint(&task.profile)?,
                json!({
                    "model": task.profile.model_id,
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": request.user_text }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "thinking": { "type": "disabled" },
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": DEEPSEEK_STRICT_TOOL_NAME,
                            "description": "Submit the exact structured ostudy document-task result.",
                            "strict": true,
                            "parameters": strict_schema
                        }
                    }],
                    "tool_choice": {
                        "type": "function",
                        "function": { "name": DEEPSEEK_STRICT_TOOL_NAME }
                    }
                }),
            )
        }
        _ => {
            return Err(CommandError::new(
                "ai_provider_not_implemented",
                "The selected AI provider cannot analyze PDF documents in this ostudy build.",
            ));
        }
    };

    if serde_json::to_vec(&body)
        .map_err(CommandError::serialization)?
        .len()
        > MAX_REQUEST_JSON_BYTES
    {
        return Err(CommandError::new(
            "ai_request_too_large",
            "This semantic document window is too large for ostudy's bounded provider request.",
        ));
    }

    let first_response = runtime
        .gateway
        .post_json(
            url.clone(),
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
            // Keep structured generation enforced on compatibility retry. Older
            // llama-server builds vary in which schema request shape they accept,
            // but top-level json_schema is still compiled into a sampling grammar.
            let mut compatibility_body = body.clone();
            if let Some(object) = compatibility_body.as_object_mut() {
                object.remove("chat_template_kwargs");
                object.remove("response_format");
                object.insert("json_schema".to_string(), request.schema.clone());
            }
            runtime
                .gateway
                .post_json(
                    url,
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

    match task.profile.provider_kind {
        ProviderKind::Deepseek => parse_strict_tool_arguments(&response),
        _ => {
            let content = parse_completion_content(&response)?;
            serde_json::from_str(normalize_json_content(content)).map_err(|_| {
                CommandError::new(
                    "ai_document_output_invalid",
                    "The AI provider returned content that was not valid JSON for the document task.",
                )
            })
        }
    }
}

fn deepseek_strict_completion_endpoint(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    join_endpoint(&profile.base_url, "beta/chat/completions")
}

fn parse_strict_tool_arguments(value: &Value) -> Result<Value, CommandError> {
    let function = value
        .pointer("/choices/0/message/tool_calls/0/function")
        .ok_or_else(|| {
            output_error("DeepSeek did not return the required schema-constrained tool result.")
        })?;
    if function.get("name").and_then(Value::as_str) != Some(DEEPSEEK_STRICT_TOOL_NAME) {
        return Err(output_error(
            "DeepSeek returned an unexpected structured-output tool name.",
        ));
    }
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| output_error("DeepSeek returned empty structured-output arguments."))?;
    serde_json::from_str(arguments).map_err(|_| {
        output_error("DeepSeek returned tool arguments that were not valid JSON.")
    })
}

fn deepseek_strict_schema(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(deepseek_strict_schema).collect()),
        Value::Object(object) => {
            if let Some(constant) = object.get("const") {
                let value_type = match constant {
                    Value::String(_) => "string",
                    Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
                    Value::Number(_) => "number",
                    Value::Bool(_) => "boolean",
                    _ => "string",
                };
                return json!({
                    "type": value_type,
                    "enum": [constant.clone()]
                });
            }

            let mut normalized = serde_json::Map::new();
            for (key, child) in object {
                match key.as_str() {
                    "minLength" | "maxLength" | "minItems" | "maxItems" | "required" => {}
                    "oneOf" => {
                        normalized.insert("anyOf".to_string(), deepseek_strict_schema(child));
                    }
                    _ => {
                        normalized.insert(key.clone(), deepseek_strict_schema(child));
                    }
                }
            }

            if let Some(properties) = normalized.get("properties").and_then(Value::as_object) {
                let required = properties.keys().cloned().map(Value::String).collect::<Vec<_>>();
                normalized.insert("required".to_string(), Value::Array(required));
                normalized.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}
`;
  source = `${source.slice(0, requestStart)}${replacement}${source.slice(requestEnd)}`;

  source = replaceRequired(
    source,
    "flashcard whole-response collection",
    `    output
        .cards
        .into_iter()
        .enumerate()
        .map(|(index, card)| normalize_generated_card(index, card))
        .collect()`,
    `    let mut accepted = Vec::with_capacity(output.cards.len());
    let mut rejected = 0_usize;
    for (index, card) in output.cards.into_iter().enumerate() {
        match normalize_generated_card(index, card) {
            Ok(card) => accepted.push(card),
            Err(_) => rejected = rejected.saturating_add(1),
        }
    }
    if rejected > 0 {
        crate::logging::info(
            "document_flashcards_filtered",
            format!("accepted={} rejected={rejected}", accepted.len()),
        );
    }
    Ok(accepted)`,
  );

  source = replaceRequired(
    source,
    "DeepSeek completion endpoint",
    `        ProviderKind::Deepseek => join_endpoint(&profile.base_url, "chat/completions"),`,
    `        ProviderKind::Deepseek => deepseek_strict_completion_endpoint(profile),`,
  );

  source = replaceRequired(
    source,
    "schema tests anchor",
    `    #[test]
    fn schemas_cover_all_generation_forms() {`,
    `    #[test]
    fn deepseek_strict_schema_uses_supported_required_shapes() {
        let schema = deepseek_strict_schema(&flashcard_schema());
        let serialized = serde_json::to_string(&schema).unwrap();
        assert!(serialized.contains("\\\"anyOf\\\""));
        assert!(!serialized.contains("\\\"oneOf\\\""));
        assert!(!serialized.contains("maxLength"));
        assert!(!serialized.contains("maxItems"));
        let card_properties = schema
            .pointer("/properties/cards/items/anyOf/0/properties")
            .and_then(Value::as_object)
            .unwrap();
        let required = schema
            .pointer("/properties/cards/items/anyOf/0/required")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(required.len(), card_properties.len());
    }

    #[test]
    fn strict_tool_arguments_parse_without_prompt_schema_repair() {
        let value = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": {
                            "name": DEEPSEEK_STRICT_TOOL_NAME,
                            "arguments": "{\\\"cards\\\":[]}"
                        }
                    }]
                }
            }]
        });
        assert_eq!(parse_strict_tool_arguments(&value).unwrap(), json!({ "cards": [] }));
    }

    #[test]
    fn schemas_cover_all_generation_forms() {`,
  );

  return source;
});
