# Generation contract quality and cost acceptance

This procedure supports AC-06 in the [generation contract refactor](generation-contract-refactor.md).
It does not turn structural validation, mock request counts or passing unit tests
into evidence of factual quality, real-provider latency or billing reduction.

The existing `scripts/generation-acceptance.py` pairwise comparison and its
50-percent engineering targets remain available unchanged. This stricter suite
uses a distinct command and does not replace the existing CLI or rubric.

## Two separate boundaries

Native regression tests establish conservative admission, retained siblings,
revision-safe checkpoints, bounded recovery and content-free measurements.
The comparison script establishes that **submitted** measurements and human
reviews meet explicitly selected requirements. It does not run a model, verify
that a binary actually corresponds to its declared commit, prove entailment,
certify a reviewer, or authorize a merge.

`scripts/generation-contract-acceptance.py` uses only the Python standard library. It reads
an evidence manifest and hash-bound files; it does not open `library.sqlite3`,
change app data, write source, generate artifacts or contact a provider.

## Prepare comparable evidence

Freeze two distinct full source SHAs. Build them with the same maintained
wrappers and isolated, compatible test libraries. Do not downgrade a schema-30
library for the baseline. Preserve the original user's library and source files.

Use the same original material, requested artifacts, source ranges, hardware,
provider/model selection, reasoning policy and runtime settings in each pair.
Put these settings, including collection membership/ranges and relevant device
information, into one configuration snapshot shared by the two runs. Changed
implementation-owned prompts or planning are the change under test, not an excuse
to change the selected provider or discard learning objectives.

Record fresh generation and checkpoint-resume comparisons separately using the
case `regime` and matching run `generationRegime` (`fresh` or `resume`). Include
both regimes in the suite. Resume pairs must start from equivalent retained
checkpoints; record their provenance and which work already exists in the shared
configuration snapshot. Do not compare a fresh baseline to a resumed candidate.
Provider prompt-cache state is a separate `cacheState` (`cold` or `warm`); both
runs in a pair must declare the same cache state. Keep the ordered original source
files, configuration snapshot, usage exports and published artifact exports under
one evidence folder.
Evidence references are relative to that folder; escaping paths and symlinks
outside it are refused. Ordinary source files are streamed for hashing, not
included in the resulting report. JSON evidence is bounded to 32 MiB per file.

For the latest operation, export the existing automatic diagnostic report:

```bash
python3 scripts/generation-usage.py --json > usage.json
```

Verify the operation identity before attaching this export to a benchmark run.
The existing explicit-operation option remains appropriate for archived evidence;
the comparator will not substitute the latest operation for an archived run.
`--all` exports are not single-operation acceptance evidence.

Define learning-objective IDs before reviewing outputs. A source's revision hash
and configuration alone cannot prove that useful content was preserved. Human
review must check factual support, qualifications, useful coverage, readability,
answer-key correctness and actual artifact publication, and account for every
original objective. Review every graded component and process ordering against
the sources rather than treating a structurally valid answer key as correct.
A finished guide does not stand in for missing flashcards.

## Manifest contract

The manifest is a JSON object with these fields:

| Field | Meaning |
| --- | --- |
| `schemaVersion` | Integer `1`. |
| `baselineSha`, `candidateSha` | Distinct, lowercase, full 40-character Git commit SHAs. These are operator attestations, not automatic binary provenance. |
| `limits.maxInputOutputTokenRatio` | Explicit positive finite maximum candidate/baseline token ratio. No target is invented by the script. |
| `limits.maxRecoveryInputOutputTokenRatio` | Explicit positive finite maximum candidate/baseline recovery-token ratio. Recovery spend is measured independently so lower first-pass cost cannot hide more retry/repair work. |
| `limits.maxElapsedRatio` | Explicit positive finite maximum candidate/baseline operation-duration ratio. |
| `limits.maxFirstUsableRatio` | Explicit positive finite maximum candidate/baseline first-usable-artifact latency ratio. |
| `cases` | Between 1 and 128 paired cases; complete acceptance covers every required kind below. |

Required case kinds are `textbook`, `slides`, `table`, `numeric`, `cross_page`,
`assessment`, `image` and `collection`. Multiple runs of a kind may be separate
cases, but every pair has its own usage snapshots. A case contains:

- `id`: a unique opaque lowercase identifier, using letters, digits and underscores
  and starting with a letter; maximum 64 characters. Do not encode private source
  titles or patient information in IDs.
- `regime`: `fresh` or `resume`, separate from provider prompt-cache state.
- `kind`, `objectiveIds` and `artifactKinds`: one required kind, nonempty unique
  objective identifiers, and nonempty unique artifact-kind identifiers such as
  `cards` and `guide`.
- `sources`: ordered original-file references; `configuration`: one shared
  configuration-file reference. A file reference is `{"path": "relative-name",
  "sha256": "lowercase SHA-256 of the exact bytes"}`.
- `baseline` and `candidate`: run records described below.

Every run record supplies its matching `headSha`, `operationId`, `generationRegime`,
`cacheState` (`cold` or `warm`), and `evidenceKind` (`live_provider` or `synthetic`). Declaring
`live_provider` is an explicit operator attestation, not proof supplied by the
comparator. Tests that exercise this declaration are still synthetic tests.

`sourceSetSha256` is the SHA-256 of the canonical JSON array of ordered source
SHA-256 strings. `configurationSha256` is the configuration file's exact-byte
SHA-256. `usage` is a file reference to that run's generation-usage JSON export.
`requestAccountingComplete` must explicitly be `true`; recorded requests alone
cannot establish that every physical attempt was accounted for.
`recoveryAccountingComplete` must also explicitly be `true`. This attests that
the exported rows and versioned native lineage are sufficient to classify
recovery work for that run; missing recovery classification is unknown, not zero.

`artifacts` contains exactly one file reference per requested artifact kind, with
additional `kind` and `usableAt` fields. `usableAt` is the observed Unix timestamp
in milliseconds when that exact artifact became usable, within the operation's
start/finish interval. A reused artifact already available at operation start may
use that start timestamp, but its availability still has to be observed. Do not
substitute a provider response, completed window or later export time for usable
publication. Missing observations remain unknown; this is operator-attested
measurement, not automatic proof of publication. The first usable latency is the
minimum of these observed timestamps minus the operation start. `review` contains:

| Field | Requirement |
| --- | --- |
| `artifactSetSha256` | SHA-256 of the canonical JSON object mapping artifact kind to exact-byte artifact SHA-256. Changing an artifact requires a new bound review. |
| `reviewer` | A recorded nonempty reviewer identifier. It is not emitted into comparison output. |
| `criteria` | Each of `factualSupport`, `preservedQualifications`, `usefulCoverage`, `readability`, `answerKeyCorrectness`, `artifactPublication` is `pass`, `fail` or unobserved. All must be `pass` for acceptance. |
| `objectives` | A map using the case's original objective IDs. Every value must be `covered`; `not_covered` rejects the comparison and missing/unknown values leave it incomplete. |

Canonical identity JSON is produced by Python's
`json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
allow_nan=False).encode("utf-8")`. The script exposes `identity(value)` to reuse
this exact definition. File hashes are always of original bytes, not canonicalized
file contents. The output includes the canonical manifest identity so the limits,
case selection and review bindings used by a result remain identifiable.

A zero-request run additionally requires `zeroRequestReuseObserved: true` and an
actual positive local-reuse event for the same operation in its export. Neither
an empty request list nor a zero-row database query proves zero provider work.
Even confirmed reuse does not convert synthetic evidence into live acceptance.

## Compare and interpret

From the checkout containing the script, with the evidence manifest at the stated
relative location:

```bash
python3 scripts/generation-contract-acceptance.py acceptance/manifest.json
```

The script emits JSON and exits with `0` for accepted submitted evidence, `1` for
a measured/reviewed rejection, or `2` for incomplete or invalid evidence.
There is no automatic provider rerun, source publication, PR-state change or merge.

Acceptance requires every case to meet quality plus its explicitly selected
total-token, recovery-token and latency limits. An improvement in one document
cannot average away a regression in another, and cheaper first-pass work cannot
hide a recovery-cost regression. Synthetic-only evidence always remains incomplete for
live-provider acceptance, even when its arithmetic and review declarations pass.

The script recalculates token measurements from raw request rows, not supplied
aggregate totals. Missing input or output usage remains unknown; booleans,
negative/fractional counts and values outside SQLite's signed integer range are
not measurements. Duplicate request IDs, cross-operation rows and impossible
cache/reasoning subsets invalidate the comparison. Returned provider responses
are distinct from published learning artifacts.

Input plus output is counted once. Reasoning is already within output and cached
input is already within input. Cache-read coverage is reported separately and
unknown cache observations do not become zero hits. Token reduction is not a
measured monetary-billing reduction, especially across different cache states.

Recovery cost is recalculated from the raw physical-request rows rather than a
supplied summary. A request is recovery work when its attempt ordinal is greater
than one, or when versioned `nativeLineage.recovery` records an allowed native
recovery reason even if that physical request is attempt one under a child key.
The recognized native reasons are transient transport, provider compatibility,
contract repair and output compaction. Malformed lineage is invalid evidence.
Missing recovery-accounting attestation or missing input/output usage on a
classified recovery request leaves recovery cost incomparable rather than
silently treating it as zero. The output reports recovery request count,
provider-reported recovery input/output tokens and recovery share of total tokens.

Completion latency uses the operation's recorded start and finish, not the sum
of overlapping provider intervals and not a precomputed duration from the export. Failed,
interrupted or unfinished baseline/candidate operations cannot qualify a successful
artifact comparison. This remains a paired observation, not a statistically
established performance claim; repeat cases and report variation for stronger
conclusions.

## Regression boundary

The comparator's behavioral tests live in
`tests/scripts/test_generation_contract_acceptance.py` and are found by the existing
Static Python discovery. A focused invocation is:

```bash
python3 -m unittest discover -s tests/scripts -p 'test_generation_contract_acceptance.py'
```

They use fabricated files, usage records and review declarations, including cases
that deliberately declare real-run provenance to test the decision logic. They
are not real-provider acceptance results. Native tests, full Linux and Android
qualification, visual/manual review and actual benchmark runs remain separate.
