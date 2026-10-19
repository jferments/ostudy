use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::usage::{merge_usage_snapshot, safe_usage_envelope};
use crate::ai::gateway::{
    contracts::{AiGatewayError, ProviderWire},
    errors::{classify_anthropic_stream_error, classify_stream_event_error, error},
    metadata::{safe_json_string, MAX_SAFE_PROVIDER_METADATA_CHARS},
};

pub(crate) struct SseChatAccumulator {
    pending: Vec<u8>,
    pub(crate) content: String,
    tools: BTreeMap<u64, ToolCall>,
    pub(crate) finish_reason: Option<String>,
    pub(crate) usage: Option<Value>,
    pub(crate) reasoning_bytes: usize,
    pub(crate) comment_events: u64,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    pub(crate) done: bool,
    openrouter: bool,
    wire: ProviderWire,
    anthropic_usage: AnthropicUsage,
    pub(crate) generation_id: Option<String>,
    retry_after_seconds: Option<u64>,
    pub(crate) upstream_provider: Option<String>,
    pub(crate) model: Option<String>,
}

impl SseChatAccumulator {
    pub(crate) fn new(maximum_content_bytes: usize, maximum_wire_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            content: String::new(),
            tools: BTreeMap::new(),
            finish_reason: None,
            usage: None,
            reasoning_bytes: 0,
            comment_events: 0,
            wire_bytes: 0,
            maximum_content_bytes,
            maximum_wire_bytes,
            done: false,
            openrouter: false,
            wire: ProviderWire::OpenAiCompatible,
            anthropic_usage: AnthropicUsage::default(),
            generation_id: None,
            retry_after_seconds: None,
            upstream_provider: None,
            model: None,
        }
    }

    pub(crate) fn for_request(
        maximum_content_bytes: usize,
        maximum_wire_bytes: usize,
        openrouter: bool,
        wire: ProviderWire,
        generation_id: Option<String>,
        retry_after_seconds: Option<u64>,
    ) -> Self {
        let mut stream = Self::new(maximum_content_bytes, maximum_wire_bytes);
        stream.openrouter = openrouter;
        stream.wire = wire;
        stream.generation_id = generation_id;
        stream.retry_after_seconds = retry_after_seconds;
        stream
    }

    pub(crate) fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), AiGatewayError> {
        self.wire_bytes = self.wire_bytes.saturating_add(bytes.len());
        if self.wire_bytes > self.maximum_wire_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned a streaming response larger than ostudy accepts.",
                false,
                None,
            ));
        }
        self.pending.extend_from_slice(bytes);
        while let Some(position) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=position).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.process_line(&line)?;
            if self.done {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<(), AiGatewayError> {
        if !self.pending.is_empty() && !self.done {
            let line = std::mem::take(&mut self.pending);
            self.process_line(&line)?;
        }
        if self.tools.values().any(|tool| tool.name.is_empty()) {
            return Err(protocol_error(
                "A streamed structured tool result has no function name.",
            ));
        }
        Ok(())
    }

    pub(crate) fn tool_arguments_len(&self) -> usize {
        self.tools.values().map(|tool| tool.arguments.len()).sum()
    }

    pub(crate) fn tool_calls(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|(index, tool)| {
                json!({
                    "index":index, "id":tool.id, "type":"function",
                    "function":{"name":tool.name,"arguments":tool.arguments}
                })
            })
            .collect()
    }

    pub(crate) fn model_progress_bytes(&self) -> usize {
        self.content
            .len()
            .saturating_add(self.tool_arguments_len())
            .saturating_add(self.reasoning_bytes)
    }

    fn process_line(&mut self, line: &[u8]) -> Result<(), AiGatewayError> {
        if line.is_empty() {
            return Ok(());
        }
        if line.starts_with(b":") {
            self.comment_events = self.comment_events.saturating_add(1);
            return Ok(());
        }
        let Some(data) = line.strip_prefix(b"data:") else {
            return Ok(());
        };
        let data = trim_ascii(data);
        if data == b"[DONE]" {
            self.done = true;
            return Ok(());
        }
        if data.is_empty() {
            return Ok(());
        }
        let value: Value = serde_json::from_slice(data).map_err(|_| {
            error(
                "ai_provider_protocol_error",
                "The AI provider returned a malformed streaming event.",
                false,
                None,
            )
        })?;
        if self.wire == ProviderWire::AnthropicMessages {
            return self.process_anthropic_event(&value);
        }
        if let Some(usage) = safe_usage_envelope(&value) {
            merge_usage_snapshot(&mut self.usage, usage);
        }
        if self.openrouter {
            if self.upstream_provider.is_none() {
                self.upstream_provider =
                    safe_json_string(value.get("provider"), MAX_SAFE_PROVIDER_METADATA_CHARS);
            }
            if self.model.is_none() {
                self.model = safe_json_string(value.get("model"), MAX_SAFE_PROVIDER_METADATA_CHARS);
            }
        }
        if value.get("error").is_some_and(|value| !value.is_null()) {
            return Err(classify_stream_event_error(
                &value,
                self.openrouter,
                self.generation_id.as_deref(),
                self.retry_after_seconds,
            ));
        }
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .filter(|reason| !reason.is_empty())
        {
            self.finish_reason = Some(reason.to_string());
        }
        let delta = choice.and_then(|choice| choice.get("delta"));
        if let Some(reasoning) = delta
            .and_then(|delta| delta.get("reasoning_content"))
            .and_then(Value::as_str)
        {
            self.reasoning_bytes = self.reasoning_bytes.saturating_add(reasoning.len());
        }
        if let Some(content) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            self.append_content(content, false)?;
        } else if let Some(content) = choice
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
        {
            self.append_content(content, true)?;
        } else if let Some(content) = choice
            .and_then(|choice| choice.get("text"))
            .and_then(Value::as_str)
            .or_else(|| value.get("content").and_then(Value::as_str))
        {
            self.append_content(content, false)?;
        }
        if let Some(calls) = delta.and_then(|delta| delta.get("tool_calls")) {
            self.append_tools(calls, false)?;
        } else if let Some(calls) = choice
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("tool_calls"))
        {
            self.append_tools(calls, true)?;
        }
        Ok(())
    }

    /// Translate one Anthropic Messages event into the same accumulated
    /// state the OpenAI-compatible decoder builds, so everything after the
    /// gateway sees one completion shape.
    fn process_anthropic_event(&mut self, value: &Value) -> Result<(), AiGatewayError> {
        match value.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                let message = value.get("message");
                if self.model.is_none() {
                    self.model = safe_json_string(
                        message.and_then(|message| message.get("model")),
                        MAX_SAFE_PROVIDER_METADATA_CHARS,
                    );
                }
                if self.generation_id.is_none() {
                    self.generation_id = safe_json_string(
                        message.and_then(|message| message.get("id")),
                        MAX_SAFE_PROVIDER_METADATA_CHARS,
                    );
                }
                if let Some(usage) = message.and_then(|message| message.get("usage")) {
                    self.record_anthropic_usage(usage);
                }
            }
            Some("content_block_start") => {
                let index = anthropic_block_index(value)?;
                let block = value.get("content_block");
                match block
                    .and_then(|block| block.get("type"))
                    .and_then(Value::as_str)
                {
                    Some("text") => {
                        if let Some(text) = block
                            .and_then(|block| block.get("text"))
                            .and_then(Value::as_str)
                        {
                            self.append_content(text, false)?;
                        }
                    }
                    Some("tool_use") => {
                        let call = json!([{
                            "index": index,
                            "id": block.and_then(|block| block.get("id")),
                            "type": "function",
                            "function": {
                                "name": block.and_then(|block| block.get("name")),
                                "arguments": ""
                            }
                        }]);
                        self.append_tools(&call, false)?;
                    }
                    _ => {}
                }
            }
            Some("content_block_delta") => {
                let index = anthropic_block_index(value)?;
                let delta = value.get("delta");
                match delta
                    .and_then(|delta| delta.get("type"))
                    .and_then(Value::as_str)
                {
                    Some("text_delta") => {
                        if let Some(text) = delta
                            .and_then(|delta| delta.get("text"))
                            .and_then(Value::as_str)
                        {
                            self.append_content(text, false)?;
                        }
                    }
                    Some("thinking_delta") => {
                        let thinking = delta
                            .and_then(|delta| delta.get("thinking"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        self.reasoning_bytes = self.reasoning_bytes.saturating_add(thinking.len());
                    }
                    Some("input_json_delta") => {
                        let call = json!([{
                            "index": index,
                            "function": {
                                "arguments": delta.and_then(|delta| delta.get("partial_json"))
                            }
                        }]);
                        self.append_tools(&call, false)?;
                    }
                    // Thinking signatures and citations carry no output text.
                    _ => {}
                }
            }
            Some("message_delta") => {
                if let Some(reason) = value
                    .pointer("/delta/stop_reason")
                    .and_then(Value::as_str)
                    .filter(|reason| !reason.is_empty())
                {
                    self.finish_reason = Some(anthropic_finish_reason(reason).to_string());
                }
                if let Some(usage) = value.get("usage") {
                    self.record_anthropic_usage(usage);
                }
            }
            Some("message_stop") => self.done = true,
            Some("ping") => self.comment_events = self.comment_events.saturating_add(1),
            Some("error") => {
                return Err(classify_anthropic_stream_error(
                    value,
                    self.retry_after_seconds,
                ))
            }
            // content_block_stop and event types added later carry no state
            // ostudy needs.
            _ => {}
        }
        Ok(())
    }

    fn record_anthropic_usage(&mut self, usage: &Value) {
        self.anthropic_usage.merge(usage);
        if let Some(envelope) = self.anthropic_usage.envelope() {
            self.usage = Some(envelope);
        }
    }

    fn append_content(&mut self, content: &str, snapshot: bool) -> Result<(), AiGatewayError> {
        if content.is_empty() {
            return Ok(());
        }
        let added = if snapshot {
            content.strip_prefix(&self.content).ok_or_else(|| {
                protocol_error("The final message conflicts with its streamed text.")
            })?
        } else {
            content
        };
        if self
            .content
            .len()
            .saturating_add(self.tool_arguments_len())
            .saturating_add(added.len())
            > self.maximum_content_bytes
        {
            return Err(output_limit());
        }
        // Deltas are literal increments, even when identical to earlier text.
        // A full-message snapshot contributes ONLY its not-yet-received tail.
        let added = added.to_string();
        self.content.push_str(&added);
        Ok(())
    }

    fn append_tools(&mut self, value: &Value, snapshot: bool) -> Result<(), AiGatewayError> {
        if value.is_null() {
            return Ok(());
        }
        let calls = value
            .as_array()
            .filter(|calls| calls.len() <= 128)
            .ok_or_else(|| {
                protocol_error("The provider returned an invalid tool-call collection.")
            })?;
        for (position, call) in calls.iter().enumerate() {
            let index = match call.get("index") {
                Some(index) => index.as_u64(),
                None if snapshot => Some(position as u64),
                None if calls.len() == 1 && self.tools.keys().all(|index| *index == 0) => Some(0),
                None => None,
            }
            .filter(|index| *index < 128)
            .ok_or_else(|| {
                protocol_error("Streamed tool calls require a bounded unambiguous index.")
            })?;
            if call
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind != "function")
            {
                return Err(protocol_error(
                    "The provider returned an unsupported tool-call type.",
                ));
            }
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty());
            if id.is_some_and(|id| {
                id.len() > 256
                    || self.tools.iter().any(|(other_index, tool)| {
                        *other_index != index && tool.id.as_deref() == Some(id)
                    })
            }) {
                return Err(protocol_error(
                    "The provider returned conflicting tool-call identities.",
                ));
            }
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    protocol_error("The provider returned an invalid tool-call function.")
                })?;
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty());
            if name.is_some_and(|name| name.len() > 128) {
                return Err(protocol_error(
                    "The provider returned an oversized tool name.",
                ));
            }
            let arguments = match function.get("arguments") {
                None | Some(Value::Null) => "",
                Some(Value::String(text)) => text.as_str(),
                _ => {
                    return Err(protocol_error(
                        "Tool arguments must be streamed as strings.",
                    ))
                }
            };
            let total = self.content.len().saturating_add(self.tool_arguments_len());
            let tool = self.tools.entry(index).or_default();
            if let Some(id) = id {
                if tool.id.as_deref().is_some_and(|previous| previous != id) {
                    return Err(protocol_error(
                        "The provider changed a tool-call identity mid-stream.",
                    ));
                }
                tool.id = Some(id.to_string());
            }
            if let Some(name) = name {
                if !tool.name.is_empty() && tool.name != name {
                    return Err(protocol_error(
                        "The provider changed a tool name mid-stream.",
                    ));
                }
                tool.name = name.to_string();
            }
            let added = if snapshot {
                arguments.strip_prefix(&tool.arguments).ok_or_else(|| {
                    protocol_error("The final tool result conflicts with its streamed arguments.")
                })?
            } else {
                arguments
            };
            if total.saturating_add(added.len()) > self.maximum_content_bytes {
                return Err(output_limit());
            }
            let added = added.to_string();
            tool.arguments.push_str(&added);
        }
        Ok(())
    }
}

/// Anthropic reports cumulative counts; input_tokens excludes cache reads and
/// cache writes, which are billed separately.
#[derive(Default)]
struct AnthropicUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
}

impl AnthropicUsage {
    fn merge(&mut self, usage: &Value) {
        let read = |key: &str| usage.get(key).and_then(Value::as_u64);
        if let Some(value) = read("input_tokens") {
            self.input_tokens = Some(value);
        }
        if let Some(value) = read("output_tokens") {
            self.output_tokens = Some(value);
        }
        if let Some(value) = read("cache_read_input_tokens") {
            self.cache_read_input_tokens = Some(value);
        }
        if let Some(value) = read("cache_creation_input_tokens") {
            self.cache_creation_input_tokens = Some(value);
        }
    }

    fn envelope(&self) -> Option<Value> {
        anthropic_usage_envelope(
            self.input_tokens,
            self.output_tokens,
            self.cache_read_input_tokens,
            self.cache_creation_input_tokens,
        )
    }
}

/// Project Anthropic usage onto ostudy's normalized usage: the prompt is every
/// input token the request was billed for, with cache reads and writes
/// broken out.
pub(crate) fn anthropic_usage_envelope(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
) -> Option<Value> {
    if input_tokens.is_none() && output_tokens.is_none() {
        return None;
    }
    let prompt_tokens = input_tokens.map(|input| {
        input
            .saturating_add(cache_read_input_tokens.unwrap_or(0))
            .saturating_add(cache_creation_input_tokens.unwrap_or(0))
    });
    let total_tokens = prompt_tokens
        .zip(output_tokens)
        .map(|(prompt, output)| prompt.saturating_add(output));
    Some(json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": output_tokens,
        "total_tokens": total_tokens,
        "prompt_tokens_details": {
            "cached_tokens": cache_read_input_tokens,
            "cache_write_tokens": cache_creation_input_tokens,
        },
        "completion_tokens_details": {
            "reasoning_tokens": Value::Null,
        },
        "cost": Value::Null,
    }))
}

pub(crate) fn anthropic_finish_reason(stop_reason: &str) -> &'static str {
    match stop_reason {
        "end_turn" | "stop_sequence" => "stop",
        "max_tokens" | "model_context_window_exceeded" => "length",
        "refusal" => "content_filter",
        "tool_use" => "tool_calls",
        "pause_turn" => "pause_turn",
        _ => "unknown",
    }
}

fn anthropic_block_index(value: &Value) -> Result<u64, AiGatewayError> {
    value
        .get("index")
        .and_then(Value::as_u64)
        .filter(|index| *index < 128)
        .ok_or_else(|| protocol_error("A streamed content block has no bounded index."))
}

#[derive(Default)]
struct ToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
}

fn protocol_error(message: &str) -> AiGatewayError {
    error("ai_provider_protocol_error", message, false, None)
}

fn output_limit() -> AiGatewayError {
    error(
        "ai_response_too_large",
        "The provider returned more generated content than ostudy accepts.",
        false,
        None,
    )
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}
