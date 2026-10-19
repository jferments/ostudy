# ADR 0057: Claude through the Messages API at the provider edge

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

[ADR 0047](0047-openai-compatible-provider-dialects.md) made every implemented
provider one OpenAI-compatible protocol family and allowed another family only
for a provider that is genuinely not compatible. Claude is such a provider.
Anthropic's native API is the Messages API: `x-api-key` and a pinned
`anthropic-version` header instead of a bearer token, a separate `system` field,
typed content blocks, `output_config.format` for JSON-schema output, and a
stream of typed `message_*` and `content_block_*` events. Anthropic's OpenAI
compatibility endpoint does not enforce JSON schemas, so it cannot carry
ostudy's document contracts.

Everything above the wire is already provider-neutral: document intelligence,
the Tutor, admission, usage accounting, recovery and the profile and probe
services all read one chat-completion shape.

## Decision

Add Claude as a fourth dialect, `ProviderDialect::Claude`, that lowers the
shared request at the edge and normalizes the response back.

- `providers/claude.rs` turns the shared chat body into a Messages request:
  system messages become one system block marked for prompt caching, images
  become base64 image blocks, sampling parameters are dropped (current models
  reject them), and the resolved reasoning effort becomes `thinking` and
  `output_config.effort`. Non-streaming probe responses are normalized to the
  chat-completion shape.
- The gateway gains `ProviderWire` (`OpenAiCompatible` or `AnthropicMessages`),
  chosen by the adapter rather than inferred from the URL. It selects the
  authentication headers, leaves `stream_options` out of Claude requests, and
  decodes Claude's events in the same `SseChatAccumulator`, so streaming,
  cancellation, inactivity limits, retry and usage accounting stay in one
  place. Claude's `input_tokens` excludes cache reads and writes; the
  normalized prompt count adds them back and reports them as cached and
  cache-write tokens.
- Claude has a fixed official base URL, `https://api.anthropic.com/v1/`.
- Model discovery reads the Models API capability tree. Models without
  structured outputs are hidden, as OpenRouter's are. A model that rejects
  `thinking: {type: "disabled"}` is recorded as reasoning mandatorily, so "no
  reasoning" resolves to its lowest effort instead of a request it would
  refuse. Model details supply image input and the context window.
- Document schemas are lowered with the shared portable schema. A schema inside
  Claude's documented subset (no references, scalar enums, at most 16 union
  parameters) is enforced with `output_config.format`, with one retry without
  it if the API still refuses the schema. Otherwise the schema stays in the
  instructions only. Native admission is authoritative either way.
- Claude joins OpenRouter as a provider whose catalog declares reasoning, so
  task reasoning resolves against the model and the completion budget gets
  headroom for hidden thinking.

## Consequences

- Provider-neutral code still sees one completion shape; no task service
  branches on Claude.
- Claude reports no cost, so usage records tokens only, as DeepSeek does.
- Claude has no embeddings API; embeddings assignments stay with OpenRouter and
  llama.cpp.
- Fixture tests cover lowering, streaming, usage, models, probes and schema
  limits. They do not prove behavior against the live API.
