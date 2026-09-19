//! Bounded adapters for native Tesseract output. They never parse source HTML.

use super::{OcrLine, PixelBounds, MAX_OCR_LINES};

pub(crate) fn tsv_lines(tsv: &str) -> Vec<OcrLine> {
    let mut lines = Vec::<OcrLine>::new();
    let mut current = None::<(i64, i64, i64)>;
    for row in tsv.lines().skip(1) {
        let columns = row.splitn(12, '\t').collect::<Vec<_>>();
        if columns.len() != 12 || columns[0] != "5" || columns[11].trim().is_empty() {
            continue;
        }
        // Match the legacy text projection's line grouping exactly. A malformed
        // coordinate invalidates geometry, not the word or a crucial qualifier.
        let key = (
            columns[2].parse::<i64>().unwrap_or_default(),
            columns[3].parse::<i64>().unwrap_or_default(),
            columns[4].parse::<i64>().unwrap_or_default(),
        );
        let bounds = word_bounds(&columns);
        let confidence = columns[10]
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
            .map(|value| value / 100.0);
        if current != Some(key) {
            if lines.len() == MAX_OCR_LINES {
                return Vec::new();
            }
            lines.push(OcrLine {
                text: columns[11].trim().to_string(),
                bounds,
                group: format!("{}:{}", key.0, key.1),
                confidence,
            });
            current = Some(key);
        } else if let Some(line) = lines.last_mut() {
            line.text.push(' ');
            line.text.push_str(columns[11].trim());
            line.bounds = line
                .bounds
                .zip(bounds)
                .map(|(left, right)| left.union(right));
            // A strong neighbour cannot hide a weak decimal, sign or negation.
            line.confidence = line.confidence.zip(confidence).map(|(a, b)| a.min(b));
        }
    }
    lines
}

fn word_bounds(columns: &[&str]) -> Option<PixelBounds> {
    let left = columns[6].parse::<u32>().ok()?;
    let top = columns[7].parse::<u32>().ok()?;
    let width = columns[8].parse::<u32>().ok()?;
    let height = columns[9].parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some(PixelBounds {
        left,
        top,
        right: left.checked_add(width)?,
        bottom: top.checked_add(height)?,
    })
}

pub(crate) fn hocr_lines(hocr: &str) -> Vec<OcrLine> {
    if hocr.len() > 16 * 1024 * 1024 {
        return Vec::new();
    }
    // No external retrieval or DTD processing. Unparseable native output falls
    // back to truthful unlocated text at the result reconciliation boundary.
    let Ok(document) = roxmltree::Document::parse(hocr) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for node in document
        .descendants()
        .filter(|node| has_class(*node, "ocr_line") || has_class(*node, "ocrx_line"))
    {
        if lines.len() == MAX_OCR_LINES {
            return Vec::new();
        }
        let words = node
            .descendants()
            .filter(|word| has_class(*word, "ocrx_word"))
            .collect::<Vec<_>>();
        let text = words
            .iter()
            .map(|word| {
                word.descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");
        if text.trim().is_empty() {
            continue;
        }
        let mut bounds = node.attribute("title").and_then(hocr_bounds);
        // Inconsistent native boxes are not safe regions for inclusion masks.
        // Missing word boxes do not invalidate an independently supplied line
        // box; a supplied word box outside that line does invalidate it.
        let supplied_word_bounds = words
            .iter()
            .filter_map(|word| word.attribute("title").and_then(hocr_bounds))
            .reduce(PixelBounds::union);
        if let Some((line, words)) = bounds.zip(supplied_word_bounds) {
            if !line.contains(words) {
                bounds = None;
            }
        }
        let confidence = words
            .iter()
            .map(|word| {
                word.attribute("title")
                    .and_then(|title| hocr_attribute(title, "x_wconf"))
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
                    .map(|value| value / 100.0)
            })
            .try_fold(1.0_f64, |minimum, value| {
                value.map(|value| minimum.min(value))
            });
        let group = node
            .ancestors()
            .find(|parent| has_class(*parent, "ocr_par"))
            .and_then(|parent| parent.attribute("id"))
            .unwrap_or_default();
        if group.len() > 128 {
            return Vec::new();
        }
        lines.push(OcrLine {
            text,
            bounds,
            group: group.to_string(),
            confidence,
        });
    }
    lines
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.is_element()
        && node
            .attribute("class")
            .is_some_and(|value| value.split_ascii_whitespace().any(|value| value == class))
}

fn hocr_attribute<'a>(title: &'a str, name: &str) -> Option<&'a str> {
    title.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once(' ')?;
        (key == name).then_some(value.trim())
    })
}

fn hocr_bounds(title: &str) -> Option<PixelBounds> {
    let numbers = hocr_attribute(title, "bbox")?
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if numbers.len() != 4 {
        return None;
    }
    let bounds = PixelBounds {
        left: numbers[0],
        top: numbers[1],
        right: numbers[2],
        bottom: numbers[3],
    };
    (bounds.left < bounds.right && bounds.top < bounds.bottom).then_some(bounds)
}
