#!/usr/bin/env python3
"""Finalize Stage 3.11 source and documentation changes deterministically.

Temporary transport helper; the controlled runner deletes it before publishing
the resulting commit.
"""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    text = read(path)
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} replacement target(s), found {actual}")
    write(path, text.replace(old, new, count))


# ---------------------------------------------------------------------------
# Finish the generated-import extraction and canonical card-note mapping.
# ---------------------------------------------------------------------------
prepare = "src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs"
replace_exact(
    prepare,
    '''                GeneratedNoteDraft {\n                    card_type: "basic".to_string(),\n                    reverse_enabled,\n                    cloze_source: String::new(),\n                    cloze_mode: "one_at_a_time".to_string(),\n                    front_doc,\n                    back_doc,\n                    front_text: String::new(),\n                    back_text: String::new(),\n                },''',
    '''                CardNoteDraft::Basic {\n                    reverse_enabled,\n                    front_doc,\n                    back_doc,\n                },''',
)
replace_exact(
    prepare,
    '''        GeneratedImportCard::Cloze { text, mode, tags } => (\n            GeneratedNoteDraft {\n                card_type: "cloze".to_string(),\n                reverse_enabled: false,\n                cloze_source: text,\n                cloze_mode: mode,\n                front_doc: json!({"type": "doc", "content": []}),\n                back_doc: json!({"type": "doc", "content": []}),\n                front_text: String::new(),\n                back_text: String::new(),\n            },\n            tags,\n        ),''',
    '''        GeneratedImportCard::Cloze { text, mode, tags } => (\n            CardNoteDraft::Cloze { source: text, mode },\n            tags,\n        ),''',
)
replace_exact(prepare, "    combined.extend(card_tags);", "    combined.extend(card_tags.iter().cloned());")
replace_exact(prepare, "    let prepared = prepare_generated_note(draft)", "    let prepared = prepare_card_note(draft)")

import_mod = "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs"
replace_exact(import_mod, "use markdown::markdown_document;", "#[cfg(test)]\nuse markdown::markdown_document;")

card_types = "src-tauri/src/authoring/card_types_and_templates.rs"
replace_exact(
    card_types,
    '''fn display_card_type(note_type: &str) -> &str {\n    if note_type == "cloze" {\n        "cloze"\n    } else {\n        "basic"\n    }\n}\n\n''',
    '''fn display_card_type(note_type: &str) -> &str {\n    if note_type == "cloze" {\n        "cloze"\n    } else {\n        "basic"\n    }\n}\n\nfn validate_cloze_mode(value: &str) -> Result<(), CommandError> {\n    match value {\n        "one_at_a_time" | "all_together" => Ok(()),\n        _ => Err(CommandError::new(\n            "invalid_cloze_mode",\n            "Choose one-at-a-time or all-blanks-together Fill in the blank mode.",\n        )),\n    }\n}\n\n''',
)

# ---------------------------------------------------------------------------
# Batch template child-ID ownership checks. Validation can now be pure with
# respect to the submitted graph; no pending rows and no per-child ownership
# query are needed.
# ---------------------------------------------------------------------------
templates = "src-tauri/src/authoring/card_types_and_templates/templates.rs"
replace_exact(
    templates,
    '''    ensure_template_name_available(&transaction, &normalized_name, None)?;\n    let template_id = generate_id(&transaction)?;\n    let fields = validate_template_fields(&transaction, &template_id, input.fields, false)?;''',
    '''    ensure_template_name_available(&transaction, &normalized_name, None)?;\n    let template_id = generate_id(&transaction)?;\n    ensure_template_child_ids_available(&transaction, None, &input.fields, &input.rules)?;\n    let fields = validate_template_fields(&transaction, &template_id, input.fields, false)?;''',
)
replace_exact(
    templates,
    '''    ensure_template_name_available(&transaction, &normalized_name, Some(&input.template_id))?;\n    let fields = validate_template_fields(&transaction, &input.template_id, input.fields, true)?;''',
    '''    ensure_template_name_available(&transaction, &normalized_name, Some(&input.template_id))?;\n    ensure_template_child_ids_available(\n        &transaction,\n        Some(&input.template_id),\n        &input.fields,\n        &input.rules,\n    )?;\n    let fields = validate_template_fields(&transaction, &input.template_id, input.fields, true)?;''',
)

old_resolver = '''fn resolve_template_child_id(\n    connection: &Connection,\n    table: &str,\n    template_id: &str,\n    supplied: Option<String>,\n    updating: bool,\n    label: &str,\n) -> Result<String, CommandError> {\n    let Some(id) = supplied else {\n        return generate_id(connection);\n    };\n    validate_entity_id(&id, label)?;\n    let sql = match table {\n        "card_template_fields" => "SELECT template_id FROM card_template_fields WHERE id = ?1",\n        "card_template_rules" => "SELECT template_id FROM card_template_rules WHERE id = ?1",\n        _ => {\n            return Err(CommandError::new(\n                "invalid_template_child",\n                "The template contains an unsupported child record.",\n            ));\n        }\n    };\n    let owner: Option<String> = connection\n        .query_row(sql, [id.as_str()], |row| row.get(0))\n        .optional()\n        .map_err(CommandError::database)?;\n    match (updating, owner) {\n        (true, Some(owner)) if owner == template_id => Ok(id),\n        (true, Some(_)) | (false, Some(_)) => Err(CommandError::new(\n            "invalid_template_child",\n            "A template field or rule identifier is already in use.",\n        )),\n        (true, None) => Ok(id),\n        (false, None) => Ok(id),\n    }\n}\n\n'''
new_resolver = '''fn ensure_template_child_ids_available(\n    connection: &Connection,\n    template_id: Option<&str>,\n    fields: &[CardTemplateFieldInput],\n    rules: &[CardTemplateRuleInput],\n) -> Result<(), CommandError> {\n    let field_ids = fields\n        .iter()\n        .filter_map(|field| field.id.as_deref())\n        .collect::<Vec<_>>();\n    let rule_ids = rules\n        .iter()\n        .filter_map(|rule| rule.id.as_deref())\n        .collect::<Vec<_>>();\n    ensure_template_child_table_ids_available(\n        connection,\n        "card_template_fields",\n        template_id,\n        &field_ids,\n    )?;\n    ensure_template_child_table_ids_available(\n        connection,\n        "card_template_rules",\n        template_id,\n        &rule_ids,\n    )\n}\n\nfn ensure_template_child_table_ids_available(\n    connection: &Connection,\n    table: &str,\n    template_id: Option<&str>,\n    ids: &[&str],\n) -> Result<(), CommandError> {\n    if ids.is_empty() {\n        return Ok(());\n    }\n    for id in ids {\n        validate_entity_id(id, "template child")?;\n    }\n    let table = match table {\n        "card_template_fields" => "card_template_fields",\n        "card_template_rules" => "card_template_rules",\n        _ => {\n            return Err(CommandError::new(\n                "invalid_template_child",\n                "The template contains an unsupported child record.",\n            ));\n        }\n    };\n    let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(", ");\n    let sql = format!("SELECT id, template_id FROM {table} WHERE id IN ({placeholders})");\n    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;\n    let rows = statement\n        .query_map(params_from_iter(ids.iter()), |row| {\n            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))\n        })\n        .map_err(CommandError::database)?;\n    for row in rows {\n        let (_, owner) = row.map_err(CommandError::database)?;\n        if template_id != Some(owner.as_str()) {\n            return Err(CommandError::new(\n                "invalid_template_child",\n                "A template field or rule identifier is already in use.",\n            ));\n        }\n    }\n    Ok(())\n}\n\nfn resolve_template_child_id(\n    connection: &Connection,\n    table: &str,\n    _template_id: &str,\n    supplied: Option<String>,\n    _updating: bool,\n    label: &str,\n) -> Result<String, CommandError> {\n    if !matches!(table, "card_template_fields" | "card_template_rules") {\n        return Err(CommandError::new(\n            "invalid_template_child",\n            "The template contains an unsupported child record.",\n        ));\n    }\n    let Some(id) = supplied else {\n        return generate_id(connection);\n    };\n    validate_entity_id(&id, label)?;\n    Ok(id)\n}\n\n'''
replace_exact(templates, old_resolver, new_resolver)

# Expand the regression test to prove a child ID owned by another template is
# rejected without relying on pending rows.
replace_exact(
    templates,
    '''            .expect("unused IDs should validate without persistence");\n    }\n}\n\nmod automatic_layout;''',
    '''            .expect("unused IDs should validate without persistence");\n    }\n\n    #[test]\n    fn update_rejects_child_id_owned_by_another_template() {\n        let database = Database::open_in_memory().expect("database should open");\n        database\n            .with_connection(|connection| {\n                let now = unix_time_millis()?;\n                let owner = "11111111111111111111111111111111";\n                let target = "22222222222222222222222222222222";\n                let child = "33333333333333333333333333333333";\n                connection\n                    .execute(\n                        "INSERT INTO card_templates (id, name, normalized_name, description, created_at, updated_at, revision) VALUES (?1, 'Owner', 'owner', '', ?2, ?2, 1)",\n                        params![owner, now],\n                    )\n                    .map_err(CommandError::database)?;\n                connection\n                    .execute(\n                        "INSERT INTO card_template_fields (id, template_id, name, normalized_name, description, position) VALUES (?1, ?2, 'Field', 'field', '', 0)",\n                        params![child, owner],\n                    )\n                    .map_err(CommandError::database)?;\n                let error = ensure_template_child_ids_available(\n                    connection,\n                    Some(target),\n                    &[CardTemplateFieldInput {\n                        id: Some(child.to_string()),\n                        name: "Field".to_string(),\n                        description: String::new(),\n                    }],\n                    &[],\n                )\n                .expect_err("foreign child IDs must be rejected");\n                assert_eq!(error.code, "invalid_template_child");\n                Ok(())\n            })\n            .expect("ownership check should run");\n    }\n}\n\nmod automatic_layout;''',
)

# ---------------------------------------------------------------------------
# Generated import: preload destination normalized identities once instead of
# issuing one duplicate query for every source note.
# ---------------------------------------------------------------------------
apply = "src-tauri/src/authoring/card_types_and_templates/generated_import/apply.rs"
replace_exact(
    apply,
    '''    let mut seen = HashSet::new();\n    let mut created_notes = 0_i64;''',
    '''    let destination_duplicates = load_destination_duplicate_pairs(&transaction, &deck_id)?;\n    let mut seen = HashSet::new();\n    let mut created_notes = 0_i64;''',
)
replace_exact(
    apply,
    '''        let duplicate_in_batch = !seen.insert(normalized.clone());\n        let duplicate_in_deck = has_exact_duplicate(\n            &transaction,\n            &deck_id,\n            &duplicate_hash,\n            &normalized.0,\n            &normalized.1,\n            None,\n        )?;''',
    '''        let duplicate_in_batch = !seen.insert(normalized.clone());\n        let duplicate_in_deck = destination_duplicates.contains(&normalized);''',
)
replace_exact(
    apply,
    '''fn resolve_generated_import_deck(\n''',
    '''fn load_destination_duplicate_pairs(\n    connection: &Connection,\n    deck_id: &str,\n) -> Result<HashSet<(String, String)>, CommandError> {\n    let mut statement = connection\n        .prepare(\n            "SELECT front_text, back_text FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",\n        )\n        .map_err(CommandError::database)?;\n    let rows = statement\n        .query_map([deck_id], |row| {\n            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))\n        })\n        .map_err(CommandError::database)?;\n    let mut duplicates = HashSet::new();\n    for row in rows {\n        let (front, back) = row.map_err(CommandError::database)?;\n        duplicates.insert(normalized_pair(&front, &back));\n    }\n    Ok(duplicates)\n}\n\nfn resolve_generated_import_deck(\n''',
)

# The imported note still persists its already-computed candidate hash.
if "duplicate_hash," not in read(apply):
    raise SystemExit("generated import duplicate hash persistence unexpectedly absent")

# ---------------------------------------------------------------------------
# Semantic naming for active repair code; durable repair ledger names/checksums
# remain untouched.
# ---------------------------------------------------------------------------
for path in (ROOT / "src-tauri" / "src").rglob("*.rs"):
    text = path.read_text(encoding="utf-8")
    if "apply_stage3_data_repairs" in text:
        path.write_text(
            text.replace("apply_stage3_data_repairs", "apply_card_type_template_repairs"),
            encoding="utf-8",
        )

# ---------------------------------------------------------------------------
# Documentation and agent-instruction alignment.
# ---------------------------------------------------------------------------
architecture = "docs/ARCHITECTURE.md"
replace_exact(
    architecture,
    "- `src/ipc.ts` is the only frontend module that invokes Rust commands.",
    "- Typed IPC gateway modules (`src/ipc.ts`, `src/analytics-ipc.ts`, `src/generatedImport.ts`, and `src/diagnostics.ts`) are the only frontend modules that invoke Rust commands; screens and components call those wrappers.",
)
replace_exact(
    architecture,
    '''- `database.rs` opens SQLite, applies checksummed migrations, backfills stable\n  duplicate fingerprints, runs idempotent forward normalization, and exposes one\n  mutex-protected connection.\n- `decks.rs` owns deck validation and persistence.\n- `cards.rs` owns source-note hydration, per-deck list/delete behavior, and the\n  canonical Tiptap validator.\n- `authoring.rs` remains the card-authoring boundary. Its Stage 2 modules own\n  tags, FTS5, duplicates, bulk entry, assets, and note-level actions. Its\n  `authoring/stage3_v2/` modules own card-type validation, cloze parsing,\n  generated-card lifecycle, template CRUD, typed-placeholder resolution,\n  automatic rich-value placement, and generated-card rendering.\n- `assets.rs` imports supported local image files into the managed library.''',
    '''- `database.rs` opens SQLite, applies immutable checksummed migrations, invokes\n  required versioned data repairs at their declared migration boundaries, and\n  exposes one mutex-protected connection. Once the repair ledger is complete,\n  canonical startup verifies it without rewriting application data.\n- `data_repairs.rs` owns one-time checksummed data repairs; historical repair\n  names and checksums are durable compatibility records.\n- `decks.rs` owns deck validation and persistence.\n- `cards.rs` owns source-note hydration and the canonical Tiptap validator; Rust\n  derives authoritative searchable plain text from validated documents.\n- `authoring/mod.rs` owns shared authoring contracts and persistence primitives.\n  `authoring/card_commands.rs` owns search/tags/bulk/read commands,\n  `authoring/card_types_and_templates.rs` owns canonical Basic/Cloze note\n  preparation and presentation rendering, its `templates/` subtree owns reusable\n  template validation/rendering/persistence, `generated_import/` owns the\n  external version-1 JSON schema/restricted Markdown/preparation/apply pipeline,\n  and `authoring/transfer.rs` owns note move/copy behavior.\n- `assets.rs` imports supported local image files into the managed library.''',
)
replace_exact(
    architecture,
    "Startup normalization converts it to the neutral\nrepresentation without a new database migration.",
    "A versioned data repair converts it to the neutral representation. Canonical startup only verifies that the required repair was recorded.",
)
# Add generated JSON boundary after deck interchange paragraph if not already present.
replace_exact(
    architecture,
    '''Older Basic packages without optional Stage 2 or Stage 3 tables remain\nimportable. Older Stage 3 packages with several cloze rows are normalized to one\nactive card immediately after import. Older readers can ignore additional\noptional tables in new exports.\n''',
    '''Older Basic packages without optional tag/card-type tables remain importable.\nOlder packages with several cloze rows are normalized to one active card\nimmediately after import. Older readers can ignore additional optional tables in\nnew exports.\n\nGenerated `.ostudy.json` is a separate content-only external format. Version 1\ncontains one deck destination (`create` or exact-name `append`) plus Basic/Cloze\nsource notes. Rust validates the entire file, parses only the documented\nrestricted Markdown subset, maps entries through the canonical card-note\npreparation path, skips exact duplicates, and applies accepted notes in one\ntransaction with fresh local schedules. There is deliberately no generated-import\npreview or inspect/commit protocol.\n''',
)

model = "docs/CARD-FORMAT-AND-DATA-MODEL.md"
replace_exact(model, "- `note_type`: `basic` or `cloze` for active Stage 3 authoring", "- `note_type`: `basic` or `cloze` for active authoring")
replace_exact(
    model,
    '''Saved rule documents use one neutral inert placeholder node. The older\n`templateFieldBlock` node remains readable only for compatibility with templates\ncreated by the first schema-7 build. Startup normalization rewrites those nodes\nto the neutral representation without requiring a new migration.''',
    '''Saved rule documents use one neutral inert placeholder node. The older\n`templateFieldBlock` node remains readable only for compatibility with templates\ncreated by the first schema-7 build. A checksummed one-time data repair rewrites\nthose nodes to the neutral representation; normal canonical startup does not\nrepeat that write.''',
)
replace_exact(
    model,
    '''## Notes, generated cards, and tags\n''',
    '''## Authoring command contract\n\nThe bundled editor submits a strict Basic-or-Cloze tagged request. Basic writes\ncarry canonical rich question/answer documents plus reverse configuration;\nCloze writes carry the bounded cloze source and mode. Type-inappropriate and\nunknown properties are rejected. Browser plain-text snapshots are not part of\nthe write contract: Rust derives `front_text` and `back_text` from the validated\ndocuments and enforces the authoritative text limits there.\n\nThe older Basic-only `create_organized_card` and `update_organized_card` IPC\ncommands are removed rather than retained as compatibility shims because bundled\nTauri IPC is an internal application contract.\n\n## Notes, generated cards, and tags\n''',
)

# Generated import docs: format unchanged, grammar clarified.
generated_doc = "docs/GENERATED-JSON-IMPORT.md"
replace_exact(
    generated_doc,
    '''Raw HTML, embedded or remote images, local file paths, tables, executable content, and unknown syntax are rejected. Rust converts accepted Markdown into canonical Tiptap JSON, derives authoritative plain text, and runs the normal card-content validator.''',
    '''Raw HTML, embedded or remote images, local file paths, tables, executable content, and unknown syntax are rejected. Literal comparison characters such as `2 < 3` or `5 > 4` are ordinary text and are not treated as HTML. The restricted parser decides which Markdown syntax is supported; Rust then converts accepted syntax into canonical Tiptap JSON and runs the ordinary card-content validator, which remains authoritative for safe rich-document structure, links, sizes, and derived plain text.''',
)
agent_import = "docs/OSTUDY-FLASHCARD-IMPORT-AGENT.md"
replace_exact(
    agent_import,
    '''Do not use raw HTML, images, local file paths, executable content, or Markdown tables in version 1. Represent table material as a concise list or code block.''',
    '''Do not use raw HTML, images, local file paths, executable content, or Markdown tables in version 1. Represent table material as a concise list or code block. Ordinary comparison text may contain literal `<` or `>` characters; do not wrap it in HTML tags.''',
)

# Root agent validation now reflects the tiered gate.
agents = "AGENTS.md"
replace_exact(
    agents,
    '''## Required validation\n\nRun the narrowest relevant tests while developing, then run the repository gate\nbefore proposing a merge:\n\n```bash\n./scripts/check.sh\n```\n\nFor release qualification:\n\n```bash\n./scripts/release-check.sh\n```\n\nManually exercise every affected UI workflow. Static inspection, parsing, or a\nsuccessful frontend build must never be reported as a successful native Tauri\ntest.''',
    '''## Required validation\n\nUse the repository's tiered validation model instead of repeatedly running the\nmost expensive gate while editing:\n\n```bash\n./scripts/check-changed.sh  # iteration aid based on the current diff\n./scripts/check-static.sh   # repository/IPC/type/format contracts\n./scripts/check-fast.sh     # static + complete frontend tests + Rust compile\n./scripts/check.sh          # authoritative full merge qualification\n```\n\nDraft pull requests run the fast tier; ready pull requests run the full tier on\nthe self-hosted runner. A green result qualifies only the exact commit SHA it\nran against. Use `OSTUDY_FORCE_CHECK=1` when an exact-tree cache must be bypassed.\nFor release qualification, also run `./scripts/release-check.sh`.\n\nManually exercise every affected UI workflow. Static inspection, parsing, or a\nsuccessful frontend build must never be reported as a successful native Tauri\ntest. See `docs/development/testing-and-ci.md` for the authoritative tier details.''',
)

workflow_doc = "docs/agents/WORKFLOW-AND-VALIDATION.md"
old_validation = '''## 4. Validate in layers\n\nRun narrow tests during implementation. Before proposing a merge, run:\n\n```bash\n./scripts/check.sh\n```\n\nThis is the single repository gate: it runs frontend tests, Rust checks, release\nschema fixtures, the standalone build, and the native smoke test. Do not require\ncontributors to invoke each underlying command separately.\n\nFor a release candidate, also run:\n\n```bash\n./scripts/release-check.sh\n```\n\nManually exercise the affected UI path with `./run-app.sh`. For UI work, include'''
new_validation = '''## 4. Validate in layers\n\nUse the cheapest gate that answers the current question, then escalate as the\nchange stabilizes:\n\n```bash\n./scripts/check-changed.sh  # diff-aware iteration helper\n./scripts/check-static.sh   # manifests, IPC, policy, typecheck, rustfmt\n./scripts/check-fast.sh     # static + all frontend tests + locked Rust compile\n./scripts/check.sh          # full merge qualification and native smoke\n```\n\nDraft pull requests are expected to pass the fast tier. Marking a pull request\nready triggers full qualification. Always inspect the result for the exact head\nSHA; an older green run does not qualify a newer commit. `OSTUDY_FORCE_CHECK=1`\nforces re-execution when exact-tree cache reuse should be bypassed. For a release\ncandidate, also run `./scripts/release-check.sh`.\n\nManually exercise the affected UI path with `./run-app.sh`. For UI work, include'''
replace_exact(workflow_doc, old_validation, new_validation)

# IPC guardrail supports several explicit typed gateway modules.
guards = "docs/agents/ARCHITECTURE-GUARDRAILS.md"
replace_exact(
    guards,
    '''All frontend-to-Rust invocation belongs in `src/ipc.ts`. Screens and components\nshould call typed wrapper functions rather than invoke arbitrary command names.''',
    '''Frontend-to-Rust invocation belongs only in explicitly designated typed IPC\ngateway modules (currently `src/ipc.ts`, `src/analytics-ipc.ts`,\n`src/generatedImport.ts`, and `src/diagnostics.ts`). Screens and components must\ncall typed wrappers rather than `invoke()` directly. The repository IPC contract\ncheck verifies exact parity between every wrapper command name and the Rust\nregistry.''',
)

# Tool-neutral publication workflow: retain worktrees as a useful local option,
# but do not claim they are the only valid authenticated environment.
git_workflow = "docs/OSTUDY-GIT-AND-CODING-WORKFLOW.md"
replace_exact(git_workflow, "# Ostudy Git and Coding Workflow — Corrected Publication-First Version", "# Ostudy Git and Coding Workflow — Publication-First, Tool-Neutral Version")
start_text = read(git_workflow)
start = start_text.index("## 1. Establish the exact base without editing the primary checkout")
end = start_text.index("## 3. Publish the first meaningful commit immediately", start)
new_sections = '''## 1. Establish and publish the exact base\n\nUse the authenticated repository mechanism available in the current environment.\nResolve the exact current `main` commit, create one short-lived\n`agent/<description>` branch from that commit, and publish that branch before\nsubstantial implementation begins. Do not develop directly on `main`.\n\nA local coding agent should normally protect the human's primary checkout by\nusing an isolated Git worktree. A connector-backed environment may create and\nupdate the remote task branch directly. The invariant is the same in either\ncase: exact base, one focused branch, one recoverable remote history.\n\nFor private repositories, use the authenticated connector/credential path that\nthe environment provides. Do not substitute anonymous raw GitHub URLs, scraping,\ntemporary transfer repositories, opaque/base64 payload branches, or similar\nworkarounds for authenticated repository access.\n\nLocal-worktree example:\n\n```bash\ncd ~/code/ostudy\ngit status --short\ngit fetch origin --prune\nBASE_SHA="$(git rev-parse origin/main)"\nBRANCH="agent/<description>"\nWORKTREE="../ostudy-<description>"\ngit worktree add -b "$BRANCH" "$WORKTREE" "$BASE_SHA"\ncd "$WORKTREE"\ngit push -u origin HEAD\n```\n\n## 2. Publication-first rule\n\nThe remote branch and draft pull request are the durable task record. Publish\nearly, then push coherent implementation increments to that same branch. Never\nleave the user with only an agent-local patch, temporary archive, or description\nof code that was not published.\n\n'''
write(git_workflow, start_text[:start] + new_sections + start_text[end:])

# Development docs now point to the behavior-based tier definitions.
dev = "docs/DEVELOPMENT-AND-QUALITY.md"
replace_exact(
    dev,
    '''The process is intentionally small:\n\n1. publish a focused branch and draft pull request from an exact base commit;\n2. implement and push reviewable vertical commits to that branch;\n3. run `./scripts/check.sh` locally and through GitHub Actions;\n4. launch `./run-app.sh` and manually test the changed UI path;\n5. merge only after automated and manual acceptance are satisfactory.''',
    '''The process is intentionally small:\n\n1. publish one focused branch and draft pull request from an exact base commit;\n2. implement and push reviewable vertical commits to that branch;\n3. iterate with changed/static/fast checks and consume exact-head CI failures;\n4. run `./scripts/check.sh`, launch `./run-app.sh`, and manually test the affected workflow;\n5. mark the PR ready so the self-hosted runner qualifies the exact head with the full gate;\n6. merge only after automated and human acceptance are satisfactory.''',
)
replace_exact(
    dev,
    '''./scripts/build.sh               # rebuild after source changes\n./run-app.sh                     # start the existing build without compiling\n./scripts/check.sh               # the complete automated development gate\n./scripts/export-diagnostics.sh  # create a privacy-conscious support archive''',
    '''./scripts/build.sh               # rebuild after source changes\n./run-app.sh                     # start the existing build without compiling\n./scripts/check-changed.sh       # diff-aware iteration aid\n./scripts/check-static.sh        # cheap repository/IPC/type contracts\n./scripts/check-fast.sh          # frontend tests + Rust compile\n./scripts/check.sh               # complete merge qualification\n./scripts/export-diagnostics.sh  # create a privacy-conscious support archive''',
)
old_gate = '''## What the one-command gate checks\n\n`./scripts/check.sh` performs, in order:\n\n1. pinned environment and repository-manifest diagnostics;\n2. Stage 2 migration, FTS5, tag, bulk, duplicate, and move/copy wiring checks;\n3. Stage 3 migration, generated-card identity, cloze, template-safety,\n   Search/Filter, Practice, responsive UI, and `.studydeck` wiring checks;\n4. frozen pnpm installation and exact dependency verification;\n5. Vitest/Testing Library React tests;\n6. Rust formatting and warning-free Clippy;\n7. Rust unit/integration tests, including every release-schema fixture;\n8. a standalone Tauri build suitable for `run-app.sh`;\n9. resolved Rust dependency verification;\n10. a native Linux Tauri/WebDriver smoke test under an isolated data directory.'''
new_gate = '''## Qualification tiers\n\n`check-static.sh` verifies durable environment/repository contracts: exact tool and\ndependency versions, migration checksums, IPC registry/wrapper parity, repository\npolicy, test policy, script syntax, TypeScript, and canonical Rust formatting.\n\n`check-fast.sh` reuses an exact-tree static result when possible and adds the full\nfrontend suite plus locked Rust compile checking. Draft pull requests use this\ntier.\n\n`check.sh` reuses an exact-tree fast result when possible and adds native\nenvironment qualification, production frontend build, warning-free Clippy, all\nRust/unit/integration/migration tests, standalone Tauri build, exact dependency\nverification, and the native create → review → restart → persistence smoke test.\nReady pull requests use this tier. See `docs/development/testing-and-ci.md` for\nthe authoritative mechanics.'''
replace_exact(dev, old_gate, new_gate)
replace_exact(
    dev,
    '''- **Repository verifiers:** inspect durable migration checksums, command\n  registration, typed IPC, stale API removal, template safety constraints,\n  generated-card rendering, and required UI/documentation wiring.''',
    '''- **Repository contracts:** verify durable migration checksums, exact command\n  registry/frontend-wrapper parity, stale API removal, self-hosted immutable\n  workflow policy, dependency manifests, and test-policy invariants. Feature\n  behavior is protected by executable tests rather than private source shapes.''',
)

readme = "README.md"
replace_exact(
    readme,
    "- Import and export complete `.studydeck` deck files, including optional tag\n  metadata.",
    "- Import and export complete `.studydeck` deck files, and directly import strict content-only `.ostudy.json` files generated by an LLM or other authoring tool.",
)
replace_exact(
    readme,
    '''## Test a change\n\nRun the complete automated gate from the repository root:\n\n```bash\n./scripts/check.sh\n```\n\nThe gate installs from the committed lockfiles, verifies the Stage 2 data and\ncommand wiring, runs React tests, checks Rust formatting and Clippy, runs Rust\nand database tests, creates a standalone Tauri build, and performs a native\ndesktop smoke test. After it passes, start the app with `./run-app.sh` and\nmanually exercise the changed workflow.''',
    '''## Test a change\n\nUse the tier that matches the current stage of development:\n\n```bash\n./scripts/check-changed.sh\n./scripts/check-static.sh\n./scripts/check-fast.sh\n./scripts/check.sh\n```\n\nThe static tier verifies environment, migration, dependency, IPC, repository,\ntype, formatting, and test-policy contracts. Fast adds the complete frontend\nsuite and locked Rust compile checking. The full gate adds production/native\nbuilds, Clippy, all Rust/database/migration tests, dependency verification, and a\nreal Tauri/WebDriver create → review → restart persistence smoke test. After the\nfull gate passes, start the app with `./run-app.sh` and manually exercise the\nchanged workflow. See `docs/development/testing-and-ci.md` for details.''',
)

print("Applied Stage 3.11 compiler, scale, and documentation finalization.")
