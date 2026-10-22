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

## Rubric (version 3)

A study guide is a condensed summary of the key ideas that sends the reader to the source, flashcards or lectures for the detail. It is not a rewrite of the source. Version 2 replaced version 1's coverage criterion, which rewarded restating every fact, with key ideas, distillation and concision. Version 3 lets the whole-guide judge read the whole source and guide (version 2 clipped both at 48,000 characters, so a long chapter's key ideas were judged from its first pages), counts key ideas instead of anchoring them, and moves the concision band to a tenth. The judge grades each guide section against the source chunks its locator names, and grades the whole guide against the whole source. Judged criteria are scored 1 to 5 with the anchors written into the judge prompts in `scripts/guide-eval.py`, then mapped to 0-100.

| Criterion | Weight | Judged on | What it asks |
| --- | --- | --- | --- |
| Accuracy | 25% | each section, weighted by its length | Are the statements true to the source? Wrong or misleading statements are listed as major. |
| Key ideas | 20% | whole guide | The judge lists, in source order, the ideas a summary a tenth of the source's length must keep (not supporting facts, values, examples or named techniques), and marks each taught, thin or missing in the guide. The score is the share taught, a thin idea counting half. |
| Distillation | 15% | whole guide | Do the key ideas stand out, with details left to the source or gathered in tables, or does the guide restate the source? |
| Concision | 10% | measured | Guide words per source word. From 0.05 to 0.12 scores 100. Above 0.12 the score falls, reaching 0 at 0.25. A tenth is the aim and a fifth the ceiling. |
| Source support | 10% | each section | Can each substantive statement be traced to the source? Correct outside knowledge still counts against it. |
| Readability | 10% | each section | Scannable and clear, with comparable items in tables or short lists rather than paragraphs that recite fact after fact. |
| Organization | 5% | whole guide | Learning order, section granularity, duplication, a framing overview. |
| Pointers | 5% | whole guide | Does each section tell the student where the details are: source pages, sections or cards? |

Scores from different rubric versions are not comparable; `compare` warns, and both runs should be rescored with the same version (saved verdicts from another version must be deleted first).

The guide score is the weighted mean. A run that publishes no guide scores 0 for that document. Reported beside the score, never folded into it:

- **Major unsupported claims**, the count a student would be misled by. A change that raises the score but adds major errors is a regression.
- **Cost**: provider requests, tokens and reported cost from `scripts/generation-usage.py`, plus wall time.
- **Deterministic metrics**: sections, guide and source words and their ratio, the share of source units some section covers, repeated sentences, and which content categories sections use.

## Using scores

- Keep the provider, model, reasoning effort and judge model fixed between the two runs being compared. Use a judge from a different model family than the generator, so it does not grade its own style.
- Model judges vary. Treat a mean change under about 3 points, or one document moving alone, as noise until a repeat run agrees.
- The maintainer's spot checks of `guide.md` against the source are the ground truth the judge is checked against. Where they disagree, fix the rubric and bump `RUBRIC_VERSION` rather than tuning prompts to the judge.
