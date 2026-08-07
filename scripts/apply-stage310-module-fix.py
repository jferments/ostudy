#!/usr/bin/env python3
"""Apply the final Stage 3.10 authoring façade compiler fix."""

from pathlib import Path

root = Path(__file__).resolve().parents[1]
path = root / "src-tauri/src/authoring/mod.rs"
text = path.read_text(encoding="utf-8")
old = """pub(crate) use card_types_and_templates::{
    apply_stage3_data_repairs, projected_presentation_variant, render_projected_presentation,
    ProjectedPresentationInput,
};"""
new = """pub(crate) use card_types_and_templates::{
    apply_stage3_data_repairs, projected_presentation_variant, render_projected_presentation,
    ProjectedPresentationInput, RenderedCardPresentation,
};"""
if text.count(old) != 1:
    raise SystemExit("authoring/mod.rs did not match the expected post-module-fix façade")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Restored the RenderedCardPresentation façade reexport.")
