use std::{env, fs, path::PathBuf};

// TrueType faces compiled into the study-guide PDF exporter. They are
// downloaded by `scripts/install-local-fonts.mjs` (run by `pnpm install`) and
// listed in `pdfFontSources` in `scripts/font-sources.mjs`.
const PDF_FONT_FILES: &[&str] = &[
    "ibm-plex-sans-400.ttf",
    "ibm-plex-sans-400-italic.ttf",
    "ibm-plex-sans-600.ttf",
    "ibm-plex-sans-700.ttf",
    "lato-400.ttf",
    "lato-400-italic.ttf",
    "lato-700.ttf",
    "sora-400.ttf",
    "sora-600.ttf",
    "sora-700.ttf",
    "roboto-slab-400.ttf",
    "roboto-slab-700.ttf",
    "caladea-400.ttf",
    "caladea-700.ttf",
];

fn main() {
    embed_pdf_fonts();
    tauri_build::build();
}

fn embed_pdf_fonts() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let directory = manifest.join("resources").join("pdf-fonts");
    println!("cargo:rerun-if-changed={}", directory.display());
    let missing = PDF_FONT_FILES
        .iter()
        .filter(|name| !directory.join(name).is_file())
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        panic!(
            "Study-guide PDF fonts are missing from {}: {}. Run `pnpm run fonts:install` \
             from the repository root to download them.",
            directory.display(),
            missing.join(", ")
        );
    }
    let mut generated = String::from("&[\n");
    for name in PDF_FONT_FILES {
        let path = directory.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        generated.push_str(&format!("    include_bytes!({:?}),\n", path.display().to_string()));
    }
    generated.push(']');
    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("pdf_fonts.rs");
    fs::write(output, generated).expect("write generated PDF font list");
}
