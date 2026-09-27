use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::usage::{merge_usage_snapshot, safe_usage_envelope};
use crate::ai::gateway::{
    contracts::AiGatewayError,
    errors::{classify_stream_event_error, error},
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
        generation_id: Option<String>,
        retry_after_seconds: Option<u64>,
    ) -> Self {
        let mut stream = Self::new(maximum_content_bytes, maximum_wire_bytes);
        stream.openrouter = openrouter;
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
