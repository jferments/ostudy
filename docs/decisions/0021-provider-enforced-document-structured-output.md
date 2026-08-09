# ADR 0021: Provider-enforced structured output for document intelligence

- **Status:** Accepted
- **Date:** 2026-08-09
- **Scope:** PR #38 document outline, flashcard generation, and curation

## Context

Real-provider acceptance with a large PDF showed that valid local extraction/rendering could still end in repeated `ai_document_output_invalid` failures. The previous transport guaranteed only JSON syntax on some provider paths and then relied on prompt instructions plus local validation for the exact object shape. That is insufficient for long multi-window jobs: one provider response can drift from the expected contract even when the prompt clearly describes it.

The document pipeline already owns versioned JSON Schemas and local Rust validation. Those schemas should constrain provider sampling/output whenever the selected provider exposes a compatible enforcement mechanism.

## Decision

### llama.cpp / Unsloth

llama-server document requests use schema-constrained JSON output. The primary OpenAI-compatible request supplies:

```json
{
  "response_format": {
    "type": "json_object",
    "schema": { "...": "ostudy task schema" }
  }
}
```

llama-server converts the supplied JSON Schema into its constrained sampling grammar. If an older compatible server rejects that request shape, ostudy retries once against the exact same endpoint/model using llama-server's top-level `json_schema` compatibility field. The retry remains schema/grammar constrained; there is no unconstrained JSON compatibility path and no provider fallback.

### DeepSeek

Ordinary DeepSeek JSON Output guarantees JSON syntax but not ostudy's complete schema. Document intelligence therefore uses DeepSeek's Beta strict Function Calling mode:

- `/beta/chat/completions`;
- one forced result function;
- `strict: true`;
- a provider-compatible JSON Schema in `function.parameters`;
- streamed function arguments collected by the native gateway.

DeepSeek strict mode supports a narrower JSON-Schema subset than ostudy's local contracts. Before transmission, the native adapter converts the internal task schema into that subset:

- `oneOf` becomes `anyOf`;
- string length and array item-count keywords unsupported by strict mode are omitted;
- constant discriminators are represented as single-value enums when needed;
- every object property is required;
- every object sets `additionalProperties: false`.

The omitted length/count guarantees are still enforced by ostudy after the provider returns its schema-constrained object.

### Local validation remains authoritative

Provider enforcement prevents structural drift; it does not replace native semantic validation. ostudy still validates:

- card text and tag bounds;
- MCQ option count/uniqueness and answer index;
- Cloze mode and exact blank substrings;
- outline page ranges and item bounds;
- curation indices;
- canonical generated-import Markdown/content/tag/duplicate/scheduling rules.

A structurally valid response can contain one semantically unusable card. Such a card is now dropped locally while the other valid cards from the same generation window continue. The task still fails when the response object itself cannot satisfy/parse the required contract or when no useful cards remain across the document.

## Consequences

- Long document jobs no longer depend on prose instructions for output shape.
- llama.cpp/Unsloth remains locally grammar constrained even on its compatibility retry.
- DeepSeek document tasks use the Beta strict-tool endpoint specifically because ordinary JSON mode does not enforce the full schema.
- The gateway must support streamed tool-call argument deltas in addition to ordinary streamed `delta.content`.
- Provider-specific schema lowering remains inside the native AI adapter; document/UI code stays provider neutral.
- Native validation remains necessary for constraints not expressible in the provider's supported schema subset.

## Validation

Automated tests cover llama schema request construction, DeepSeek strict-schema lowering, strict tool-argument parsing, streamed tool-call accumulation, per-card semantic filtering, and the existing canonical generated-import validation boundary. Real-provider acceptance must still exercise the representative large PDF with both DeepSeek and the configured llama.cpp/Unsloth server before merge.
