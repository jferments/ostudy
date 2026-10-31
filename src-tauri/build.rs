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
    require_lf_migrations();
    embed_pdf_fonts();
    remove_stale_pdfium_file();
    tauri_build::build();
}

/// Earlier Windows builds copied pdfium.dll to a file named `native/pdfium`
/// next to the executable. Copying resources now needs that path to be a
/// directory, and fails with "File exists" while the old file remains.
fn remove_stale_pdfium_file() {
    let Some(out_dir) = env::var_os("OUT_DIR") else {
        return;
    };
    // OUT_DIR is <profile>/build/<package>-<hash>/out.
    let Some(profile_dir) = PathBuf::from(out_dir).ancestors().nth(3).map(PathBuf::from) else {
        return;
    };
    let stale = profile_dir.join("native").join("pdfium");
    if stale.is_file() {
        fs::remove_file(&stale).expect("remove the stale native/pdfium file from an earlier build");
    }
}

/// The release migrations are compiled in and checked by SHA-256 at startup,
/// so a checkout that turned their line endings into CRLF builds an app that
/// cannot open its library. Refuse to build it instead.
fn require_lf_migrations() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let directory = manifest.join("migrations");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut converted = fs::read_dir(&directory)
        .expect("read the migrations directory")
        .map(|entry| entry.expect("read a migrations entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .filter(|path| fs::read(path).expect("read a migration").contains(&b'\r'))
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    if !converted.is_empty() {
        converted.sort();
        panic!(
            "These migrations have CRLF line endings, which would fail their checksums at \
             startup: {}. Check them out again with LF (from the repository root: \
             `git rm -r -q --cached .` then `git reset -q --hard HEAD`).",
            converted.join(", ")
        );
    }
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
        generated.push_str(&format!(
            "    include_bytes!({:?}),\n",
            path.display().to_string()
        ));
    }
    generated.push(']');
    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("pdf_fonts.rs");
    fs::write(output, generated).expect("write generated PDF font list");
}
