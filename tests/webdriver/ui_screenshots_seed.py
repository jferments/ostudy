"""Representative library content for UI screenshot review.

The screenshot harness seeds a fresh, disposable library in two phases:

1. ``seed_through_ipc`` runs inside the running app through the same native
   commands the UI uses: subjects, decks, notes of every authoring family,
   tags, a tutor conversation, and real practice sessions that produce review
   history.
2. ``finalize_library`` runs after the app has exited and edits the library the
   app created: it gives tags fixed ids, moves the review history onto fixed
   days before the capture date, gives every content row a stable timestamp,
   pins the Practice deck's due order and the finished subject's next reviews,
   and adds documents filed in their subjects, learning plans, study guides,
   and tutor turns that would otherwise need an AI provider.

Nothing here adds a production code path. Content is fixed; only calendar dates
move with the capture day because the app reads the real clock.
"""

from __future__ import annotations

import hashlib
import json
import sqlite3
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

SEED_VERSION = "ui-screenshots-seed-v2"
DAY_MS = 86_400_000
MINUTE_MS = 60_000

# --------------------------------------------------------------------------
# Rich text helpers (validated Tiptap JSON, never HTML)
# --------------------------------------------------------------------------


def text_run(value: str, *marks: str) -> dict[str, Any]:
    run: dict[str, Any] = {"type": "text", "text": value}
    if marks:
        run["marks"] = [{"type": mark} for mark in marks]
    return run


def paragraph(*runs: str | dict[str, Any]) -> dict[str, Any]:
    content = [text_run(run) if isinstance(run, str) else run for run in runs]
    return {"type": "paragraph", "content": content}


def bullets(*items: str) -> dict[str, Any]:
    return {
        "type": "bulletList",
        "content": [{"type": "listItem", "content": [paragraph(item)]} for item in items],
    }


def doc(*blocks: str | dict[str, Any]) -> dict[str, Any]:
    return {
        "type": "doc",
        "content": [paragraph(block) if isinstance(block, str) else block for block in blocks],
    }


def stable_hex(*parts: str) -> str:
    """A deterministic 32-hex identifier for fixture-owned identities."""
    return hashlib.sha256("\x1f".join((SEED_VERSION, *parts)).encode("utf-8")).hexdigest()[:32]


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


# --------------------------------------------------------------------------
# Library content
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class DeckSpec:
    key: str
    name: str
    description: str
    subject: str | None  # None files the deck under No subject


@dataclass(frozen=True)
class NoteSpec:
    key: str
    deck: str
    kind: str  # basic | cloze | question
    tags: tuple[str, ...]
    front: str = ""
    back: str = ""
    front_doc: dict[str, Any] | None = None
    back_doc: dict[str, Any] | None = None
    reverse: bool = False
    cloze: str = ""
    question: dict[str, Any] | None = None


# Subjects in the learner's order, which is also their creation order (and so
# their colors on Home). Physiology has decks and a document with a guide,
# Biochemistry has only decks, Pharmacology has a deck and a document,
# Microbiology has nothing due today, and one deck and one document (the
# renal lecture, not yet filed) have no subject.
SUBJECTS = ("Physiology", "Biochemistry", "Pharmacology", "Microbiology")
# The subject whose page the capture shows: it has decks and a filed document.
SUBJECT_PAGE = "Physiology"

DECKS: tuple[DeckSpec, ...] = (
    DeckSpec("cardio", "Cardiovascular physiology", "Cardiac cycle, hemodynamics, and blood pressure control.", "Physiology"),
    DeckSpec("renal", "Renal physiology", "Nephron transport, filtration, and water balance.", "Physiology"),
    DeckSpec("resp", "Respiratory physiology", "Ventilation, gas exchange, and oxygen transport.", "Physiology"),
    DeckSpec("metab", "Metabolism", "Glycolysis, the TCA cycle, and fuel regulation.", "Biochemistry"),
    DeckSpec("molbio", "Molecular biology", "Replication, transcription, and translation.", "Biochemistry"),
    DeckSpec("pharm", "Pharmacology essentials", "Pharmacokinetics and autonomic drugs.", "Pharmacology"),
    DeckSpec("micro", "Bacterial structure", "Cell walls, spores, and gene transfer.", "Microbiology"),
    DeckSpec("terms", "Medical terminology", "Prefixes, suffixes, and word roots.", None),
)

# The deck whose Practice flow is captured. Its cards get distinct due dates so
# the one-click Practice order is the same on every run.
PRACTICE_DECK = "renal"
PRACTICE_ORDER = ("renal-glucose", "renal-macula", "renal-adh", "renal-gfr", "renal-aldosterone")
# Ratings used while capturing Practice: 0 Wrong, 1 Hard, 2 Medium, 3 Easy.
PRACTICE_RATINGS = (2, 3, 0, 2, 3)

# Every card of this deck has been reviewed and is next due after the capture
# day, so its subject shows "All done" on Home.
FINISHED_DECK = "micro"

# The note opened in the card editor screenshot.
EDITOR_NOTE = "cardio-oxygen-demand"
# The deck opened for the deck card-list screenshot.
DETAIL_DECK = "cardio"


def _choices(key: str, labels: list[str]) -> list[dict[str, Any]]:
    return [
        {"id": stable_hex(key, "option", label), "content": doc(label), "rationale": None}
        for label in labels
    ]


def _items(key: str, labels: list[str], role: str = "item") -> list[dict[str, Any]]:
    return [{"id": stable_hex(key, role, label), "content": doc(label)} for label in labels]


def _single_choice(key: str, prompt: str, labels: list[str], correct: int, explanation: str) -> dict[str, Any]:
    options = _choices(key, labels)
    return {
        "kind": "single_choice",
        "prompt": doc(prompt),
        "options": options,
        "correctOptionId": options[correct]["id"],
        "explanation": doc(explanation),
    }


def _select_all(key: str, prompt: str, labels: list[str], correct: list[int], explanation: str) -> dict[str, Any]:
    options = _choices(key, labels)
    return {
        "kind": "select_all",
        "prompt": doc(prompt),
        "options": options,
        "correctOptionIds": [options[index]["id"] for index in correct],
        "explanation": doc(explanation),
    }


def _true_false(statement: str, correct: bool, explanation: str, corrected: str | None = None) -> dict[str, Any]:
    return {
        "kind": "true_false",
        "statement": doc(statement),
        "correct": correct,
        "explanation": doc(explanation),
        "correctedStatement": doc(corrected) if corrected else None,
    }


def _matching(key: str, prompt: str, pairs: list[tuple[str, str]]) -> dict[str, Any]:
    return {
        "kind": "matching",
        "prompt": doc(prompt),
        "pairs": [
            {
                "left": {"id": stable_hex(key, "left", left), "content": doc(left)},
                "right": {"id": stable_hex(key, "right", right), "content": doc(right)},
                "rationale": None,
            }
            for left, right in pairs
        ],
        "explanation": None,
    }


def _ordering(key: str, prompt: str, labels: list[str]) -> dict[str, Any]:
    return {"kind": "ordering", "prompt": doc(prompt), "items": _items(key, labels), "explanation": None}


def _grouping(key: str, prompt: str, groups: dict[str, list[str]]) -> dict[str, Any]:
    categories = _items(key, list(groups), "category")
    category_ids = {category_name: category["id"] for category_name, category in zip(groups, categories)}
    items = [
        {
            "item": {"id": stable_hex(key, "item", label), "content": doc(label)},
            "categoryId": category_ids[category_name],
            "rationale": None,
        }
        for category_name, labels in groups.items()
        for label in labels
    ]
    return {"kind": "grouping", "prompt": doc(prompt), "categories": categories, "items": items, "explanation": None}


NOTES: tuple[NoteSpec, ...] = (
    # Cardiovascular physiology
    NoteSpec("cardio-stroke-volume", "cardio", "basic", ("cardiovascular", "exam-1"),
             front="Stroke volume", back="Volume of blood ejected by a ventricle in one beat (EDV − ESV).", reverse=True),
    NoteSpec("cardio-frank-starling", "cardio", "basic", ("cardiovascular", "high-yield"),
             front="What does the Frank–Starling mechanism describe?",
             back="Stroke volume rises as end-diastolic volume (preload) rises, within physiological limits."),
    NoteSpec(EDITOR_NOTE, "cardio", "basic", ("cardiovascular", "high-yield", "exam-1"),
             front_doc=doc(paragraph("Which factors increase ", text_run("myocardial oxygen demand", "bold"), "?")),
             back_doc=doc(
                 paragraph("Demand rises with anything that increases cardiac work:"),
                 bullets(
                     "Heart rate — the strongest single determinant",
                     "Contractility (sympathetic stimulation, inotropes)",
                     "Wall tension: afterload and ventricular radius (Laplace's law)",
                 ),
                 paragraph(text_run("Clinical link:", "bold"), " beta blockers relieve angina by lowering heart rate and contractility."),
             )),
    NoteSpec("cardio-output", "cardio", "cloze", ("cardiovascular", "exam-1"),
             cloze="Cardiac output = {{c1::heart rate}} × {{c2::stroke volume}}"),
    NoteSpec("cardio-s2", "cardio", "basic", ("cardiovascular",),
             front="Which heart sound marks closure of the aortic and pulmonary valves?",
             back="S2 — it marks the end of ventricular systole."),
    NoteSpec("cardio-baroreceptors", "cardio", "question", ("cardiovascular", "high-yield"),
             question=_single_choice(
                 "cardio-baroreceptors",
                 "Mean arterial pressure rises abruptly. Which receptors detect the change first?",
                 ["Carotid sinus baroreceptors", "Peripheral chemoreceptors", "Juxtaglomerular cells", "Atrial volume receptors"],
                 0,
                 "Carotid sinus and aortic arch baroreceptors respond to stretch within a single beat.")),
    NoteSpec("cardio-preload", "cardio", "question", ("cardiovascular",),
             question=_select_all(
                 "cardio-preload",
                 "Which changes increase cardiac preload?",
                 ["Increased venous return", "Venoconstriction", "Atrial contraction", "Arteriolar vasodilation", "Standing up quickly"],
                 [0, 1, 2],
                 "Preload depends on ventricular filling; standing pools blood in the legs and lowers it.")),
    NoteSpec("cardio-ecg", "cardio", "question", ("cardiovascular", "exam-1"),
             question=_matching(
                 "cardio-ecg",
                 "Match each ECG feature with the event it represents.",
                 [("P wave", "Atrial depolarization"), ("QRS complex", "Ventricular depolarization"),
                  ("T wave", "Ventricular repolarization"), ("PR interval", "AV nodal conduction delay")])),
    NoteSpec("cardio-resistance", "cardio", "basic", ("cardiovascular",),
             front="What is the main determinant of resistance to blood flow?",
             back="Vessel radius — resistance is inversely proportional to the fourth power of the radius (Poiseuille's law)."),
    NoteSpec("cardio-afterload", "cardio", "question", ("cardiovascular",),
             question=_true_false(
                 "Increasing afterload increases stroke volume.", False,
                 "A ventricle ejects less blood against a higher pressure.",
                 "Increasing afterload decreases stroke volume.")),
    # Renal physiology (the captured Practice deck: recall cards only)
    NoteSpec("renal-glucose", "renal", "basic", ("renal", "high-yield"),
             front="Where is most filtered glucose reabsorbed?",
             back="In the proximal convoluted tubule, through SGLT2 and SGLT1 cotransporters."),
    NoteSpec("renal-macula", "renal", "basic", ("renal", "exam-2"),
             front="What does the macula densa sense?",
             back="NaCl delivery to the distal tubule, which drives tubuloglomerular feedback."),
    NoteSpec("renal-adh", "renal", "basic", ("renal", "high-yield"),
             front="Which hormone inserts aquaporin-2 channels into collecting duct cells?",
             back="Antidiuretic hormone (vasopressin), acting through V2 receptors."),
    NoteSpec("renal-gfr", "renal", "basic", ("renal", "exam-2"),
             front="What is a normal glomerular filtration rate in a healthy adult?",
             back="About 125 mL/min, or roughly 180 L per day."),
    NoteSpec("renal-aldosterone", "renal", "basic", ("renal", "exam-2"),
             front="How does aldosterone act on principal cells?",
             back="It increases ENaC and Na⁺/K⁺-ATPase activity, promoting sodium reabsorption and potassium secretion."),
    # Respiratory physiology
    NoteSpec("resp-surfactant", "resp", "basic", ("respiratory", "exam-1"),
             front="What is the role of pulmonary surfactant?",
             back="It lowers alveolar surface tension, preventing collapse and improving compliance."),
    NoteSpec("resp-shift", "resp", "cloze", ("respiratory", "high-yield"),
             cloze="The oxygen–hemoglobin dissociation curve shifts {{c1::right}} with more CO₂, acidity, heat, and 2,3-BPG."),
    NoteSpec("resp-tidal", "resp", "basic", ("respiratory",),
             front="Tidal volume", back="Air moved in or out during a normal quiet breath (about 500 mL).", reverse=True),
    NoteSpec("resp-airway", "resp", "question", ("respiratory", "exam-1"),
             question=_ordering(
                 "resp-airway", "Order the path of inspired air.",
                 ["Trachea", "Main bronchus", "Bronchioles", "Alveolar ducts", "Alveoli"])),
    NoteSpec("resp-bicarbonate", "resp", "question", ("respiratory",),
             question=_true_false("Most carbon dioxide is carried in blood as bicarbonate.", True,
                                  "About 70% travels as bicarbonate after carbonic anhydrase acts in red cells.")),
    NoteSpec("resp-dead-space", "resp", "basic", ("respiratory", "exam-1"),
             front="What is physiological dead space?",
             back="Anatomical plus alveolar dead space: inspired air that takes no part in gas exchange."),
    NoteSpec("resp-rhythm", "resp", "basic", ("respiratory",),
             front="Which brainstem area generates the basic breathing rhythm?",
             back="The pre-Bötzinger complex in the ventral respiratory group of the medulla."),
    # Metabolism
    NoteSpec("metab-pfk", "metab", "basic", ("biochemistry", "metabolism", "high-yield"),
             front="What is the rate-limiting enzyme of glycolysis?", back="Phosphofructokinase-1 (PFK-1)."),
    NoteSpec("metab-tca", "metab", "cloze", ("biochemistry", "metabolism", "exam-2"),
             cloze="Each turn of the TCA cycle yields {{c1::3 NADH}}, {{c2::1 FADH₂}}, and 1 GTP."),
    NoteSpec("metab-location", "metab", "question", ("biochemistry", "metabolism"),
             question=_grouping(
                 "metab-location", "Sort each pathway by where it runs in the cell.",
                 {"Cytosol": ["Glycolysis", "Fatty acid synthesis", "Pentose phosphate pathway"],
                  "Mitochondrial matrix": ["TCA cycle", "Beta-oxidation", "Pyruvate dehydrogenase reaction"]})),
    NoteSpec("metab-glucagon", "metab", "question", ("biochemistry", "metabolism", "exam-2"),
             question=_single_choice(
                 "metab-glucagon", "Which hormone stimulates hepatic glycogenolysis during fasting?",
                 ["Glucagon", "Insulin", "Aldosterone", "Calcitonin"], 0,
                 "Glucagon raises cAMP in hepatocytes and activates glycogen phosphorylase.")),
    NoteSpec("metab-thiamine", "metab", "basic", ("biochemistry", "metabolism"),
             front="Which vitamin deficiency impairs pyruvate dehydrogenase?",
             back="Thiamine (vitamin B1): thiamine pyrophosphate is a required cofactor."),
    NoteSpec("metab-atp", "metab", "basic", ("biochemistry", "metabolism"),
             front="What is the net ATP yield of anaerobic glycolysis per glucose?", back="Two ATP."),
    NoteSpec("metab-gluconeogenesis", "metab", "basic", ("biochemistry", "metabolism", "exam-2"),
             front="Gluconeogenesis",
             back="Synthesis of glucose from non-carbohydrate precursors such as lactate, glycerol, and alanine.", reverse=True),
    # Molecular biology
    NoteSpec("molbio-topoisomerase", "molbio", "basic", ("biochemistry", "molecular-biology"),
             front="Which enzyme relieves supercoiling ahead of the replication fork?",
             back="Topoisomerase (DNA gyrase in bacteria)."),
    NoteSpec("molbio-polymerase", "molbio", "cloze", ("biochemistry", "molecular-biology"),
             cloze="Eukaryotic mRNA is transcribed by {{c1::RNA polymerase II}}."),
    NoteSpec("molbio-expression", "molbio", "question", ("biochemistry", "molecular-biology"),
             question=_ordering(
                 "molbio-expression", "Order the steps of eukaryotic gene expression.",
                 ["Transcription in the nucleus", "Pre-mRNA processing", "mRNA export", "Translation at the ribosome", "Protein folding"])),
    NoteSpec("molbio-codon", "molbio", "basic", ("biochemistry", "molecular-biology"),
             front="What is a codon?",
             back="Three mRNA nucleotides that specify one amino acid or a stop signal."),
    NoteSpec("molbio-primer", "molbio", "question", ("biochemistry", "molecular-biology"),
             question=_true_false(
                 "DNA polymerase can start a new strand without a primer.", False,
                 "Primase lays down a short RNA primer first.",
                 "DNA polymerase needs a primer with a free 3′-OH group.")),
    # Pharmacology essentials
    NoteSpec("pharm-half-life", "pharm", "basic", ("pharmacology", "high-yield"),
             front="Half-life (t½)", back="Time for the plasma drug concentration to fall by 50%.", reverse=True),
    NoteSpec("pharm-steady-state", "pharm", "basic", ("pharmacology", "exam-2"),
             front="How many half-lives does it take to reach steady state?", back="About four to five half-lives."),
    NoteSpec("pharm-beta-blockers", "pharm", "question", ("pharmacology", "autonomic"),
             question=_select_all(
                 "pharm-beta-blockers", "Which drugs are β-adrenergic antagonists?",
                 ["Propranolol", "Metoprolol", "Atenolol", "Salbutamol", "Phenylephrine"], [0, 1, 2],
                 "Salbutamol is a β2 agonist; phenylephrine is an α1 agonist.")),
    NoteSpec("pharm-receptors", "pharm", "question", ("pharmacology", "autonomic", "high-yield"),
             question=_matching(
                 "pharm-receptors", "Match each receptor with its signaling mechanism.",
                 [("α1 adrenergic", "Gq → IP₃ and DAG"), ("β1 adrenergic", "Gs → more cAMP"),
                  ("α2 adrenergic", "Gi → less cAMP"), ("Nicotinic", "Ligand-gated ion channel")])),
    NoteSpec("pharm-bioavailability", "pharm", "basic", ("pharmacology",),
             front="What does bioavailability measure?",
             back="The fraction of a dose that reaches the systemic circulation unchanged."),
    NoteSpec("pharm-first-order", "pharm", "cloze", ("pharmacology", "exam-2"),
             cloze="First-order elimination removes a constant {{c1::fraction}} of drug per unit time."),
    # Bacterial structure (recall cards only, all reviewed: nothing due today)
    NoteSpec("micro-gram", "micro", "basic", ("microbiology", "high-yield"),
             front="What does the Gram stain distinguish?",
             back="A thick peptidoglycan wall that keeps crystal violet (Gram-positive, purple) from a thin wall"
                  " under an outer membrane (Gram-negative, pink)."),
    NoteSpec("micro-endospore", "micro", "basic", ("microbiology",),
             front="Which bacterial structure survives heat, drying, and disinfectants?",
             back="The endospore, formed by Bacillus and Clostridium species."),
    NoteSpec("micro-lps", "micro", "cloze", ("microbiology", "high-yield"),
             cloze="The outer membrane of Gram-negative bacteria contains {{c1::lipopolysaccharide}},"
                   " whose lipid A acts as an endotoxin."),
    NoteSpec("micro-conjugation", "micro", "basic", ("microbiology",),
             front="Conjugation", back="Transfer of DNA, often a plasmid, from one bacterium to another through a pilus."),
    # Medical terminology (no subject; new cards only)
    NoteSpec("terms-brady", "terms", "basic", ("terminology",),
             front="What does the prefix brady- mean?", back="Slow, as in bradycardia: a heart rate below 60 beats per minute."),
    NoteSpec("terms-itis", "terms", "basic", ("terminology",),
             front="What does the suffix -itis mean?", back="Inflammation, as in nephritis: inflammation of the kidney."),
    NoteSpec("terms-hyper", "terms", "cloze", ("terminology",),
             cloze="{{c1::Hyper-}} means above normal; {{c2::hypo-}} means below normal."),
)

# Review history: (days before the capture day, minute of day, deck, notes).
# Only recall cards (basic/cloze) are practiced; interactive questions require
# an answer check first and are left for live Practice.
HISTORY: tuple[tuple[int, int, str, tuple[str, ...]], ...] = (
    (41, 1110, "cardio", ("cardio-stroke-volume", "cardio-frank-starling", EDITOR_NOTE, "cardio-output", "cardio-s2", "cardio-resistance")),
    (40, 1140, "resp", ("resp-surfactant", "resp-shift", "resp-tidal", "resp-dead-space", "resp-rhythm")),
    (38, 1095, "metab", ("metab-pfk", "metab-tca", "metab-thiamine", "metab-atp", "metab-gluconeogenesis")),
    (37, 1170, "cardio", ("cardio-frank-starling", "cardio-output", "cardio-s2", EDITOR_NOTE)),
    (35, 1125, "pharm", ("pharm-half-life", "pharm-steady-state", "pharm-bioavailability", "pharm-first-order")),
    (34, 1200, "renal", PRACTICE_ORDER),
    (31, 1080, "molbio", ("molbio-topoisomerase", "molbio-polymerase", "molbio-codon")),
    (30, 1150, "resp", ("resp-surfactant", "resp-shift", "resp-dead-space")),
    (28, 1110, "metab", ("metab-pfk", "metab-tca", "metab-atp")),
    (27, 1185, "cardio", ("cardio-stroke-volume", "cardio-resistance", "cardio-s2")),
    (24, 1100, "pharm", ("pharm-half-life", "pharm-first-order", "pharm-steady-state")),
    (23, 1160, "renal", ("renal-glucose", "renal-adh", "renal-gfr")),
    (21, 1130, "resp", ("resp-tidal", "resp-rhythm", "resp-surfactant", "resp-dead-space")),
    (20, 1090, "metab", ("metab-gluconeogenesis", "metab-thiamine", "metab-tca")),
    (17, 1175, "cardio", ("cardio-frank-starling", EDITOR_NOTE, "cardio-output")),
    (16, 1120, "molbio", ("molbio-codon", "molbio-topoisomerase", "molbio-polymerase")),
    (14, 1145, "renal", ("renal-macula", "renal-aldosterone", "renal-glucose")),
    (13, 1105, "pharm", ("pharm-bioavailability", "pharm-half-life")),
    (10, 1165, "resp", ("resp-shift", "resp-surfactant", "resp-tidal")),
    (9, 1115, "metab", ("metab-pfk", "metab-atp", "metab-tca", "metab-gluconeogenesis")),
    (7, 1190, "cardio", ("cardio-stroke-volume", "cardio-s2", "cardio-resistance", "cardio-frank-starling")),
    (6, 1135, "renal", ("renal-adh", "renal-gfr", "renal-macula")),
    (5, 1095, "pharm", ("pharm-steady-state", "pharm-first-order", "pharm-half-life")),
    (4, 1155, "resp", ("resp-dead-space", "resp-rhythm", "resp-shift")),
    (3, 1125, "molbio", ("molbio-polymerase", "molbio-codon")),
    (2, 1180, "cardio", (EDITOR_NOTE, "cardio-output", "cardio-frank-starling")),
    (1, 1110, "metab", ("metab-thiamine", "metab-tca", "metab-pfk")),
    # Bacterial structure, appended so the sessions above keep their ratings.
    (33, 1120, "micro", ("micro-gram", "micro-endospore", "micro-lps", "micro-conjugation")),
    (19, 1135, "micro", ("micro-gram", "micro-endospore", "micro-lps", "micro-conjugation")),
    (8, 1105, "micro", ("micro-gram", "micro-endospore", "micro-lps", "micro-conjugation")),
)

OUTCOMES = ("wrong", "not_confident", "somewhat_confident", "very_confident")
# A fixed, mostly-correct rating cycle (~80% remembered).
OUTCOME_CYCLE = (3, 2, 3, 1, 2, 3, 0, 3, 2, 2, 3, 1, 3, 2, 0, 3, 2, 3)

TUTOR_CONVERSATION_TITLE = "Why does ADH concentrate urine?"
TUTOR_TURNS: tuple[tuple[str, str], ...] = (
    (
        "Why does ADH make urine more concentrated?",
        (
            "ADH (vasopressin) makes the **collecting duct permeable to water**.\n\n"
            "1. ADH binds **V2 receptors** on principal cells.\n"
            "2. cAMP signaling moves **aquaporin-2** channels into the apical membrane.\n"
            "3. Water leaves the duct by osmosis into the hypertonic medulla that the loop of Henle builds.\n\n"
            "The result is a small volume of concentrated urine — up to about 1,200 mOsm/kg."
        ),
    ),
    (
        "What happens when ADH is missing?",
        (
            "Without ADH the collecting duct stays **water-impermeable**, so dilute urine passes straight through.\n\n"
            "- **Central diabetes insipidus:** the posterior pituitary releases too little ADH.\n"
            "- **Nephrogenic diabetes insipidus:** the kidney does not respond to ADH.\n\n"
            "Both cause polyuria and thirst; giving desmopressin separates the two."
        ),
    ),
)

# A reusable card template, so Settings > Card templates shows a template list.
TEMPLATE_NAME = "Drug profile"
TEMPLATE_DESCRIPTION = "Enter a drug once and get cards for its class, mechanism, and key adverse effect."
TEMPLATE_FIELDS: tuple[tuple[str, str], ...] = (
    ("Drug", "Generic name"),
    ("Class", "Pharmacological class"),
    ("Mechanism", "How the drug works"),
    ("Adverse effect", "The effect most worth remembering"),
)
TEMPLATE_RULES: tuple[tuple[str, tuple[str, str, str], str], ...] = (
    ("Class", ("Which drug class does ", "Drug", " belong to?"), "Class"),
    ("Mechanism", ("How does ", "Drug", " work?"), "Mechanism"),
    ("Adverse effect", ("Name a key adverse effect of ", "Drug", "."), "Adverse effect"),
)


def template_field_id(name: str) -> str:
    return stable_hex("template-field", name)


def _placeholder(name: str) -> dict[str, Any]:
    return {"type": "templateFieldInline", "attrs": {"fieldId": template_field_id(name), "label": name}}


def card_template_input() -> dict[str, Any]:
    return {
        "name": TEMPLATE_NAME,
        "description": TEMPLATE_DESCRIPTION,
        "fields": [
            {"id": template_field_id(name), "name": name, "description": description}
            for name, description in TEMPLATE_FIELDS
        ],
        "rules": [
            {
                "id": stable_hex("template-rule", rule),
                "name": rule,
                "frontDoc": doc(paragraph(before, _placeholder(field), after)),
                "backDoc": doc(paragraph(_placeholder(answer))),
            }
            for rule, (before, field, after), answer in TEMPLATE_RULES
        ],
    }


# --------------------------------------------------------------------------
# Documents, learning plans, and study guides
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class TopicSpec:
    title: str
    summary: str
    page: int
    objectives: tuple[str, ...]
    concepts: tuple[str, ...]
    content: dict[str, Any]


@dataclass(frozen=True)
class DocumentSpec:
    key: str
    title: str
    filename: str
    days_ago: int
    pages: tuple[tuple[str, tuple[str, ...]], ...]
    subject: str | None = None
    summary: str = ""
    objectives: tuple[str, ...] = ()
    concepts: tuple[str, ...] = ()
    topics: tuple[TopicSpec, ...] = ()


RENAL_TOPICS = (
    TopicSpec(
        "Glomerular filtration",
        "How Starling forces across the glomerular capillary set the filtration rate.",
        1,
        ("Explain the Starling forces that drive filtration.", "Estimate GFR from clearance."),
        ("GFR", "Filtration fraction", "Starling forces"),
        {
            "overview": "Filtration moves about 20% of renal plasma flow into Bowman's space. The rate depends on capillary pressure, oncotic pressure, and the filtration barrier.",
            "keyPoints": [
                "Normal GFR is about 125 mL/min.",
                "Glomerular capillary hydrostatic pressure is the main driving force.",
                "Plasma oncotic pressure rises along the capillary and opposes filtration.",
            ],
            "importantTerms": [
                {"term": "Filtration fraction", "explanation": "GFR divided by renal plasma flow, normally about 0.2."},
                {"term": "Inulin clearance", "explanation": "The reference measurement of GFR: freely filtered, neither reabsorbed nor secreted."},
            ],
            "tables": [
                {
                    "title": "Arteriolar changes and filtration",
                    "columns": ["Change", "GFR", "Renal plasma flow"],
                    "rows": [
                        ["Afferent constriction", "Decreases", "Decreases"],
                        ["Efferent constriction", "Increases", "Decreases"],
                        ["Afferent dilation", "Increases", "Increases"],
                    ],
                    "caption": "Efferent constriction raises filtration fraction.",
                }
            ],
            "callouts": [
                {"kind": "key_idea", "title": "Autoregulation", "body": "Myogenic and tubuloglomerular feedback keep GFR stable between mean pressures of about 80 and 180 mmHg."}
            ],
            "retrievalChecks": [
                {"prompt": "What happens to filtration fraction when the efferent arteriole constricts?", "answer": "It rises, because GFR increases while plasma flow falls."}
            ],
        },
    ),
    TopicSpec(
        "Proximal tubule transport",
        "Bulk, iso-osmotic reabsorption of sodium, glucose, amino acids, and bicarbonate.",
        2,
        ("Describe sodium-coupled transport in the proximal tubule.",),
        ("SGLT2", "Transport maximum", "Iso-osmotic reabsorption"),
        {
            "overview": "The proximal tubule reabsorbs about two thirds of filtered sodium and water and nearly all filtered glucose and amino acids.",
            "keyPoints": [
                "Na⁺/K⁺-ATPase on the basolateral membrane powers secondary active transport.",
                "Glucose uses SGLT2 early in the tubule and SGLT1 later.",
                "Glucosuria appears once the transport maximum is exceeded.",
            ],
            "processSteps": [
                "Na⁺/K⁺-ATPase keeps intracellular sodium low.",
                "Sodium enters from the lumen together with glucose through SGLT2.",
                "Glucose leaves the cell through GLUT2 on the basolateral side.",
                "Water follows the solute through aquaporin-1.",
            ],
            "commonMisconceptions": [
                "Proximal reabsorption is not driven by ADH; water follows solute here regardless of hydration.",
            ],
            "workedExamples": [
                {
                    "title": "Glucose threshold",
                    "prompt": "Plasma glucose is 250 mg/dL in a patient with a normal GFR. Is glucose in the urine expected?",
                    "steps": ["The renal threshold is about 200 mg/dL.", "Some nephrons saturate their transporters before the average transport maximum."],
                    "answer": "Yes — glucosuria is expected above roughly 200 mg/dL.",
                    "takeaway": "SGLT2 inhibitors lower the threshold on purpose.",
                }
            ],
        },
    ),
    TopicSpec(
        "Loop of Henle and the medullary gradient",
        "Countercurrent multiplication builds the hypertonic medulla used to concentrate urine.",
        3,
        ("Explain countercurrent multiplication.",),
        ("Countercurrent multiplier", "NKCC2", "Vasa recta"),
        {
            "overview": "The thick ascending limb pumps NaCl without water, diluting tubular fluid while building a concentration gradient in the medulla.",
            "keyPoints": [
                "The descending limb is permeable to water but not to solute.",
                "The thick ascending limb is water-impermeable and reabsorbs NaCl through NKCC2.",
                "Loop diuretics block NKCC2 and wash out the gradient.",
            ],
            "comparisons": [
                {"subjectA": "Descending limb", "subjectB": "Thick ascending limb", "relationship": "Water leaves the descending limb; solute leaves the ascending limb."}
            ],
            "explanations": [
                {"heading": "Why the vasa recta matter", "body": "Their hairpin shape lets blood exchange solute without washing out the gradient."}
            ],
        },
    ),
    TopicSpec(
        "Hormonal control of water and sodium",
        "ADH, aldosterone, and natriuretic peptides adjust final urine volume and composition.",
        4,
        ("Compare the actions of ADH and aldosterone.",),
        ("ADH", "Aldosterone", "ANP"),
        {
            "overview": "Distal segments fine-tune what the proximal tubule and loop leave behind, under hormonal control.",
            "keyPoints": [
                "ADH inserts aquaporin-2 in the collecting duct through V2 receptors.",
                "Aldosterone increases ENaC and Na⁺/K⁺-ATPase activity in principal cells.",
                "ANP increases GFR and inhibits sodium reabsorption.",
            ],
            "importantTerms": [
                {"term": "Principal cells", "explanation": "Collecting duct cells that reabsorb sodium and secrete potassium."},
                {"term": "Intercalated cells", "explanation": "Collecting duct cells that secrete acid or bicarbonate."},
            ],
            "callouts": [
                {"kind": "warning", "title": "Hyperkalemia risk", "body": "Drugs that block aldosterone or ENaC reduce potassium secretion."}
            ],
            "retrievalChecks": [
                {"prompt": "Which receptor mediates the renal water effect of ADH?", "answer": "The V2 receptor."}
            ],
        },
    ),
)

CARDIAC_TOPICS = (
    TopicSpec(
        "Phases of the cardiac cycle",
        "Pressure and volume changes from filling to ejection.",
        1,
        ("Name the phases of the cardiac cycle in order.",),
        ("Isovolumetric contraction", "Ejection", "Isovolumetric relaxation"),
        {
            "overview": "Each beat moves through filling, isovolumetric contraction, ejection, and isovolumetric relaxation.",
            "processSteps": [
                "Ventricular filling while the AV valves are open.",
                "Isovolumetric contraction after the mitral valve closes (S1).",
                "Ejection once ventricular pressure exceeds aortic pressure.",
                "Isovolumetric relaxation after the aortic valve closes (S2).",
            ],
            "keyPoints": ["Both isovolumetric phases happen with all valves closed."],
        },
    ),
    TopicSpec(
        "Pressure–volume loops",
        "Reading preload, afterload, and contractility from the loop.",
        2,
        ("Interpret changes in a pressure–volume loop.",),
        ("End-diastolic volume", "End-systolic volume", "Stroke work"),
        {
            "overview": "The loop's width is stroke volume and its area is stroke work.",
            "tables": [
                {
                    "title": "Loop changes",
                    "columns": ["Change", "Effect on loop"],
                    "rows": [
                        ["More preload", "Wider loop, end-diastolic point moves right"],
                        ["More afterload", "Taller, narrower loop"],
                        ["More contractility", "End-systolic point moves left"],
                    ],
                    "caption": "",
                }
            ],
        },
    ),
    TopicSpec(
        "Heart sounds and murmurs",
        "What S1–S4 represent and when murmurs occur.",
        3,
        ("Relate heart sounds to valve events.",),
        ("S1", "S2", "Systolic murmur"),
        {
            "overview": "Heart sounds come from valve closure and from blood decelerating in the ventricles.",
            "keyPoints": [
                "S1: mitral and tricuspid closure.",
                "S2: aortic and pulmonary closure.",
                "S3 can be normal in young people; S4 suggests a stiff ventricle.",
            ],
            "callouts": [{"kind": "tip", "title": "Timing first", "body": "Decide whether a murmur is systolic or diastolic before naming the valve."}],
        },
    ),
)

DOCUMENTS: tuple[DocumentSpec, ...] = (
    DocumentSpec(
        "renal-lecture",
        "Renal physiology — lecture 4",
        "renal-physiology-lecture-4.pdf",
        12,
        (
            ("Glomerular filtration", (
                "The kidneys filter about 180 L of plasma each day. Glomerular filtration rate (GFR)",
                "depends on Starling forces across the glomerular capillary wall: capillary hydrostatic",
                "pressure favors filtration, while Bowman's space pressure and plasma oncotic pressure",
                "oppose it. Constricting the efferent arteriole raises GFR and the filtration fraction.",
            )),
            ("Proximal tubule transport", (
                "The proximal convoluted tubule reabsorbs about two thirds of filtered sodium and water.",
                "Glucose enters with sodium through SGLT2 and leaves through GLUT2. When plasma glucose",
                "exceeds the transport maximum, glucose appears in the urine.",
            )),
            ("The loop of Henle", (
                "The thick ascending limb reabsorbs NaCl through NKCC2 but is impermeable to water.",
                "Countercurrent multiplication builds an osmotic gradient of up to 1200 mOsm/kg in",
                "the inner medulla. Loop diuretics block NKCC2.",
            )),
            ("Hormonal control", (
                "ADH acts on V2 receptors to insert aquaporin-2 channels in collecting duct cells.",
                "Aldosterone increases sodium reabsorption and potassium secretion in principal cells.",
                "Atrial natriuretic peptide promotes sodium excretion.",
            )),
        ),
        summary="Renal filtration, tubular transport, the medullary gradient, and hormonal control of water and sodium.",
        objectives=("Explain how GFR is regulated.", "Describe transport in each nephron segment.", "Compare ADH and aldosterone."),
        concepts=("GFR", "Countercurrent multiplication", "ADH", "Aldosterone"),
        topics=RENAL_TOPICS,
    ),
    DocumentSpec(
        "cardiac-review",
        "Cardiac cycle review",
        "cardiac-cycle-review.pdf",
        19,
        (
            ("The cardiac cycle", (
                "Each heartbeat alternates between systole and diastole. Valves open and close",
                "passively as pressure gradients reverse between the atria, ventricles, and arteries.",
            )),
            ("Pressure-volume loops", (
                "A left ventricular pressure-volume loop shows end-diastolic volume, end-systolic",
                "volume, and stroke work. Preload, afterload, and contractility each change its shape.",
            )),
            ("Heart sounds", (
                "S1 marks closure of the mitral and tricuspid valves; S2 marks closure of the aortic",
                "and pulmonary valves. S3 and S4 are filling sounds.",
            )),
        ),
        subject="Physiology",
        summary="The phases of the cardiac cycle, pressure-volume loops, and heart sounds.",
        objectives=("Sequence the phases of the cardiac cycle.", "Interpret pressure-volume loops."),
        concepts=("Cardiac cycle", "Stroke work", "Heart sounds"),
        topics=CARDIAC_TOPICS,
    ),
    DocumentSpec(
        "pk-primer",
        "Pharmacokinetics primer",
        "pharmacokinetics-primer.pdf",
        26,
        (
            ("Absorption and distribution", (
                "Bioavailability is the fraction of a dose that reaches the systemic circulation.",
                "Volume of distribution relates the amount of drug in the body to its plasma concentration.",
            )),
            ("Elimination", (
                "Most drugs follow first-order kinetics: a constant fraction is eliminated per unit time.",
                "Steady state is reached after about four to five half-lives.",
            )),
        ),
        subject="Pharmacology",
    ),
)


# The document whose generation info is captured. Documents offers that action
# only on a document in no subject.
INFO_DOCUMENT = DOCUMENTS[0]


def home_tile_count() -> int:
    """Home's subject tiles: one per subject, and No subject when anything is unfiled."""
    unfiled = any(deck.subject is None for deck in DECKS) or any(spec.subject is None for spec in DOCUMENTS)
    return len(SUBJECTS) + (1 if unfiled else 0)


# --------------------------------------------------------------------------
# A minimal deterministic PDF writer (standard fonts, WinAnsi text)
# --------------------------------------------------------------------------


def _pdf_text(value: str) -> str:
    encoded = value.encode("cp1252", errors="replace").decode("latin-1")
    return encoded.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def build_pdf(title: str, pages: tuple[tuple[str, tuple[str, ...]], ...]) -> bytes:
    """Return a small multi-page PDF that PDFium can render and extract."""
    objects: list[bytes] = []
    page_count = len(pages)
    first_page_object = 5
    kids = " ".join(f"{first_page_object + index * 2} 0 R" for index in range(page_count))
    objects.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {page_count} >>".encode("ascii"))
    objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>")
    for index, (heading, lines) in enumerate(pages):
        content_object = first_page_object + index * 2 + 1
        objects.append(
            (
                f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                f"/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_object} 0 R >>"
            ).encode("ascii")
        )
        commands = [
            "BT /F1 9 Tf 72 750 Td (" + _pdf_text(title) + f" - page {index + 1} of {page_count}) Tj ET",
            "BT /F2 18 Tf 72 700 Td (" + _pdf_text(heading) + ") Tj ET",
            "BT /F1 11 Tf 15 TL 72 670 Td",
        ]
        commands.extend("(" + _pdf_text(line) + ") Tj T*" for line in lines)
        commands.append("ET")
        stream = "\n".join(commands).encode("latin-1")
        objects.append(b"<< /Length " + str(len(stream)).encode("ascii") + b" >>\nstream\n" + stream + b"\nendstream")
    output = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(output))
        output += f"{number} 0 obj\n".encode("ascii") + body + b"\nendobj\n"
    xref = len(output)
    output += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode("ascii")
    for offset in offsets:
        output += f"{offset:010d} 00000 n \n".encode("ascii")
    output += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode("ascii")
    return bytes(output)


# --------------------------------------------------------------------------
# Phase 1: seed through the running app's native commands
# --------------------------------------------------------------------------

# Runs [{"command", "args", "chainRevision"}, ...] in the page; returns each result.
Batch = Callable[[list[dict[str, Any]]], list[Any]]
# Runs practice plans in the page; returns [{"sessionId", "order"}, ...].
PracticeBatch = Callable[[list[dict[str, Any]]], list[dict[str, Any]]]


@dataclass
class SeedState:
    subject_ids: dict[str, str] = field(default_factory=dict)
    deck_ids: dict[str, str] = field(default_factory=dict)
    note_ids: dict[str, str] = field(default_factory=dict)
    card_ids: dict[str, list[str]] = field(default_factory=dict)
    sessions: list[dict[str, Any]] = field(default_factory=list)
    tutor_conversation_id: str = ""
    template_id: str = ""

    def to_json(self) -> dict[str, Any]:
        return {
            "subjectIds": self.subject_ids,
            "deckIds": self.deck_ids,
            "noteIds": self.note_ids,
            "cardIds": self.card_ids,
            "sessions": self.sessions,
            "tutorConversationId": self.tutor_conversation_id,
            "templateId": self.template_id,
        }

    @classmethod
    def from_json(cls, value: dict[str, Any]) -> SeedState:
        return cls(
            subject_ids=dict(value["subjectIds"]),
            deck_ids=dict(value["deckIds"]),
            note_ids=dict(value["noteIds"]),
            card_ids={key: list(ids) for key, ids in value["cardIds"].items()},
            sessions=list(value["sessions"]),
            tutor_conversation_id=str(value.get("tutorConversationId") or ""),
            template_id=str(value.get("templateId") or ""),
        )


def deck_name(key: str) -> str:
    return next(deck.name for deck in DECKS if deck.key == key)


def note_spec(key: str) -> NoteSpec:
    return next(note for note in NOTES if note.key == key)


def question_text(note: NoteSpec) -> str:
    """The visible question text of a recall note, as it appears in lists."""
    if note.front:
        return note.front
    if note.front_doc is not None:
        return "".join(
            str(run.get("text", ""))
            for block in note.front_doc["content"]
            for run in block.get("content", [])
        )
    return ""


def _step(command: str, args: dict[str, Any], chain_revision: bool = False) -> dict[str, Any]:
    return {"command": command, "args": args, "chainRevision": chain_revision}


def _note_step(note: NoteSpec, deck_id: str) -> dict[str, Any]:
    tags = list(note.tags)
    if note.kind == "question":
        assert note.question is not None
        return _step(
            "save_question_note",
            {
                "input": {
                    "operationId": stable_hex("note", note.key),
                    "noteId": None,
                    "revision": None,
                    "deckId": deck_id,
                    "definitionJson": json.dumps({"schemaVersion": 1, "question": note.question}),
                    "tags": tags,
                    "allowDuplicate": False,
                }
            },
        )
    if note.kind == "cloze":
        payload: dict[str, Any] = {"cardType": "cloze", "clozeSource": note.cloze, "clozeMode": "one_at_a_time"}
    else:
        payload = {
            "cardType": "basic",
            "reverseEnabled": note.reverse,
            "frontDoc": note.front_doc or doc(note.front),
            "backDoc": note.back_doc or doc(note.back),
        }
    return _step("create_card_note", {"input": {"deckId": deck_id, "tags": tags, "allowDuplicate": False, **payload}})


def practice_plans(state: SeedState) -> list[dict[str, Any]]:
    """One plan per historical session: cards, fixed outcomes, and response times."""
    plans: list[dict[str, Any]] = []
    cycle_index = 0
    for session_index, (_days_ago, _minute, _deck, note_keys) in enumerate(HISTORY):
        card_ids: list[str] = []
        outcomes: dict[str, str] = {}
        response_times: dict[str, int] = {}
        for note_key in note_keys:
            for card_id in state.card_ids[note_key]:
                card_ids.append(card_id)
                outcomes[card_id] = OUTCOMES[OUTCOME_CYCLE[cycle_index % len(OUTCOME_CYCLE)]]
                response_times[card_id] = 3_200 + (cycle_index * 2_731) % 9_000
                cycle_index += 1
        plans.append(
            {
                "cardIds": card_ids,
                "operationId": stable_hex("practice", str(session_index)),
                "submissionIds": [stable_hex("submission", str(session_index), str(i)) for i in range(len(card_ids))],
                "outcomes": outcomes,
                "responseTimes": response_times,
            }
        )
    return plans


def seed_through_ipc(batch: Batch, practice: PracticeBatch, chunk: int = 9) -> SeedState:
    """Create the library content with the app's own commands.

    ``batch`` runs a list of native commands inside the page in one WebDriver
    round trip; a step with ``chainRevision`` takes the previous result's
    library-organization revision. ``practice`` runs whole practice sessions.
    Few round trips keep seeding fast and avoid bursts of WebDriver requests.
    """
    state = SeedState()
    # The deck folder commands create and fill subjects; a folder id is the
    # subject id.
    results = batch(
        [_step("get_deck_library_organization", {})]
        + [_step("create_deck_folder", {"input": {"name": subject}}, chain_revision=True) for subject in SUBJECTS]
    )
    for subject in SUBJECTS:
        state.subject_ids[subject] = next(item["id"] for item in results[-1]["folders"] if item["name"] == subject)

    results = batch(
        [_step("create_deck", {"input": {"name": deck.name, "description": deck.description}}) for deck in DECKS]
    )
    for deck, summary in zip(DECKS, results):
        state.deck_ids[deck.key] = summary["id"]

    moves: list[dict[str, Any]] = [_step("get_deck_library_organization", {})]
    subject_positions: dict[str, int] = {}
    for deck in DECKS:
        if deck.subject is None:
            continue
        index = subject_positions.get(deck.subject, 0)
        moves.append(
            _step(
                "move_deck_in_library",
                {"input": {"deckId": state.deck_ids[deck.key], "targetFolderId": state.subject_ids[deck.subject],
                           "targetIndex": index}},
                chain_revision=True,
            )
        )
        subject_positions[deck.subject] = index + 1
    batch(moves)

    results = batch([_note_step(note, state.deck_ids[note.deck]) for note in NOTES])
    for note, receipt in zip(NOTES, results):
        state.note_ids[note.key] = receipt["noteId"]
        if note.kind == "question":
            state.card_ids[note.key] = list(receipt["cardIds"])

    # Card identities per recall note, ordered by presentation label so a
    # reverse or multi-deletion note always lists its cards the same way.
    rows: list[dict[str, Any]] = []
    offset = 0
    while True:
        page = batch([_step("search_cards", {"input": {
            "deckIds": [], "query": "", "tagIds": [], "availabilityFilter": "all", "statusFilter": "all",
            "reviewedFilter": "all", "sort": "updated_desc", "limit": 100, "offset": offset,
        }})])[0]
        rows.extend(page["cards"])
        offset += len(page["cards"])
        if not page["cards"] or offset >= int(page["totalCount"]):
            break
    by_note_id = {note_id: key for key, note_id in state.note_ids.items()}
    grouped: dict[str, list[tuple[str, str]]] = {}
    for row in rows:
        key = by_note_id.get(str(row["noteId"]))
        if key is not None and key not in state.card_ids:
            grouped.setdefault(key, []).append((str(row["cardLabel"]), str(row["cardId"])))
    for key, cards in grouped.items():
        state.card_ids[key] = [card_id for _label, card_id in sorted(cards)]
    missing = [note.key for note in NOTES if not state.card_ids.get(note.key)]
    if missing:
        raise RuntimeError(f"Seeded notes produced no cards: {missing}")

    plans = practice_plans(state)
    outcomes: list[dict[str, Any]] = []
    for first in range(0, len(plans), chunk):
        outcomes.extend(practice(plans[first:first + chunk]))
    for (days_ago, minute, deck, _note_keys), plan, result in zip(HISTORY, plans, outcomes):
        state.sessions.append(
            {
                "sessionId": result["sessionId"],
                "daysAgo": days_ago,
                "minuteOfDay": minute,
                "deck": deck,
                "order": result["order"],
                "responseTimes": plan["responseTimes"],
            }
        )

    template, conversation = batch([
        _step("create_card_template", {"input": card_template_input()}),
        _step("create_tutor_conversation", {"input": {
            "operationId": stable_hex("tutor", "conversation"),
            "scope": {"kind": "library", "id": None},
            "title": TUTOR_CONVERSATION_TITLE,
        }}),
    ])
    state.template_id = template["id"]
    state.tutor_conversation_id = conversation["id"]
    return state


# --------------------------------------------------------------------------
# Phase 2: deterministic finishing on the closed library
# --------------------------------------------------------------------------


def local_date(utc_ms: int) -> str:
    import datetime as _datetime

    return _datetime.datetime.fromtimestamp(utc_ms / 1000, tz=_datetime.timezone.utc).strftime("%Y-%m-%d")


def finalize_library(app_data_dir: Path, state: SeedState, anchor_day_ms: int, settings: dict[str, Any]) -> list[str]:
    """Edit the closed library so every screen is populated and repeatable.

    ``anchor_day_ms`` is UTC midnight of the capture day; every seeded date is
    relative to it so two captures on the same day see identical content.
    Returns human-readable notes about what was seeded.
    """
    database = app_data_dir / "library.sqlite3"
    connection = sqlite3.connect(database)
    connection.execute("PRAGMA foreign_keys = ON")
    notes: list[str] = []
    try:
        stabilize_tag_ids(connection)
        with connection:
            _rewrite_history(connection, state, anchor_day_ms)
            _normalize_content_times(connection, state, anchor_day_ms)
            _pin_practice_order(connection, state, anchor_day_ms)
            _pin_finished_deck(connection, state, anchor_day_ms)
            _write_settings(connection, settings, anchor_day_ms)
        with connection:
            notes.extend(_insert_documents(connection, app_data_dir, state, anchor_day_ms))
        with connection:
            _insert_tutor_turns(connection, state, anchor_day_ms)
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()
    return notes


TAG_ID_COLUMNS = (
    ("tags", "id"),
    ("note_tags", "tag_id"),
    ("practice_session_card_tag_snapshots", "tag_id"),
    ("review_event_tag_attributions", "tag_id"),
)


def fixed_tag_ids(normalized_names: list[str]) -> list[str]:
    """Fixed 32-hex tag ids whose sort order follows the names' sort order."""
    ordered = sorted(normalized_names)
    rank = {name: index for index, name in enumerate(ordered)}
    return [f"{rank[name]:04x}{stable_hex('tag', name)[4:]}" for name in normalized_names]


def stabilize_tag_ids(connection: sqlite3.Connection) -> None:
    """Replace the random tag ids the app generated with fixed ones in name order.

    A deck's card list shows each note's tags in tag-id order (the search
    query's ORDER BY sits outside its group_concat), so random ids would
    reorder the tag chips between two otherwise identical libraries.
    """
    rows = connection.execute("SELECT id, normalized_name FROM tags").fetchall()
    mapping = dict(zip([row[0] for row in rows], fixed_tag_ids([row[1] for row in rows]), strict=True))
    if set(mapping) & set(mapping.values()):
        raise RuntimeError("A generated tag id collides with a fixed one; seed again.")
    connection.execute("BEGIN")
    try:
        # Parent and child ids change together, so foreign keys are checked at commit.
        connection.execute("PRAGMA defer_foreign_keys = ON")
        for old, new in mapping.items():
            for table, column in TAG_ID_COLUMNS:
                connection.execute(f"UPDATE {table} SET {column} = ?1 WHERE {column} = ?2", (new, old))
        for session_id, selected in connection.execute(
            "SELECT id, selected_tag_ids_json FROM practice_sessions WHERE selected_tag_ids_json <> '[]'"
        ).fetchall():
            ids = [mapping.get(value, value) for value in json.loads(selected)]
            connection.execute(
                "UPDATE practice_sessions SET selected_tag_ids_json = ?1 WHERE id = ?2",
                (json.dumps(ids, separators=(",", ":")), session_id),
            )
        connection.commit()
    except BaseException:
        connection.rollback()
        raise


def _rewrite_history(connection: sqlite3.Connection, state: SeedState, anchor_day_ms: int) -> None:
    latest: dict[str, tuple[int, int]] = {}
    for session in state.sessions:
        session_id = session["sessionId"]
        start = anchor_day_ms - session["daysAgo"] * DAY_MS + session["minuteOfDay"] * MINUTE_MS
        cursor = start + 4_000
        rated_times: dict[str, int] = {}
        for card_id in session["order"]:
            response = int(session["responseTimes"][card_id])
            viewed = cursor
            revealed = viewed + response
            rated = revealed + 1_800
            rated_times[card_id] = rated
            connection.execute(
                "UPDATE practice_session_cards SET viewed_at = ?1, revealed_at = ?2, rated_at = ?3,"
                " response_time_ms = ?4 WHERE session_id = ?5 AND card_id = ?6",
                (viewed, revealed, rated, response, session_id, card_id),
            )
            cursor = rated + 2_500
        completed = cursor
        deck_id = state.deck_ids[session["deck"]]
        connection.execute(
            "UPDATE practice_sessions SET started_at = ?1, completed_at = ?2, mode = 'scheduled',"
            " selected_deck_ids_json = ?3 WHERE id = ?4",
            (start, completed, json.dumps([deck_id]), session_id),
        )
        connection.execute(
            "UPDATE practice_session_card_snapshots SET captured_at = ?1 WHERE session_id = ?2",
            (start, session_id),
        )
        events = connection.execute(
            "SELECT id, card_id, reviewed_at, due_before, due_after FROM review_events WHERE session_id = ?1",
            (session_id,),
        ).fetchall()
        for event_id, card_id, reviewed_at, due_before, due_after in events:
            new_reviewed = rated_times[card_id]
            delta = new_reviewed - int(reviewed_at)
            new_due_after = int(due_after) + delta
            connection.execute(
                "UPDATE review_events SET reviewed_at = ?1, created_at = ?1, due_before = ?2, due_after = ?3"
                " WHERE id = ?4",
                (new_reviewed, int(due_before) + delta, new_due_after, event_id),
            )
            connection.execute(
                "UPDATE review_event_attributions SET reviewed_at = ?1, review_local_date = ?2,"
                " utc_offset_minutes = 0 WHERE review_event_id = ?3",
                (new_reviewed, local_date(new_reviewed), event_id),
            )
            previous = latest.get(card_id)
            if previous is None or previous[0] < new_reviewed:
                latest[card_id] = (new_reviewed, new_due_after)
    for card_id, (reviewed, due) in latest.items():
        connection.execute(
            "UPDATE card_schedules SET last_review_at = ?1, due_at = ?2, updated_at = ?1 WHERE card_id = ?3",
            (reviewed, due, card_id),
        )


def _normalize_content_times(connection: sqlite3.Connection, state: SeedState, anchor_day_ms: int) -> None:
    base = anchor_day_ms - 45 * DAY_MS + 9 * 60 * MINUTE_MS
    # Subjects are document_collections rows; Home colors them in creation order.
    for index, subject in enumerate(SUBJECTS):
        stamp = base + index * MINUTE_MS
        connection.execute(
            "UPDATE document_collections SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
            (stamp, state.subject_ids[subject]),
        )
    for index, deck in enumerate(DECKS):
        stamp = base + (10 + index) * MINUTE_MS
        connection.execute(
            "UPDATE decks SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
            (stamp, state.deck_ids[deck.key]),
        )
    for index, note in enumerate(NOTES):
        stamp = base + (30 + index) * MINUTE_MS
        note_id = state.note_ids[note.key]
        connection.execute("UPDATE notes SET created_at = ?1, updated_at = ?1 WHERE id = ?2", (stamp, note_id))
        connection.execute("UPDATE cards SET created_at = ?1, updated_at = ?1 WHERE note_id = ?2", (stamp, note_id))
        connection.execute(
            "UPDATE card_schedules SET due_at = ?1, last_review_at = ?1, updated_at = ?1"
            " WHERE state = 0 AND card_id IN (SELECT id FROM cards WHERE note_id = ?2)",
            (stamp, note_id),
        )
    tag_rows = connection.execute("SELECT id FROM tags ORDER BY normalized_name").fetchall()
    for index, (tag_id,) in enumerate(tag_rows):
        stamp = base + (5 + index) * 1_000
        connection.execute("UPDATE tags SET created_at = ?1, updated_at = ?1 WHERE id = ?2", (stamp, tag_id))
    connection.execute("UPDATE library_organization SET updated_at = ?1", (base + 20 * MINUTE_MS,))
    if state.template_id:
        stamp = base + 90 * MINUTE_MS
        connection.execute(
            "UPDATE card_templates SET created_at = ?1, updated_at = ?1 WHERE id = ?2", (stamp, state.template_id)
        )


def _pin_practice_order(connection: sqlite3.Connection, state: SeedState, anchor_day_ms: int) -> None:
    """Give the Practice deck distinct, overdue local due dates in a fixed order."""
    count = len(PRACTICE_ORDER)
    for index, note_key in enumerate(PRACTICE_ORDER):
        due = anchor_day_ms - (count - index) * DAY_MS + 8 * 60 * MINUTE_MS
        for card_id in state.card_ids[note_key]:
            connection.execute("UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2", (due, card_id))


def _pin_finished_deck(connection: sqlite3.Connection, state: SeedState, anchor_day_ms: int) -> None:
    """Move the finished deck's next reviews to the days after the capture day."""
    for index, note in enumerate(note for note in NOTES if note.deck == FINISHED_DECK):
        due = anchor_day_ms + (3 + index) * DAY_MS + 8 * 60 * MINUTE_MS
        for card_id in state.card_ids[note.key]:
            connection.execute("UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2", (due, card_id))


def write_settings(app_data_dir: Path, settings: dict[str, Any], anchor_day_ms: int) -> None:
    """Store capture settings in a library the app created and closed, such as an empty one."""
    connection = sqlite3.connect(app_data_dir / "library.sqlite3")
    try:
        with connection:
            _write_settings(connection, settings, anchor_day_ms)
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()


def _write_settings(connection: sqlite3.Connection, settings: dict[str, Any], anchor_day_ms: int) -> None:
    for key, value in settings.items():
        connection.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)"
            " ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            (key, json.dumps(value), anchor_day_ms - 45 * DAY_MS),
        )


def set_theme(app_data_dir: Path, theme: str) -> None:
    connection = sqlite3.connect(app_data_dir / "library.sqlite3")
    try:
        with connection:
            connection.execute(
                "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('theme', ?1, 0)"
                " ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                (json.dumps(theme),),
            )
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()


def _locator(page: int, label: str) -> str:
    return json.dumps(
        {"unitKind": "page", "unitNumbers": [page], "sectionLabel": label, "assetIds": [], "timeRangeMs": None},
        separators=(",", ":"),
    )


def _section_text(content: dict[str, Any]) -> str:
    lines: list[str] = [content.get("overview", "")]
    lines.extend(content.get("keyPoints", []))
    for item in content.get("explanations", []):
        lines.extend([item["heading"], item["body"]])
    for item in content.get("importantTerms", []):
        lines.append(f"{item['term']}: {item['explanation']}")
    lines.extend(content.get("processSteps", []))
    for item in content.get("comparisons", []):
        lines.append(f"{item['subjectA']} / {item['subjectB']}: {item['relationship']}")
    lines.extend(content.get("commonMisconceptions", []))
    for item in content.get("callouts", []):
        lines.extend([item["title"], item["body"]])
    for item in content.get("tables", []):
        lines.append(item["title"])
        lines.append(" | ".join(item["columns"]))
        lines.extend(" | ".join(row) for row in item["rows"])
    for item in content.get("workedExamples", []):
        lines.extend([item["title"], item["prompt"], *item["steps"], item["answer"]])
    for item in content.get("retrievalChecks", []):
        lines.extend([item["prompt"], item["answer"]])
    return "\n".join(line for line in lines if line)


def _full_section_content(content: dict[str, Any]) -> dict[str, Any]:
    keys = (
        "overview", "keyPoints", "explanations", "importantTerms", "processSteps", "comparisons",
        "commonMisconceptions", "callouts", "tables", "workedExamples", "retrievalChecks", "visualExplanations",
    )
    full: dict[str, Any] = {}
    for key in keys:
        full[key] = content.get(key, "" if key == "overview" else [])
    return full


def _insert_documents(
    connection: sqlite3.Connection, app_data_dir: Path, state: SeedState, anchor_day_ms: int
) -> list[str]:
    seeded: list[str] = []
    for spec in DOCUMENTS:
        document_id = stable_hex("document", spec.key)
        stamp = anchor_day_ms - spec.days_ago * DAY_MS + 15 * 60 * MINUTE_MS
        pdf = build_pdf(spec.title, spec.pages)
        relative = f"documents/{document_id}/source.pdf"
        target = app_data_dir / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(pdf)
        digest = hashlib.sha256(pdf).hexdigest()
        page_count = len(spec.pages)
        connection.execute(
            """INSERT INTO documents (
                   id, title, original_filename, mime_type, byte_size, sha256,
                   source_relative_path, lifecycle_state, page_count, extracted_page_count,
                   rendered_page_count, generated_note_count, generated_card_count, created_at, updated_at,
                   completed_at, archived_at, revision, extraction_backend, render_backend,
                   outline_state, source_format, source_unit_kind, source_unit_count,
                   extracted_source_unit_count, source_media_type, adapter_backend,
                   semantic_chunk_count, processing_started_at, processing_duration_ms, subject_id
               ) VALUES (?1, ?2, ?3, 'application/pdf', ?4, ?5, ?6, 'ready', ?7, ?7, 0, 0, 0, ?8, ?8,
                         ?8, NULL, 1, 'pdfium-text-v1', 'pdfium-render-v1', 'ready', 'pdf', 'page', ?7, ?7,
                         'application/pdf', 'native-pdf-adapter-v1', ?7, ?9, 41000, ?10)""",
            (
                document_id, spec.title, spec.filename, len(pdf), digest, relative, page_count, stamp, stamp - 41_000,
                state.subject_ids[spec.subject] if spec.subject else None,
            ),
        )
        connection.execute(
            """INSERT INTO document_source_files (
                   document_id, file_ordinal, original_filename, media_type, relative_path,
                   byte_size, sha256, created_at, updated_at
               ) VALUES (?1, 1, ?2, 'application/pdf', ?3, ?4, ?5, ?6, ?6)""",
            (document_id, spec.filename, relative, len(pdf), digest, stamp),
        )
        chunk_hashes: list[str] = []
        for number, (heading, lines) in enumerate(spec.pages, start=1):
            text = heading + "\n" + " ".join(lines)
            text_hash = sha256_text(text)
            chunk_hashes.append(text_hash)
            locator = _locator(number, heading)
            connection.execute(
                """INSERT INTO document_source_units (
                       document_id, unit_number, unit_kind, label, title, plain_text, text_sha256,
                       source_file_ordinal, locator_json, extraction_confidence, created_at, updated_at
                   ) VALUES (?1, ?2, 'page', ?3, ?4, ?5, ?6, 1, ?7, 1.0, ?8, ?8)""",
                (document_id, number, f"Page {number}", heading, text, text_hash, locator, stamp),
            )
            connection.execute(
                """INSERT INTO document_semantic_chunk_sources (
                       document_id, chunk_index, text, full_text, source_locator_json,
                       visual_asset_ids_json, element_types_json, heading_context, token_estimate,
                       is_oversized, content_sha256, canonical_schema_version, created_at, updated_at
                   ) VALUES (?1, ?2, ?3, ?3, ?4, '[]', '["heading","paragraph"]', ?5, ?6, 0, ?7, 1, ?8, ?8)""",
                (document_id, number - 1, text, locator, heading, max(1, len(text) // 4), text_hash, stamp),
            )
        if not spec.topics:
            seeded.append(f"document without a study guide: {spec.title}")
            continue
        plan_id = stable_hex("plan", spec.key)
        plan_stamp = stamp + 5 * MINUTE_MS
        connection.execute(
            """INSERT INTO document_learning_plans (
                   id, document_id, revision, compiler_version, outline_fingerprint, plan_fingerprint,
                   summary_text, learning_objectives_json, key_concepts_json, state, created_at, updated_at
               ) VALUES (?1, ?2, 1, 1, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?8)""",
            (
                plan_id, document_id, sha256_text(f"outline:{spec.key}"), sha256_text(f"plan:{spec.key}"),
                spec.summary, json.dumps(list(spec.objectives)), json.dumps(list(spec.concepts)), plan_stamp,
            ),
        )
        guide_id = stable_hex("guide", spec.key)
        overview = {"summary": spec.summary, "learningObjectives": list(spec.objectives), "keyConcepts": list(spec.concepts)}
        overview_text = "\n".join([spec.summary, *spec.objectives, *spec.concepts])
        guide_stamp = stamp + 9 * MINUTE_MS
        connection.execute(
            """INSERT INTO document_study_guides (
                   id, document_id, plan_id, job_id, revision, schema_version, prompt_version,
                   assembly_version, overview_json, overview_text, state, created_at, updated_at, published_at
               ) VALUES (?1, ?2, ?3, NULL, 1, 2, 8, 4, ?4, ?5, 'active', ?6, ?6, ?6)""",
            (guide_id, document_id, plan_id, json.dumps(overview, ensure_ascii=False), overview_text, guide_stamp),
        )
        for index, topic in enumerate(spec.topics):
            topic_id = stable_hex("topic", spec.key, str(index))
            locator = _locator(topic.page, topic.title)
            evidence_hash = chunk_hashes[topic.page - 1]
            connection.execute(
                "INSERT INTO document_learning_topics (id, document_id, anchor_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
                (topic_id, document_id, sha256_text(f"anchor:{spec.key}:{index}"), plan_stamp),
            )
            connection.execute(
                """INSERT INTO document_learning_plan_topics (
                       plan_id, topic_id, parent_topic_id, sort_index, title, summary_text,
                       learning_objectives_json, key_concepts_json, source_locator_json,
                       evidence_chunk_indices_json, evidence_hashes_json, evidence_hash, created_at
                   ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)""",
                (
                    plan_id, topic_id, index, topic.title, topic.summary, json.dumps(list(topic.objectives)),
                    json.dumps(list(topic.concepts)), locator, json.dumps([topic.page - 1]),
                    json.dumps([evidence_hash]), sha256_text(f"evidence:{evidence_hash}"), plan_stamp,
                ),
            )
            content = _full_section_content(topic.content)
            content_json = json.dumps(content, ensure_ascii=False)
            connection.execute(
                """INSERT INTO document_study_guide_sections (
                       guide_id, topic_id, sort_index, content_json, content_text, content_hash,
                       source_locator_json, created_at, updated_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)""",
                (guide_id, topic_id, index, content_json, _section_text(content), sha256_text(content_json), locator, guide_stamp),
            )
        seeded.append(f"document with a {len(spec.topics)}-section study guide: {spec.title}")
    return seeded


def _insert_tutor_turns(connection: sqlite3.Connection, state: SeedState, anchor_day_ms: int) -> None:
    conversation_id = state.tutor_conversation_id
    if not conversation_id:
        return
    start = anchor_day_ms - 3 * DAY_MS + 20 * 60 * MINUTE_MS
    scope = json.dumps({"kind": "library", "id": None}, separators=(",", ":"))
    for index, (question, answer) in enumerate(TUTOR_TURNS, start=1):
        created = start + index * 2 * MINUTE_MS
        request = json.dumps({"mode": "explain", "text": question}, separators=(",", ":"))
        connection.execute(
            """INSERT INTO tutor_turns (
                   id, conversation_id, operation_id, sequence, request_hash, request_json, mode,
                   user_text, assistant_text, state, citations_json, quiz_json, evidence_current,
                   profile_id, profile_revision, provider_kind, model_id, total_tokens, created_at,
                   updated_at, captured_scope_json
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'explain', ?7, ?8, 'complete', '[]', NULL, 1,
                         NULL, 1, 'ollama', 'llama3.1:8b', ?9, ?10, ?10, ?11)""",
            (
                stable_hex("tutor-turn", str(index)), conversation_id, stable_hex("tutor-operation", str(index)), index,
                sha256_text(request), request, question, answer, 400 + index * 37, created, scope,
            ),
        )
    end = start + (len(TUTOR_TURNS) + 1) * 2 * MINUTE_MS
    connection.execute(
        "UPDATE tutor_conversations SET created_at = ?1, updated_at = ?2, revision = revision + ?3 WHERE id = ?4",
        (start, end, len(TUTOR_TURNS), conversation_id),
    )
