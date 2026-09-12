# ADR 0046: Keep semantic retrieval application-owned

Status: accepted for PR #94

## Context

The semantic tutor needs local nearest-neighbour retrieval over embeddings returned by a user-selected remote provider. The retrieval layer must also enforce oStudy-specific rules that are more important than the vector math itself: material scope, exact source revision, deletion/invalidation, embedding-space identity, explicit preparation consent, conversation-retrieval permission, bounded memory/disk use, cancellation, and rebuildable-cache semantics.

The implementation in PR #94 stores vectors in a separate device-local SQLite cache. Native code first constructs the exact eligible entry set from authoritative library state, including source revisions and privacy controls. The vector index then ranks only that bounded set with exact cosine distance and keeps only the top-k results in memory. Results are revision-checked again before they become Tutor context. Embeddings are semantic provider outputs; no keyword match is required.

Two alternatives were considered while finishing the Tutor implementation: adopting the broader Rig framework (including rig-sqlite), or replacing the vector-index implementation with sqlite-vec.

## Decision

Keep the application-owned `VectorIndex` boundary and the current bounded exact cosine implementation for this feature. Do not add Rig, rig-sqlite, or sqlite-vec to PR #94.

This is an intentional implementation choice, not a claim that the current backend is intrinsically faster than sqlite-vec. The index remains rebuildable derived data and stays isolated behind the knowledge retrieval interface so a future backend can replace it without changing Tutor, provider, consent, source-authority, or conversation code.

## Why not Rig in this change

Rig provides useful generic RAG, agent-loop, tool-calling, and vector-store abstractions. oStudy's current Tutor deliberately is not an autonomous agent loop: it performs user-requested explanation, hint, quiz, and answer-evaluation operations through purpose-specific native services.

Introducing Rig here would duplicate or wrap application-owned behavior that already has stricter contracts:

- AI provider profiles and secrets remain native oStudy concepts;
- provider transmission requires explicit material selection/consent;
- task admission, cancellation, idempotent operation IDs, and restart recovery are native;
- sources have revisioned identities and provenance rules independent of embeddings;
- conversation retrieval has separate user permission and memory-cutoff semantics;
- quiz criteria are fixed privately before answer evaluation;
- model output never receives general SQL/file/tool authority.

Using an agent framework only for its vector-store facade would add an additional abstraction layer without removing those responsibilities. If oStudy later introduces genuinely tool-driven agent workflows, Rig can be evaluated at that boundary without making the semantic content model depend on it.

## Why not sqlite-vec in this change

sqlite-vec is a reasonable future backend and does not imply a bundled embedding model. Its scalar `vec_distance_cosine()` can move exact distance computation into SQLite, but an `ORDER BY vec_distance_cosine(...)` query is still a brute-force exact scan. Merely replacing the Rust loop with the scalar function therefore does not establish better asymptotic behavior or a meaningful mobile performance win.

The current index also has requirements that must survive any replacement:

- multiple independently versioned embedding spaces and dimensions;
- native scope/privacy/revision filtering before ranking;
- bounded eligible-set size and top-k memory use;
- cancellation during long searches;
- durable source-eviction replay across all spaces;
- a 512 MiB cache safety limit;
- no vector data in the webview or authoritative library backup.

A `vec0` design could eventually be useful, but it should be selected from measurements rather than introduced at the end of this feature without a representative library benchmark. It would also add a C/native dependency that must be qualified across Linux, Android, macOS, Windows, and iOS.

## Consequences

The current implementation is exact semantic retrieval, not keyword search and not approximate nearest-neighbour search. For an eligible set of `n` vectors with `d` dimensions, distance work is approximately `O(n*d)`. Scope filtering normally keeps `n` below the whole-library size; a defensive cap rejects excessively broad searches. Only one indexed vector plus the bounded top-k heap is hydrated at a time.

No performance claim is made beyond those explicit bounds. The implementation should be profiled on representative mobile and desktop libraries. If retrieval becomes a material latency or energy bottleneck, compare the existing index with sqlite-vec (including `vec0` where appropriate) using identical scope/revision/privacy semantics before changing the backend.

A future replacement must preserve the `VectorIndex`-level behavioral contract rather than leaking sqlite-vec, Rig, or another library's types into Tutor, document processing, provider profiles, or the shared knowledge representation.
