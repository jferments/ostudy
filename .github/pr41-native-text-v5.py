from pathlib import Path
import re


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# PDF recovery v5: separate localized glyph damage from page-wide unusability.
# Nunn-style textbook PDFs can have readable body text plus custom-font C0,
# private-use, replacement, or noncharacter glyphs. Those defects must never
# cause a whole page to be rasterized and OCR'd merely because they exist.
# ---------------------------------------------------------------------------
pdf_path = "src-tauri/src/pdf_backend.rs"
pdf = read(pdf_path)
if pdf.count("oxidize-pdf-4.2.2-rag-native-first-ocr-v4") != 2:
    raise SystemExit("expected v4 PDF backend id exactly twice")
pdf = pdf.replace(
    "oxidize-pdf-4.2.2-rag-native-first-ocr-v4",
    "oxidize-pdf-4.2.2-rag-native-first-ocr-v5",
)

old = '''        let raw = assess_text_quality(&page.text);
        let extraction_failed = document.native_text_failed_pages.contains(&page_number);
        if extraction_failed || raw.suspicious {
            let mut reasons = if extraction_failed {
                vec!["native_text_extraction_failed"]
            } else {
                raw.reasons.clone()
            };
            if extraction_failed && raw.suspicious {
                reasons.extend(raw.reasons.iter().copied());
            }
            reasons.sort_unstable();
            reasons.dedup();
            ocr_pages.push(OcrPageCandidate {
                page_number,
                required: true,
                reasons,
            });
            continue;
        }
        if page.text.trim().is_empty() && !has_good_semantics {
'''
new = '''        let raw = assess_text_quality(&page.text);
        let extraction_failed = document.native_text_failed_pages.contains(&page_number);
        if extraction_failed {
            let mut reasons = vec!["native_text_extraction_failed"];
            reasons.extend(raw.reasons.iter().copied());
            reasons.sort_unstable();
            reasons.dedup();
            ocr_pages.push(OcrPageCandidate {
                page_number,
                required: true,
                reasons,
            });
            continue;
        }
        if page.text.trim().is_empty() && !has_good_semantics {
'''
if pdf.count(old) != 1:
    raise SystemExit("plan_text_recovery raw suspicious block not found")
pdf = pdf.replace(old, new, 1)

old = '''        if !raw.substantive || has_suspicious_semantics || document.semantic_analysis_degraded {
            native_semantic_fallback_pages.push(page_number);
        }
'''
new = '''        if native_page_requires_ocr(&page.text, &raw) {
            ocr_pages.push(OcrPageCandidate {
                page_number,
                required: true,
                reasons: raw.reasons.clone(),
            });
            continue;
        }
        if raw.suspicious
            || !raw.substantive
            || has_suspicious_semantics
            || document.semantic_analysis_degraded
        {
            // A page can contain malformed glyph mappings while its surrounding
            // native prose remains perfectly readable. In that case rebuild
            // semantics from cleaned native page text; do not pay the OCR cost.
            native_semantic_fallback_pages.push(page_number);
        }
'''
if pdf.count(old) != 1:
    raise SystemExit("plan_text_recovery semantic fallback block not found")
pdf = pdf.replace(old, new, 1)

marker = '''pub(crate) fn plan_text_recovery(document: &ExtractedDocument) -> TextRecoveryPlan {
'''
helper = '''fn native_page_requires_ocr(text: &str, assessment: &TextQualityAssessment) -> bool {
    if !assessment.suspicious {
        return false;
    }
    // Font-substitution ciphers can look alphabetic while being unreadable, so
    // this whole-page pattern still requires OCR. The other quality signals are
    // often localized failures of individual embedded font maps; only escalate
    // those when corruption dominates the page rather than merely appearing.
    if assessment.reasons.contains(&"probable_font_substitution") {
        return true;
    }

    let mut visible = 0_usize;
    let mut damaged = 0_usize;
    for character in text.chars() {
        if character.is_whitespace() {
            continue;
        }
        visible = visible.saturating_add(1);
        if is_unmapped_pdf_glyph(character) {
            damaged = damaged.saturating_add(1);
        }
    }
    // Require both a meaningful absolute amount and severe density. This keeps
    // image/table-heavy textbook pages out of OCR when they have less prose and
    // therefore a deceptively high percentage of a few broken symbol glyphs.
    damaged >= 24 && damaged.saturating_mul(100) >= visible.max(1).saturating_mul(30)
}

fn is_unmapped_pdf_glyph(character: char) -> bool {
    let value = character as u32;
    (character.is_control() && !matches!(character, '\\n' | '\\t'))
        || character == '\\u{fffd}'
        || ('\\u{e000}'..='\\u{f8ff}').contains(&character)
        || (0xFDD0..=0xFDEF).contains(&value)
        || value & 0xFFFF == 0xFFFE
        || value & 0xFFFF == 0xFFFF
}

'''
if pdf.count(marker) != 1:
    raise SystemExit("plan_text_recovery marker not found")
pdf = pdf.replace(marker, helper + marker, 1)

old_test = '''    #[test]
    fn recovered_ocr_text_replaces_corrupt_page_and_semantic_content() {
'''
new_tests = '''    #[test]
    fn nunn_style_font_map_damage_uses_native_fallback_without_ocr() {
        let page = concat!(
            "The respiratory tract contains readable scientific prose. ",
            "\\u{0007}e alveolar septa remain thin and gas exchange remains efficient. ",
            "The active side is around 0.3 \\u{001e}m thick. Airways are divided into bronchi and bronchioles. ",
            "Text from figures can contain a stray \\u{fffe} marker while the paragraphs remain usable. ",
            "This sentence deliberately adds enough ordinary English text to model an image-heavy textbook page."
        );
        let assessment = assess_text_quality(page);
        let document = base_document(page, "\\u{e01c} bad semantic font mapping");
        let plan = plan_text_recovery(&document);
        assert!(assessment.substantive);
        assert!(plan.ocr_pages.is_empty());
        assert_eq!(plan.native_semantic_fallback_pages, vec![1]);
    }

    #[test]
    fn catastrophic_page_wide_unmapped_glyphs_still_require_ocr() {
        let corrupt = (0..40)
            .map(|_| "\\u{e001}\\u{e002}x")
            .collect::<String>();
        let assessment = assess_text_quality(&corrupt);
        assert!(assessment.suspicious);
        assert!(native_page_requires_ocr(&corrupt, &assessment));
        let document = base_document(&corrupt, "");
        let plan = plan_text_recovery(&document);
        assert_eq!(plan.ocr_pages.len(), 1);
        assert!(plan.ocr_pages[0].required);
    }

    #[test]
    fn font_substitution_cipher_still_requires_ocr_even_when_letters_dominate() {
        let corrupt = "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× èåÝ çôó Õâóïáäèå ëóèïç Õííîäáèçáîå ñðæãáíôóÝ åóØ ÛðáÝóãáåóí";
        let assessment = assess_text_quality(corrupt);
        assert!(native_page_requires_ocr(corrupt, &assessment));
    }

    #[test]
    fn recovered_ocr_text_replaces_corrupt_page_and_semantic_content() {
'''
if pdf.count(old_test) != 1:
    raise SystemExit("PDF test insertion point not found")
pdf = pdf.replace(old_test, new_tests, 1)
write(pdf_path, pdf)

# Backend identity fixture.
replace_once(
    "src-tauri/src/document_pdf_tests.rs",
    '        "oxidize-pdf-4.2.2-rag-native-first-ocr-v4"\n',
    '        "oxidize-pdf-4.2.2-rag-native-first-ocr-v5"\n',
)

# Native text cleaning: remove unmapped glyph placeholders before semantic
# reconstruction/provider traffic. Do not guess which scientific symbol a broken
# custom font meant; preserve surrounding readable text instead.
ingestion_path = "src-tauri/src/document_ingestion.rs"
ingestion = read(ingestion_path)
old = '''fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\\n' | '\\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}
'''
new = '''fn normalize_semantic_text(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !is_unmapped_pdf_text_character(*character))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_string()
}

fn is_unmapped_pdf_text_character(character: char) -> bool {
    let value = character as u32;
    (character.is_control() && !matches!(character, '\\n' | '\\t'))
        || character == '\\u{fffd}'
        || ('\\u{e000}'..='\\u{f8ff}').contains(&character)
        || (0xFDD0..=0xFDEF).contains(&value)
        || value & 0xFFFF == 0xFFFE
        || value & 0xFFFF == 0xFFFF
}
'''
if ingestion.count(old) != 1:
    raise SystemExit("normalize_semantic_text block not found")
ingestion = ingestion.replace(old, new, 1)
old = '''    #[test]
    fn extraction_modes_have_stable_database_values() {
'''
new = '''    #[test]
    fn normalizes_nunn_style_unmapped_font_glyphs_without_discarding_native_text() {
        assert_eq!(
            normalize_page_text("\\u{0007}e airway has 0.3 \\u{001e}m text air\\u{fffe}ways and \\u{e001} one bad private glyph"),
            "e airway has 0.3 m text airways and  one bad private glyph"
        );
    }

    #[test]
    fn extraction_modes_have_stable_database_values() {
'''
if ingestion.count(old) != 1:
    raise SystemExit("ingestion test insertion point not found")
ingestion = ingestion.replace(old, new, 1)
write(ingestion_path, ingestion)

# ---------------------------------------------------------------------------
# llama.cpp token accounting. Prefer standard OpenAI usage. When only llama.cpp
# timing/native fields are exposed, include cached prompt tokens and support the
# legacy __verbose envelope. Accept nonnegative signed integer JSON values too.
# ---------------------------------------------------------------------------
ai_path = "src-tauri/src/ai/document_intelligence.rs"
ai = read(ai_path)
old = '''fn response_token_usage(response: &Value) -> (Option<u64>, Option<u64>, Option<u64>) {
    let read = |pointers: &[&str]| {
        pointers
            .iter()
            .find_map(|pointer| response.pointer(pointer).and_then(Value::as_u64))
    };
    let prompt = read(&[
        "/usage/prompt_tokens",
        "/usage/input_tokens",
        "/timings/prompt_n",
        "/tokens_evaluated",
    ]);
    let completion = read(&[
        "/usage/completion_tokens",
        "/usage/output_tokens",
        "/timings/predicted_n",
        "/tokens_predicted",
    ]);
    let total = read(&["/usage/total_tokens"]).or_else(|| {
        prompt
            .zip(completion)
            .map(|(left, right)| left.saturating_add(right))
    });
    (prompt, completion, total)
}
'''
new = '''fn response_token_usage(response: &Value) -> (Option<u64>, Option<u64>, Option<u64>) {
    let read = |pointers: &[&str]| {
        pointers
            .iter()
            .find_map(|pointer| response.pointer(pointer).and_then(nonnegative_json_integer))
    };

    let standard_prompt = read(&[
        "/usage/prompt_tokens",
        "/usage/input_tokens",
        "/response/usage/prompt_tokens",
        "/data/usage/prompt_tokens",
    ]);
    let standard_completion = read(&[
        "/usage/completion_tokens",
        "/usage/output_tokens",
        "/response/usage/completion_tokens",
        "/data/usage/completion_tokens",
    ]);
    let standard_total = read(&[
        "/usage/total_tokens",
        "/response/usage/total_tokens",
        "/data/usage/total_tokens",
    ]);
    if standard_prompt.is_some() || standard_completion.is_some() || standard_total.is_some() {
        let total = standard_total.or_else(|| {
            standard_prompt
                .zip(standard_completion)
                .map(|(left, right)| left.saturating_add(right))
        });
        return (standard_prompt, standard_completion, total);
    }

    // llama.cpp timing semantics distinguish cached prompt tokens from prompt
    // tokens newly evaluated. A provider total reconstructed from timings must
    // include both; otherwise repeated document windows are substantially
    // undercounted. Older server builds may place these fields in __verbose.
    let prompt_processed = read(&[
        "/timings/prompt_n",
        "/__verbose/timings/prompt_n",
        "/tokens_evaluated",
        "/__verbose/tokens_evaluated",
    ]);
    let cached = read(&[
        "/timings/cache_n",
        "/__verbose/timings/cache_n",
        "/tokens_cached",
        "/__verbose/tokens_cached",
    ])
    .unwrap_or(0);
    let completion = read(&[
        "/timings/predicted_n",
        "/__verbose/timings/predicted_n",
        "/tokens_predicted",
        "/__verbose/tokens_predicted",
    ]);
    let prompt = prompt_processed.map(|value| value.saturating_add(cached));
    let total = prompt
        .zip(completion)
        .map(|(left, right)| left.saturating_add(right));
    (prompt, completion, total)
}

fn nonnegative_json_integer(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}
'''
if ai.count(old) != 1:
    raise SystemExit("response_token_usage block not found")
ai = ai.replace(old, new, 1)
old = '''        assert_eq!(
            response_token_usage(&json!({"timings": {"prompt_n": 220, "predicted_n": 45}})),
            (Some(220), Some(45), Some(265))
        );
        assert_eq!(
            response_token_usage(&json!({"tokens_evaluated": 80, "tokens_predicted": 20})),
            (Some(80), Some(20), Some(100))
        );
'''
new = '''        assert_eq!(
            response_token_usage(&json!({
                "timings": {"cache_n": 180, "prompt_n": 40, "predicted_n": 45}
            })),
            (Some(220), Some(45), Some(265))
        );
        assert_eq!(
            response_token_usage(&json!({
                "tokens_cached": 20, "tokens_evaluated": 80, "tokens_predicted": 20
            })),
            (Some(100), Some(20), Some(120))
        );
        assert_eq!(
            response_token_usage(&json!({
                "__verbose": {
                    "timings": {"cache_n": 60, "prompt_n": 15, "predicted_n": 25}
                }
            })),
            (Some(75), Some(25), Some(100))
        );
        assert_eq!(
            response_token_usage(&json!({
                "response": {"usage": {"prompt_tokens": 9, "completion_tokens": 4, "total_tokens": 13}}
            })),
            (Some(9), Some(4), Some(13))
        );
'''
if ai.count(old) != 1:
    raise SystemExit("token usage test fragment not found")
ai = ai.replace(old, new, 1)
write(ai_path, ai)

# ---------------------------------------------------------------------------
# Settings UI: roadmap-only planned providers don't belong in the form.
# ---------------------------------------------------------------------------
ui_path = "src/screens/settings/AiLlmSettings.tsx"
ui = read(ui_path)
old = '''  const plannedProviders = controller.catalog.filter(
    (provider) => provider.implementationStatus === "planned",
  );
'''
if ui.count(old) != 1:
    raise SystemExit("plannedProviders variable not found")
ui = ui.replace(old, "", 1)
old = '''
        <section className={styles.settingsCard}>
          <h2>Planned providers</h2>
          <div className={styles.plannedProviderGrid}>
            {plannedProviders.map((provider) => (
              <article key={provider.providerKind}>
                <strong>{provider.displayName}</strong>
                <p>{provider.description}</p>
              </article>
            ))}
          </div>
        </section>
'''
if ui.count(old) != 1:
    raise SystemExit("planned providers section not found")
ui = ui.replace(old, "", 1)
write(ui_path, ui)

css_path = "src/screens/settings/Settings.module.css"
css = read(css_path)
css = css.replace(",\n.plannedProviderGrid p", "", 1)
css = css.replace(",\n.plannedProviderGrid {", " {", 1)
css = re.sub(r'''\n\.plannedProviderGrid article \{[\s\S]*?\n\.plannedProviderGrid p \{[\s\S]*?\n\}\n''', "\n", css, count=1)
css = css.replace(",\n  .plannedProviderGrid,", ",", 1)
css = css.replace(",\n  .plannedProviderGrid {", " {", 1)
write(css_path, css)

# Test that the roadmap section is absent.
settings_test = "src/screens/SettingsScreen.test.tsx"
test = read(settings_test)
old = '''    expect(screen.queryByRole("button", { name: /Add DeepSeek/u })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
'''
new = '''    expect(screen.queryByRole("button", { name: /Add DeepSeek/u })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Planned providers" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
'''
if test.count(old) != 1:
    raise SystemExit("settings test insertion point not found")
test = test.replace(old, new, 1)
write(settings_test, test)

# Architecture wording: explicitly document the two-level quality boundary.
doc_path = "docs/architecture/ai-and-document-processing.md"
doc = read(doc_path)
old = '''Good raw page text with bad semantic structure uses page-text semantic fallback instead of OCR. **Readable native text is never replaced merely because it is short or sparse**: title pages, headings, captions, and other low-text pages keep the oxidize-pdf text and use the PDFium raster as visual grounding when the provider supports images. OCR is reserved for native extraction failures, detected corrupt/obfuscated text, or genuinely empty native text where recognition is needed. Corrupt non-empty raw text requires trustworthy OCR before provider traffic. Blank/image-only pages may remain visual evidence when no text is recoverable. Recovered text passes the same quality boundary again; ostudy does not hard-code a substitution alphabet for one PDF/font.
'''
new = '''Good raw page text with bad semantic structure uses page-text semantic fallback instead of OCR. The quality system deliberately has two levels: localized malformed glyph mappings invalidate structured semantics and are removed from the native page-text fallback, while only page-wide unusability is allowed to invoke OCR. **Readable native text is never replaced merely because it is short, image-heavy, table-heavy, or contains a minority of unmapped scientific-symbol/font glyphs.** This prevents image-heavy textbook pages from crossing an OCR threshold simply because they contain less prose. OCR is reserved for native extraction failures, genuinely empty native text where recognition is needed, whole-page substitution/obfuscation, or corruption that dominates the visible text layer. Blank/image-only pages may remain visual evidence when no text is recoverable. Recovered text passes the same quality boundary again; ostudy does not hard-code a substitution alphabet for one PDF/font.
'''
if doc.count(old) != 1:
    raise SystemExit("architecture OCR paragraph not found")
doc = doc.replace(old, new, 1)
write(doc_path, doc)

print("PR41 native-text v5/token/UI fixes applied")
