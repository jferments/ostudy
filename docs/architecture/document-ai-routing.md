# Document AI routing and concurrency

Document hierarchy and provider request boundaries are deliberately separate concerns in ostudy.

Semantic extraction preserves headings, page/source provenance, element types, and section relationships. Provider requests are then packed by bounded source-token and source-unit capacity rather than splitting every time a heading changes. A highly structured textbook must therefore not degenerate into one outline request per heading or page.

For large documents, outline and flashcard windows are independent units of provider work. Cloud-backed windows may execute with bounded concurrency while checkpoint/database writes remain serialized in source-window order. This preserves deterministic durable state while allowing network/inference latency to overlap. The current provider concurrency policy is conservative: local llama.cpp runs one request at a time, direct DeepSeek uses two, and OpenRouter may use four.

OpenRouter routing treats compatibility, privacy, and model precision as hard eligibility constraints. Document requests require parameter-compatible endpoints, deny provider data collection by default, honor the profile's optional ZDR requirement, and restrict eligible endpoint quantization to INT8, FP8, FP16, BF16, or FP32 execution. Among eligible endpoints, routing prefers low price while applying throughput and latency preferences; low-grade INT4, FP4, FP6, and unknown-precision endpoints are not acceptable fallbacks.

A document-scoped `session_id` keeps related OpenRouter requests on a sticky provider after the first successful request, improving cache locality while still permitting provider fallback when that route becomes unavailable.

Provider attempts are also bounded at the document-task layer. A provider that accepts a stream but fails to complete useful work cannot stall the entire document indefinitely. Output exhaustion (`finish_reason=length`) is classified as truncation, not malformed JSON: flashcard generation responds by retrying the affected source window with a smaller card/output budget rather than repeating the identical oversized request twice.

These rules apply to both initial generation and future document-aware tasks. New document features should reuse semantic chunks, capacity-bounded windows, deterministic native assembly, and the shared provider routing/concurrency policy rather than introducing feature-specific page loops or unbounded request fan-out.
