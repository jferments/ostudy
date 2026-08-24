use serde_json::Value;

use super::usage::safe_usage_envelope;
use crate::ai::gateway::{
    contracts::AiGatewayError,
    errors::{classify_stream_event_error, error},
    metadata::{safe_json_string, MAX_SAFE_PROVIDER_METADATA_CHARS},
};

pub(super) struct SseChatAccumulator {
    pending: Vec<u8>,
    pub(super) content: String,
    pub(super) tool_name: Option<String>,
    pub(super) tool_arguments: String,
    pub(super) finish_reason: Option<String>,
    pub(super) usage: Option<Value>,
    pub(super) reasoning_bytes: usize,
    pub(super) comment_events: u64,
    wire_bytes: usize,
    maximum_content_bytes: usize,
    maximum_wire_bytes: usize,
    pub(super) done: bool,
    openrouter: bool,
    pub(super) generation_id: Option<String>,
    retry_after_seconds: Option<u64>,
    pub(super) upstream_provider: Option<String>,
    pub(super) model: Option<String>,
}

impl SseChatAccumulator {
    pub(super) fn new(maximum_content_bytes: usize, maximum_wire_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            content: String::new(),
            tool_name: None,
            tool_arguments: String::new(),
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

    pub(super) fn for_request(
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

    pub(super) fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), AiGatewayError> {
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

    pub(super) fn finish(&mut self) -> Result<(), AiGatewayError> {
        if !self.pending.is_empty() && !self.done {
            let line = std::mem::take(&mut self.pending);
            self.process_line(&line)?;
        }
        Ok(())
    }

    pub(super) fn model_progress_bytes(&self) -> usize {
        self.content
            .len()
            .saturating_add(self.tool_arguments.len())
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
            self.usage = Some(usage);
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
            self.append_compat_content(content)?;
        } else if let Some(content) = choice
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .or_else(|| {
                choice
                    .and_then(|choice| choice.get("text"))
                    .and_then(Value::as_str)
            })
            .or_else(|| value.get("content").and_then(Value::as_str))
        {
            self.append_compat_content(content)?;
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
        Ok(())
    }

    fn append_compat_content(&mut self, content: &str) -> Result<(), AiGatewayError> {
        if content.is_empty() {
            return Ok(());
        }
        if !self.content.is_empty() && content.starts_with(&self.content) {
            return Ok(());
        }
        if self.content.len().saturating_add(content.len()) > self.maximum_content_bytes {
            return Err(error(
                "ai_response_too_large",
                "The AI provider returned more generated content than ostudy accepts.",
                false,
                None,
            ));
        }
        self.content.push_str(content);
        Ok(())
    }
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
