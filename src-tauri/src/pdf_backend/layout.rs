//! Rebuilds a page the structure reader broke into scraps from the page's own
//! text runs, keeping its section headings.
//!
//! The structure reader starts a new element at every small glyph, so a page
//! full of Na⁺-K⁺ or HCO₃⁻ comes out as pieces such as "-K" and "+" under
//! headings that are really pieces of a sentence. The flat page text keeps the
//! sentences whole but loses the headings, and on a two-column page with a
//! figure it can read the right column before the left. Here the page's text
//! runs are set back into lines, read column by column, and split into
//! sections at the lines set in a heading's own font.

use super::contracts::{ExtractedSemanticChunk, ExtractedSemanticElement, SemanticBBox};

/// One run of text as the PDF draws it, in PDF points with y upward.
#[derive(Clone, Debug)]
pub(crate) struct TextRun {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size: f64,
    pub font: Option<String>,
}

/// The rebuilt page: its text in reading order and its sections.
#[derive(Debug)]
pub(crate) struct RebuiltPage {
    pub text: String,
    pub sections: Vec<Section>,
    /// The page's small print: figure captions first, then figure labels and
    /// other small text, kept out of the sections so it does not break up
    /// their sentences.
    pub figure: Option<Section>,
}

#[derive(Debug)]
pub(crate) struct Section {
    pub heading: Option<String>,
    pub heading_box: Option<SemanticBBox>,
    pub body: String,
    pub body_box: SemanticBBox,
}

/// A run at most this share of the body size is a sub- or superscript.
const SMALL_RUN_RATIO: f64 = 0.85;
/// Runs whose baselines differ by at most this share of the body size share a
/// line.
const SAME_LINE_RATIO: f64 = 0.25;
/// A small run within this share of the body size of a line's baseline is one
/// of its marks.
const MARK_LINE_RATIO: f64 = 0.6;
/// A gap wider than this many body sizes starts a new piece of line.
const LINE_GAP_RATIO: f64 = 1.0;
/// A page whose full-size runs cross its middle at most this share of the
/// time has two columns.
const MAX_CROSSING_SHARE: f64 = 0.1;
/// A gap wider than this share of the body size is a space.
const SPACE_GAP_RATIO: f64 = 0.15;
/// A heading is at least this share of the body size and at most this many
/// words.
const HEADING_MIN_SIZE_RATIO: f64 = 0.8;
const HEADING_MAX_WORDS: usize = 10;
/// A heading's baseline sits at least this many line spacings below the line
/// above it.
const HEADING_SPACE_RATIO: f64 = 1.4;
/// Chunks are kept within the structure chunker's size, counted in words.
const MAX_CHUNK_WORDS: usize = 400;

struct Line {
    runs: Vec<TextRun>,
    x0: f64,
    x1: f64,
    y: f64,
}

impl Line {
    fn text(&self, body: f64) -> String {
        let mut text = String::new();
        let mut end = f64::NEG_INFINITY;
        for run in &self.runs {
            let gap = run.x - end;
            if !text.is_empty()
                && gap > body * SPACE_GAP_RATIO
                && !text.ends_with(char::is_whitespace)
                && !run.text.starts_with(char::is_whitespace)
            {
                text.push(' ');
            }
            text.push_str(&run.text);
            end = end.max(run.x + run.width);
        }
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn full_size_runs(&self, body: f64) -> impl Iterator<Item = &TextRun> {
        self.runs.iter().filter(move |run| {
            run.font_size > body * SMALL_RUN_RATIO && !run.text.trim().is_empty()
        })
    }

    fn bbox(&self) -> SemanticBBox {
        bbox_of(self.runs.iter())
    }
}

/// Rebuilds a page from its text runs, or `None` when the runs hold no line
/// of body text.
pub(crate) fn rebuild_page(runs: &[TextRun]) -> Option<RebuiltPage> {
    let runs = runs
        .iter()
        .filter(|run| {
            !run.text.is_empty()
                && [run.x, run.y, run.width, run.font_size]
                    .iter()
                    .all(|value| value.is_finite())
                && run.font_size > 0.0
        })
        .cloned()
        .collect::<Vec<_>>();
    let visible = || runs.iter().filter(|run| !run.text.trim().is_empty());
    let body =
        dominant(visible().map(|run| (run.font_size.to_bits(), run))).map(|run| run.font_size)?;
    let body_font = dominant(
        visible()
            .filter(|run| run.font_size > body * SMALL_RUN_RATIO)
            .map(|run| (run.font.clone(), run)),
    )
    .and_then(|run| run.font.clone());
    let gutter = column_gutter(&runs, body);
    let mut lines = reading_order(build_lines(runs, body, gutter), gutter);
    lines.retain(|line| !line.text(body).is_empty());
    if lines.is_empty() {
        return None;
    }

    let is_body_line = |line: &Line| {
        line.full_size_runs(body)
            .any(|run| run.font == body_font && (run.font_size - body).abs() < 0.5)
    };
    let running_head = running_head(&lines, body);
    let is_small_print =
        |line: &Line| line.full_size_runs(body).next().is_none() || running_head == Some(line.y);
    let leading = leading(&lines, body);
    let mut headings = vec![false; lines.len()];
    for (index, line) in lines.iter().enumerate() {
        let next_is_body = lines[index + 1..]
            .iter()
            .filter(|next| !is_small_print(next))
            .find(|next| same_column(line, next))
            .is_some_and(&is_body_line);
        // A heading has its row of the column to itself; a table's header
        // row does not.
        let alone = !lines.iter().any(|other| {
            !std::ptr::eq(other, line)
                && !is_small_print(other)
                && (other.y - line.y).abs() <= body * SAME_LINE_RATIO
                && side(other.x0, gutter) == side(line.x0, gutter)
        });
        // A heading has more space above it than the lines of text have.
        let spaced = lines[..index]
            .iter()
            .rev()
            .filter(|previous| !is_small_print(previous))
            .find(|previous| same_column(line, previous))
            .is_none_or(|previous| previous.y - line.y >= leading * HEADING_SPACE_RATIO);
        headings[index] = next_is_body
            && alone
            && spaced
            && !is_small_print(line)
            && is_heading(line, &line.text(body), body, &body_font);
    }

    let mut sections = Vec::<Section>::new();
    let mut captions = Vec::<&Line>::new();
    let mut small_print = Vec::<&Line>::new();
    let mut in_caption = false;
    let mut current: Option<Pending> = None;
    for (line, is_heading) in lines.iter().zip(headings) {
        let line_text = line.text(body);
        if is_small_print(line) {
            in_caption = is_caption(&line_text) || in_caption;
            if in_caption {
                captions.push(line);
            } else {
                small_print.push(line);
            }
            continue;
        }
        in_caption = false;
        if is_heading {
            if let Some(section) = current.take() {
                sections.extend(finish(section, body));
            }
            current = Some((Some((line_text, line.bbox())), Vec::new()));
        } else {
            current
                .get_or_insert_with(|| (None, Vec::new()))
                .1
                .push(line);
        }
    }
    if let Some(section) = current {
        sections.extend(finish(section, body));
    }
    if !lines.iter().any(is_body_line) {
        return None;
    }
    captions.extend(small_print);
    let figure = finish((None, captions), body);
    // The page text holds each section whole, then the small print, so every
    // chunk is a stretch of it.
    let mut text = String::new();
    for section in sections.iter().chain(&figure) {
        for part in section.heading.iter().chain([&section.body]) {
            if !part.is_empty() {
                text.push_str(part);
                text.push('\n');
            }
        }
    }
    Some(RebuiltPage {
        text: text.trim_end().to_string(),
        sections,
        figure,
    })
}

/// The usual distance between the baselines of consecutive lines.
fn leading(lines: &[Line], body: f64) -> f64 {
    let mut gaps = lines
        .windows(2)
        .map(|pair| pair[0].y - pair[1].y)
        .filter(|gap| *gap > body * 0.5 && *gap < body * 3.0)
        .collect::<Vec<_>>();
    gaps.sort_by(f64::total_cmp);
    gaps.get(gaps.len() / 2).copied().unwrap_or(body * 1.2)
}

/// The baseline of the page's running head: its top line, set apart from the
/// text below it, when that line is short.
fn running_head(lines: &[Line], body: f64) -> Option<f64> {
    let mut baselines = lines.iter().map(|line| line.y).collect::<Vec<_>>();
    baselines.sort_by(|a, b| b.total_cmp(a));
    let top = *baselines.first()?;
    let below = baselines
        .iter()
        .find(|y| top - **y > body * SAME_LINE_RATIO)?;
    let words = lines
        .iter()
        .filter(|line| (line.y - top).abs() <= body * SAME_LINE_RATIO)
        .map(|line| line.text(body).split_whitespace().count())
        .sum::<usize>();
    (top - below >= body * 1.5 && words <= HEADING_MAX_WORDS + 2).then_some(top)
}

fn is_caption(text: &str) -> bool {
    [
        "Fig.", "Figure ", "FIGURE ", "FIG.", "Table ", "TABLE ", "Box ",
    ]
    .iter()
    .any(|caption| text.starts_with(caption))
}

/// A section being gathered: its heading and box, and its lines so far.
type Pending<'a> = (Option<(String, SemanticBBox)>, Vec<&'a Line>);

fn finish((heading, lines): Pending, body: f64) -> Option<Section> {
    let text = lines
        .iter()
        .map(|line| line.text(body))
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() && heading.is_none() {
        return None;
    }
    let (heading, heading_box) =
        heading.map_or((None, None), |(text, bbox)| (Some(text), Some(bbox)));
    Some(Section {
        heading,
        heading_box,
        body: text,
        body_box: bbox_of(lines.iter().flat_map(|line| line.runs.iter())),
    })
}

/// A heading is one line in a font of its own, at least near body size, a few
/// words starting with a capital and not ending a sentence, followed in its
/// column by body text. Figure captions and running heads are not headings.
fn is_heading(line: &Line, text: &str, body: f64, body_font: &Option<String>) -> bool {
    let mut full = line.full_size_runs(body).peekable();
    if full.peek().is_none() {
        return false;
    }
    let distinct = full.all(|run| {
        run.font_size >= body * HEADING_MIN_SIZE_RATIO
            && (run.font != *body_font || run.font_size > body + 0.5)
    });
    let words = text.split_whitespace().count();
    distinct
        && (1..=HEADING_MAX_WORDS).contains(&words)
        && text
            .chars()
            .filter(|character| character.is_alphabetic())
            .count()
            >= 3
        && text.starts_with(|first: char| first.is_uppercase())
        && !text.ends_with(['.', ',', ';', ':'])
        && !text.ends_with(|last: char| last.is_ascii_digit())
        && !is_caption(text)
}

/// The x of the gap between a page's two columns, when it has two.
fn column_gutter(runs: &[TextRun], body: f64) -> Option<f64> {
    let full = runs
        .iter()
        .filter(|run| run.font_size > body * SMALL_RUN_RATIO && !run.text.trim().is_empty())
        .collect::<Vec<_>>();
    let left = full.iter().map(|run| run.x).reduce(f64::min)?;
    let right = full.iter().map(|run| run.x + run.width).reduce(f64::max)?;
    let middle = (left + right) / 2.0;
    let crossing = full
        .iter()
        .filter(|run| run.x < middle && run.x + run.width > middle)
        .count();
    let on_right = full.iter().filter(|run| run.x >= middle).count();
    let on_left = full.len() - crossing - on_right;
    let two_columns = (crossing as f64) <= MAX_CROSSING_SHARE * full.len() as f64
        && on_left >= 3
        && on_right >= 3;
    two_columns.then_some(middle)
}

fn side(x: f64, gutter: Option<f64>) -> bool {
    gutter.is_some_and(|gutter| x >= gutter)
}

fn build_lines(mut runs: Vec<TextRun>, body: f64, gutter: Option<f64>) -> Vec<Line> {
    runs.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut lines = Vec::<Line>::new();
    for run in runs {
        let joins = lines.iter_mut().rev().take(12).find(|line| {
            (line.y - run.y).abs() <= body * SAME_LINE_RATIO
                && run.x - line.x1 <= body * LINE_GAP_RATIO
                && line.x0 - (run.x + run.width) <= body * LINE_GAP_RATIO
        });
        match joins {
            Some(line) => {
                line.x0 = line.x0.min(run.x);
                line.x1 = line.x1.max(run.x + run.width);
                line.runs.push(run);
            }
            None => lines.push(Line {
                x0: run.x,
                x1: run.x + run.width,
                y: run.y,
                runs: vec![run],
            }),
        }
    }
    // Raised marks sit above their line's baseline and start lines of their
    // own; each joins the full-size line it belongs to.
    let is_marks = |line: &Line| {
        line.runs.iter().all(|run| {
            run.text.trim().is_empty()
                || (run.font_size <= body * SMALL_RUN_RATIO && run.text.trim().chars().count() <= 3)
        })
    };
    let (marks, mut lines): (Vec<_>, Vec<_>) = lines.into_iter().partition(is_marks);
    for mark in marks {
        let nearest = lines
            .iter_mut()
            .filter(|line| {
                line.full_size_runs(body).next().is_some()
                    && (line.y - mark.y).abs() <= body * MARK_LINE_RATIO
                    && side(mark.x0, gutter) == side(line.x0, gutter)
                    && mark.x0 >= line.x0 - body
                    && mark.x0 <= line.x1 + body
            })
            .min_by(|a, b| (a.y - mark.y).abs().total_cmp(&(b.y - mark.y).abs()));
        match nearest {
            Some(line) => {
                line.x1 = line.x1.max(mark.x1);
                line.runs.extend(mark.runs);
            }
            None => lines.push(mark),
        }
    }
    for line in &mut lines {
        line.runs.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    lines
}

/// Lines top to bottom, and on a page with two columns, the left column
/// before the right between any lines that cross the gutter.
fn reading_order(mut lines: Vec<Line>, gutter: Option<f64>) -> Vec<Line> {
    lines.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x0.total_cmp(&b.x0)));
    let Some(gutter) = gutter else {
        return lines;
    };
    let mut ordered = Vec::with_capacity(lines.len());
    let mut left_column = Vec::new();
    let mut right_column = Vec::new();
    for line in lines {
        let crosses = line.x0 < gutter && line.x1 > gutter;
        if crosses {
            ordered.append(&mut left_column);
            ordered.append(&mut right_column);
            ordered.push(line);
        } else if line.x0 < gutter {
            left_column.push(line);
        } else {
            right_column.push(line);
        }
    }
    ordered.append(&mut left_column);
    ordered.append(&mut right_column);
    ordered
}

fn same_column(a: &Line, b: &Line) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1
}

fn dominant<K: Ord, V>(items: impl Iterator<Item = (K, V)>) -> Option<V> {
    let mut counts = std::collections::BTreeMap::<K, (usize, V)>::new();
    for (key, value) in items {
        counts.entry(key).or_insert((0, value)).0 += 1;
    }
    counts
        .into_values()
        .max_by_key(|(count, _)| *count)
        .map(|(_, value)| value)
}

fn bbox_of<'a>(runs: impl Iterator<Item = &'a TextRun>) -> SemanticBBox {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for run in runs {
        x0 = x0.min(run.x);
        y0 = y0.min(run.y);
        x1 = x1.max(run.x + run.width);
        y1 = y1.max(run.y + run.height.max(run.font_size));
    }
    if x0 > x1 {
        return SemanticBBox {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    SemanticBBox {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}

/// The page's elements and chunks from its sections. A section before the
/// page's first heading continues `carried_heading`, the heading the text
/// before it was under.
pub(crate) fn page_structure(
    page_number: usize,
    page: &RebuiltPage,
    carried_heading: Option<&str>,
) -> (Vec<ExtractedSemanticElement>, Vec<ExtractedSemanticChunk>) {
    let mut elements = Vec::new();
    let mut chunks = Vec::new();
    for section in &page.sections {
        if let (Some(heading), Some(bbox)) = (&section.heading, section.heading_box) {
            elements.push(element(page_number, "title", heading, None, bbox));
        }
        let heading = section.heading.as_deref().or(carried_heading);
        if section.body.trim().is_empty() {
            continue;
        }
        elements.push(element(
            page_number,
            "paragraph",
            &section.body,
            heading,
            section.body_box,
        ));
        for part in split_words(&section.body, MAX_CHUNK_WORDS) {
            let full_text = match heading {
                Some(heading) => format!("{heading}\n\n{part}"),
                None => part.clone(),
            };
            chunks.push(ExtractedSemanticChunk {
                chunk_index: 0,
                token_estimate: full_text.chars().count().div_ceil(4).max(1),
                text: part,
                full_text,
                page_numbers: vec![page_number],
                bounding_boxes: vec![section.body_box],
                element_types: if section.heading.is_some() {
                    vec!["title".to_string(), "paragraph".to_string()]
                } else {
                    vec!["paragraph".to_string()]
                },
                heading_context: heading.map(str::to_string),
                is_oversized: false,
            });
        }
    }
    if let Some(figure) = &page.figure {
        elements.push(element(
            page_number,
            "caption",
            &figure.body,
            None,
            figure.body_box,
        ));
        chunks.push(ExtractedSemanticChunk {
            chunk_index: 0,
            token_estimate: figure.body.chars().count().div_ceil(4).max(1),
            text: figure.body.clone(),
            full_text: figure.body.clone(),
            page_numbers: vec![page_number],
            bounding_boxes: vec![figure.body_box],
            element_types: vec!["caption".to_string(), "image".to_string()],
            heading_context: None,
            is_oversized: false,
        });
    }
    (elements, chunks)
}

fn element(
    page_number: usize,
    element_type: &str,
    text: &str,
    heading: Option<&str>,
    bbox: SemanticBBox,
) -> ExtractedSemanticElement {
    ExtractedSemanticElement {
        element_index: 0,
        page_number,
        element_type: element_type.to_string(),
        text: text.to_string(),
        parent_heading: heading.map(str::to_string),
        heading_path: heading.map(str::to_string).into_iter().collect(),
        bbox,
        confidence: 1.0,
        font_name: None,
        font_size: None,
        is_bold: false,
        is_italic: false,
    }
}

/// Splits text at line ends into parts of at most `maximum` words where it can.
fn split_words(text: &str, maximum: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut words = 0;
    for line in text.lines() {
        let count = line.split_whitespace().count();
        if words > 0 && words + count > maximum {
            parts.push(std::mem::take(&mut part));
            words = 0;
        }
        if !part.is_empty() {
            part.push('\n');
        }
        part.push_str(line);
        words += count;
    }
    if !part.trim().is_empty() {
        parts.push(part);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, x: f64, y: f64, font_size: f64, font: &str) -> TextRun {
        TextRun {
            text: text.to_string(),
            x,
            y,
            width: text.chars().count() as f64 * font_size * 0.45,
            height: font_size,
            font_size,
            font: Some(font.to_string()),
        }
    }

    /// One printed line from `x`, its pieces set end to end. A piece starting
    /// with `^` is a raised mark and `_` a lowered one, in a small size.
    fn line(pieces: &[&str], x: f64, y: f64, font: &str) -> Vec<TextRun> {
        let mut x = x;
        let mut runs = Vec::new();
        for piece in pieces {
            let (text, size, y, font) = match piece.chars().next() {
                Some('^') => (&piece[1..], 5.8, y + 4.0, "Marks"),
                Some('_') => (&piece[1..], 5.8, y - 1.5, font),
                _ => (*piece, 9.7, y, font),
            };
            let piece = run(text, x, y, size, font);
            x += piece.width;
            runs.push(piece);
        }
        runs
    }

    /// Page 23 of a physiology chapter in miniature: a figure caption across
    /// the top, then a left column with two headings and a right column the
    /// left one runs into, with charges set as small raised runs.
    fn two_column_page() -> Vec<TextRun> {
        let mut runs = vec![
            run(
                "Fig. 8.17 Mechanism of HCl secretion by gastric parietal cells, as described.",
                60.0,
                510.0,
                7.8,
                "Caption",
            ),
            run("HCl Secretion", 58.0, 460.0, 9.2, "Head"),
            run("CELLULAR MECHANISM", 58.0, 416.0, 8.3, "Small caps"),
        ];
        runs.extend(line(&["into the circulation."], 58.0, 480.0, "Body"));
        runs.extend(line(
            &["A major function of the parietal cells is"],
            58.0,
            448.0,
            "Body",
        ));
        runs.extend(line(&["secretion of HCl."], 58.0, 436.0, "Body"));
        runs.extend(line(
            &[
                "The apical membranes contain H",
                "^+",
                "-K",
                "^+",
                " ATPase, and the",
            ],
            58.0,
            404.0,
            "Body",
        ));
        runs.extend(line(
            &[
                "basolateral membranes contain Na",
                "^+",
                "-K",
                "^+",
                " ATPase.",
            ],
            309.0,
            468.0,
            "Body",
        ));
        runs.extend(line(
            &[
                "H",
                "_2",
                "CO",
                "_3",
                " dissociates into H",
                "^+",
                " and HCO",
                "_3",
                "^−",
                ".",
            ],
            309.0,
            456.0,
            "Body",
        ));
        runs
    }

    #[test]
    fn reads_columns_in_order_and_keeps_their_headings() {
        let page = rebuild_page(&two_column_page()).unwrap();
        assert_eq!(
            page.text,
            "into the circulation.\n\
             HCl Secretion\n\
             A major function of the parietal cells is\n\
             secretion of HCl.\n\
             CELLULAR MECHANISM\n\
             The apical membranes contain H+-K+ ATPase, and the\n\
             basolateral membranes contain Na+-K+ ATPase.\n\
             H2CO3 dissociates into H+ and HCO3−.\n\
             Fig. 8.17 Mechanism of HCl secretion by gastric parietal cells, as described."
        );
        let headings = page
            .sections
            .iter()
            .map(|section| section.heading.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            headings,
            [None, Some("HCl Secretion"), Some("CELLULAR MECHANISM")]
        );
        assert_eq!(
            page.sections[2].body,
            "The apical membranes contain H+-K+ ATPase, and the\n\
             basolateral membranes contain Na+-K+ ATPase.\n\
             H2CO3 dissociates into H+ and HCO3−."
        );
    }

    #[test]
    fn sections_become_chunks_under_their_headings() {
        let page = rebuild_page(&two_column_page()).unwrap();
        let (elements, chunks) = page_structure(23, &page, Some("Gastric Secretion"));
        assert_eq!(chunks.len(), 4);
        assert_eq!(
            chunks[0].heading_context.as_deref(),
            Some("Gastric Secretion")
        );
        assert_eq!(chunks[0].text, "into the circulation.");
        // The caption is a chunk of its own, marked so its page render goes with it.
        assert!(chunks[3].text.starts_with("Fig. 8.17"));
        assert_eq!(chunks[3].element_types, ["caption", "image"]);
        assert_eq!(chunks[1].heading_context.as_deref(), Some("HCl Secretion"));
        assert_eq!(
            chunks[1].full_text,
            "HCl Secretion\n\nA major function of the parietal cells is\nsecretion of HCl."
        );
        assert_eq!(
            elements
                .iter()
                .filter(|element| element.element_type == "title")
                .map(|element| element.text.as_str())
                .collect::<Vec<_>>(),
            ["HCl Secretion", "CELLULAR MECHANISM"]
        );
    }

    #[test]
    fn figure_labels_and_sentences_in_another_font_are_not_headings() {
        let runs = vec![
            run("Lumen Blood", 160.0, 700.0, 7.8, "Labels"),
            run("Gastric parietal cell", 260.0, 700.0, 7.8, "Labels"),
            run(
                "The chief cells secrete pepsinogen.",
                58.0,
                600.0,
                9.7,
                "Body",
            ),
            run(
                "omeprazole is used to treat ulcers",
                58.0,
                588.0,
                9.7,
                "Italic",
            ),
            run("and reduces acid secretion.", 58.0, 576.0, 9.7, "Body"),
        ];
        let page = rebuild_page(&runs).unwrap();
        assert!(page
            .sections
            .iter()
            .all(|section| section.heading.is_none()));
    }

    #[test]
    fn long_sections_split_at_line_ends() {
        let line = "word ".repeat(150);
        let text = [line.trim(), line.trim(), line.trim()].join("\n");
        let parts = split_words(&text, MAX_CHUNK_WORDS);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].lines().count(), 2);
    }
}
