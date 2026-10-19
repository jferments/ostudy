# ADR 0058: Gemini through generateContent at the provider edge

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

[ADR 0057](0057-claude-messages-api-at-the-provider-edge.md) added Claude as a
dialect that lowers the shared chat request at the edge. Google's Gemini API
is another provider that is not OpenAI-compatible in the ways ostudy needs. Its
native API takes `contents` and `systemInstruction`, enforces JSON Schema output
with `responseJsonSchema`, controls thinking with a level or a token budget,
reports thinking tokens separately, and streams whole partial responses from a
sibling method, `streamGenerateContent?alt=sse`, with no terminator event.
Google's OpenAI compatibility layer hides those thinking and finish details.

## Decision

Add Gemini as `ProviderDialect::Gemini` on a third wire,
`ProviderWire::GeminiGenerateContent`.

- `providers/gemini.rs` lowers the shared chat body: system messages become
  `systemInstruction`, images become `inlineData`, the output cap and schema go
  in `generationConfig`, and the resolved effort becomes `thinkingLevel`
  (Gemini 3) or `thinkingBudget` (Gemini 2.5). Gemini 3 keeps its default
  temperature. One-shot responses are normalized to the chat-completion shape.
- The gateway sends `x-goog-api-key` as a sensitive header, streams from the
  sibling method without adding `stream` to the body, and decodes each chunk in
  the shared `SseChatAccumulator`. Thought parts count as reasoning, not
  content; usage counts thinking tokens as output and reports them as
  reasoning tokens.
- The fixed base URL is `https://generativelanguage.googleapis.com/v1beta/`.
  The models list keeps chat models that support `generateContent`; model
  details supply the context window.
- Document requests enforce the portable schema with `responseJsonSchema` and
  get one retry in JSON mode with the model's default thinking if the API
  refuses the request.

## Consequences

- Provider-neutral code still sees one completion shape.
- Gemini reports no cost; usage records tokens only.
- Gemini embeddings (`embedContent`) are not wired yet.
- Thinking support per family is a table kept in `providers/gemini.rs`;
  connection tests confirm it for the chosen model.
