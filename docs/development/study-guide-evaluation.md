# Study-guide evaluation

A repeatable way to measure study-guide quality, so a change to guide generation reports a before and after score instead of an impression. It has three parts:

- **`guide-eval`**, a native runner behind the `guide-eval` cargo feature. It creates an isolated library, configures one provider through the same profile, credential, probe and task-default services as Settings, stages and imports one source file through the document import path, and creates its guide through the reader's own generation path (`generate_document_study_guide_for_document`). It writes `outcome.json`: the published guide, the learning plan, the canonical source chunks with their locators, the probe result, timings, and the failing stage when a run fails. The library stays beside it for usage reports.
- **`scripts/guide-eval.py`**, standard-library Python. `generate` runs the native runner over a corpus, `score` computes deterministic metrics and the rubric score below, and `compare` puts two scored runs side by side.
- **A corpus and its results**, kept outside this repository. Real study material is often copyrighted or personal and must not be committed here.

The runner is never compiled into the application and registers no Tauri command.

## Running it

Paid provider calls need the maintainer's go and a cost estimate first; pilot on one document.

```bash
cd ~/code/ostudy
export OSTUDY_ROOT_DIR="$PWD"
cargo build --manifest-path src-tauri/Cargo.toml --features guide-eval --bin guide-eval

export OSTUDY_EVAL_API_KEY=...          # generating provider (not needed for llama.cpp)
python3 scripts/guide-eval.py generate --corpus /path/to/corpus.json \
  --out /path/to/runs/main-a5f4a16 --provider deepseek \
  --base-url https://api.deepseek.com --model deepseek-chat

export OSTUDY_JUDGE_API_KEY=...         # judge, any OpenAI-compatible endpoint
python3 scripts/guide-eval.py score --run /path/to/runs/main-a5f4a16 \
  --corpus /path/to/corpus.json --judge-model <judge model>

python3 scripts/guide-eval.py compare /path/to/runs/main-a5f4a16 /path/to/runs/my-change
```

`corpus.json` is a list of `{"id", "file", "kind", "title"}` entries with `file` relative to the corpus file. Each run directory holds `run.json` (commit, provider, model), one folder per document (`outcome.json`, `usage.json`, `guide.md` for reading, `judge/` prompts and verdicts), and `score.json` / `score.md`.

`--judge manual` writes every judge prompt to `judge/*.prompt.txt` and scores whatever `*.response.json` files exist, so a person or an assistant session can act as the judge. Saved verdicts are reused on a re-score; delete a `judge/` folder to judge again.

## Rubric (version 1)

The judge grades each guide section against the source chunks its locator names, and grades the whole guide against the whole source. Criteria are scored 1 to 5 with the anchors written into the judge prompts in `scripts/guide-eval.py`, then mapped to 0-100.

| Criterion | Weight | Judged on | What it asks |
| --- | --- | --- | --- |
| Accuracy | 30% | each section, weighted by its length | Are the statements true to the source? Wrong or misleading statements are listed as major. |
| Coverage | 25% | whole guide | Does the guide teach every core concept in the source? Missing concepts are listed. |
| Source support | 15% | each section | Can each substantive statement be traced to the source? Correct outside knowledge still counts against it. |
| Organization | 15% | whole guide | Learning order, section granularity, duplication, a framing overview. |
| Clarity | 15% | each section | Readable explanation of mechanisms and relationships rather than terse lists. |

The guide score is the weighted mean. A run that publishes no guide scores 0 for that document. Reported beside the score, never folded into it:

- **Major unsupported claims**, the count a student would be misled by. A change that raises the score but adds major errors is a regression.
- **Cost**: provider requests, tokens and reported cost from `scripts/generation-usage.py`, plus wall time.
- **Deterministic metrics**: sections, words, the share of source units some section covers, repeated sentences, and which content categories sections use.

## Using scores

- Keep the provider, model, reasoning effort and judge model fixed between the two runs being compared. Use a judge from a different model family than the generator, so it does not grade its own style.
- Model judges vary. Treat a mean change under about 3 points, or one document moving alone, as noise until a repeat run agrees.
- The maintainer's spot checks of `guide.md` against the source are the ground truth the judge is checked against. Where they disagree, fix the rubric and bump `RUBRIC_VERSION` rather than tuning prompts to the judge.
