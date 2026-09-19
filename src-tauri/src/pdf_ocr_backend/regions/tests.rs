use super::{parsers, *};

const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

fn result(text: &str, lines: Vec<OcrLine>) -> RegionOcrResult {
    RegionOcrResult {
        version: 1,
        text: text.to_string(),
        confidence: 0.9,
        lines,
        duration_millis: 12,
        engine: "fixture-native-ocr-v1".to_string(),
        language: "eng".to_string(),
        width: 200,
        height: 100,
    }
}

fn bounds(left: u32, top: u32, right: u32, bottom: u32) -> PixelBounds {
    PixelBounds {
        left,
        top,
        right,
        bottom,
    }
}

#[test]
fn image_ocr_tsv_retains_actual_lines_bounds_and_the_weakest_word_confidence() {
    let tsv = format!(
        "{HEADER}{}",
        concat!(
            "5\t1\t2\t3\t1\t1\t10\t20\t30\t10\t99\tDose\n",
            "5\t1\t2\t3\t1\t2\t50\t21\t20\t9\t40\t0.50\n",
            "5\t1\t2\t3\t1\t3\t75\t20\t10\t10\t95\tµg\n",
            "5\t1\t2\t3\t2\t1\t10\t40\t70\t10\t91\tnot positive\n",
        )
    );
    let lines = parsers::tsv_lines(&tsv);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "Dose 0.50 µg");
    assert_eq!(lines[0].bounds, Some(bounds(10, 20, 85, 30)));
    assert_eq!(lines[0].confidence, Some(0.4));
    assert_eq!(lines[0].group, "2:3");
    assert_eq!(lines[1].text, "not positive");
    result("Dose 0.50 µg\nnot positive", lines)
        .validate()
        .unwrap();
}

#[test]
fn image_ocr_invalid_word_coordinates_never_remove_literals_or_guess_a_box() {
    for invalid in ["-1", "4294967295", "NaN"] {
        let tsv = format!("{HEADER}5\t1\t1\t1\t1\t1\t1\t1\t4\t4\t95\t<\n5\t1\t1\t1\t1\t2\t{invalid}\t1\t10\t4\t95\t10\n");
        let lines = parsers::tsv_lines(&tsv);
        assert_eq!(lines[0].text, "< 10");
        assert_eq!(lines[0].bounds, None);
        result("< 10", lines).validate().unwrap();
    }
}

#[test]
fn image_ocr_hocr_retains_nested_literal_text_and_extractor_coordinates() {
    let hocr = r#"<div class="ocr_page"><p class="ocr_par" id="par_1">
      <span class="ocr_line" title="bbox 10 20 100 40; baseline 0 -2">
        <span class="ocrx_word" title="bbox 10 20 30 40; x_wconf 98"><strong>x²</strong></span>
        <span class="ocrx_word" title="bbox 40 20 50 40; x_wconf 75">&lt;</span>
        <span class="ocrx_word" title="bbox 60 20 100 40; x_wconf 90">0.50</span>
      </span></p></div>"#;
    let lines = parsers::hocr_lines(hocr);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "x² < 0.50");
    assert_eq!(lines[0].bounds, Some(bounds(10, 20, 100, 40)));
    assert_eq!(lines[0].confidence, Some(0.75));
    assert_eq!(lines[0].group, "par_1");
    result("x² < 0.50", lines).validate().unwrap();
    let inconsistent = hocr.replace("bbox 10 20 100 40; baseline", "bbox 10 20 80 40; baseline");
    assert_eq!(parsers::hocr_lines(&inconsistent)[0].bounds, None);
}

#[test]
fn image_ocr_unparseable_or_inconsistent_layout_falls_back_without_text_loss() {
    let text = "Use 5 mg, not 50 mg.\nx² > x³ is not universally true.";
    for lines in [
        parsers::hocr_lines("<div>broken"),
        parsers::hocr_lines(r#"<!DOCTYPE d [<!ENTITY e SYSTEM "file:///secret">]><d>&e;</d>"#),
        vec![OcrLine {
            text: "Use 50 mg".to_string(),
            bounds: Some(bounds(1, 1, 100, 10)),
            group: "wrong".to_string(),
            confidence: Some(0.99),
        }],
    ] {
        let mut output = result(text, lines);
        assert!(output.validate().is_err());
        output.reconcile_geometry();
        output.validate().unwrap();
        assert_eq!(output.text, text);
        assert_eq!(output.lines.len(), 1);
        assert_eq!(output.lines[0].text, text);
        assert_eq!(output.lines[0].bounds, None);
    }
}

#[test]
fn image_ocr_validation_rejects_forged_order_numbers_geometry_and_versions() {
    let line = OcrLine {
        text: "not positive".to_string(),
        bounds: Some(bounds(1, 1, 90, 10)),
        group: String::new(),
        confidence: Some(0.9),
    };
    let valid = result("not positive", vec![line]);
    valid.validate().unwrap();
    for altered in ["positive", "positive not", "not  positive 1"] {
        let mut candidate = valid.clone();
        candidate.lines[0].text = altered.to_string();
        assert!(candidate.validate().is_err());
    }
    let mut candidate = valid.clone();
    candidate.lines[0].bounds = Some(bounds(1, 1, 300, 10));
    assert!(candidate.validate().is_err());
    candidate.reconcile_geometry();
    assert_eq!(candidate.lines[0].bounds, None);
    candidate.validate().unwrap();
    let mut candidate = valid.clone();
    candidate.version = 2;
    assert!(candidate.validate().is_err());
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let mut candidate = valid.clone();
        candidate.lines[0].confidence = Some(value);
        assert!(candidate.validate().is_err());
    }
    let encoded = serde_json::to_vec(&valid).unwrap();
    assert_eq!(
        serde_json::from_slice::<RegionOcrResult>(&encoded).unwrap(),
        valid
    );
}

#[test]
fn image_ocr_excess_layout_is_not_misrepresented_as_complete_partial_text() {
    let mut tsv = HEADER.to_string();
    for line in 0..=MAX_OCR_LINES {
        tsv.push_str(&format!("5\t1\t1\t1\t{line}\t1\t1\t1\t10\t10\t90\tword\n"));
    }
    assert!(parsers::tsv_lines(&tsv).is_empty());
    let text = "word ".repeat(MAX_OCR_LINES + 1);
    let mut output = result(&text, Vec::new());
    output.reconcile_geometry();
    output.validate().unwrap();
    assert_eq!(output.lines[0].text, text);
    assert!(output.lines[0].bounds.is_none());
    output.text = "a".repeat(MAX_OCR_TEXT + 1);
    output.reconcile_geometry();
    assert!(output.validate().is_err());
}
