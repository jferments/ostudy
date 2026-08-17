# ADR 0037: First-class OpenRouter provider adapter

## Decision

OpenRouter is a fixed-endpoint cloud provider at `https://openrouter.ai/api/v1/`, authenticated with the existing OS-backed bearer-secret boundary. ostudy uses the account-filtered `/models/user` catalog with a bounded public-catalog fallback, and `/chat/completions` for probes and document generation.

Structured document requests use strict JSON Schema plus `provider.require_parameters=true`. The default routing policy sets `provider.data_collection=deny` and allows compatible endpoint fallback. ostudy does not request OpenRouter plugins/context compression and never enables the provider's upstream-body debug echo. Streaming top-level errors abort the window and discard partial output.

The HTTP gateway remains responsible for transport, redirects, bounds, cancellation, and safe errors. Provider adapters own endpoint paths, request bodies, model parsing, and provider-specific error normalization.
