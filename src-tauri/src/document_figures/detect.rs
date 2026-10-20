//! Finding figures in page layouts, with no AI and no I/O.
//!
//! A figure is a picture or a drawing big enough to study. Pictures are image
//! and form objects; drawings are clusters of vector paths that are not a ruled
//! table or a boxed paragraph. Anything that repeats in the same place on
//! several pages, or sits in a page's header or footer band, is page furniture.
//! A caption is the run of lines starting "Figure 3.2" (or "Fig.", "Diagram",
//! "Chart", ...) just below or above a figure. A figure's passages are the
//! places elsewhere that name its label, and, for a figure that fills its page,
//! the pages on either side, where the explanation usually is.

use std::collections::HashMap;

use crate::pdf_render_backend::{PageGraphicKind, PageLayout, PageTextRun};

/// Names this detector in stored scans, so a better one can find figures again.
pub(crate) const DETECTOR: &str = "pdfium-objects-v1";

pub(crate) const MAX_FIGURES_PER_PAGE: usize = 12;
pub(crate) const MAX_FIGURES: usize = 400;
pub(crate) const MAX_CAPTION_CHARS: usize = 2_000;
pub(crate) const MAX_LABEL_CHARS: usize = 64;
pub(crate) const MAX_EXCERPT_CHARS: usize = 400;
const MAX_MENTIONS_PER_FIGURE: usize = 8;
const MAX_MENTIONS_PER_PAGE: usize = 2;
const MAX_CAPTION_LINES: usize = 8;

/// The smallest share of the page a figure covers, and its shortest side.
const MIN_AREA_SHARE: f64 = 0.012;
const MIN_SIDE: f64 = 28.0;
/// An object covering this much of the page is a background or a scanned page.
const PAGE_COVER_SHARE: f64 = 0.9;
/// A path thinner than this is a rule, as in a table or under a heading.
const RULE_THICKNESS: f64 = 2.5;
/// Paths this close belong to one drawing.
const CLUSTER_GAP: f64 = 4.0;
/// Pictures and drawings this close are one figure.
const MERGE_GAP: f64 = 6.0;
const MIN_DRAWING_PATHS: usize = 6;
/// Header and footer bands, as a share of the page height.
const FURNITURE_BAND: f64 = 0.07;
/// The same box on this many pages is furniture.
const FURNITURE_PAGES: usize = 3;
/// How far a caption may sit from its figure, in points.
const CAPTION_REACH: f64 = 48.0;
/// A figure covering this share of its page, or a page with less body text
/// than this, brings in the pages on either side.
const PAGE_FILLING_SHARE: f64 = 0.45;
const SPARSE_PAGE_CHARS: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub(crate) fn from_bounds([left, top, width, height]: [f64; 4]) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    fn right(&self) -> f64 {
        self.left + self.width
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn union(&self, other: &Rect) -> Rect {
        let left = self.left.min(other.left);
        let top = self.top.min(other.top);
        Rect {
            left,
            top,
            width: self.right().max(other.right()) - left,
            height: self.bottom().max(other.bottom()) - top,
        }
    }

    fn intersection_area(&self, other: &Rect) -> f64 {
        let width = self.right().min(other.right()) - self.left.max(other.left);
        let height = self.bottom().min(other.bottom()) - self.top.max(other.top);
        if width > 0.0 && height > 0.0 {
            width * height
        } else {
            0.0
        }
    }

    /// Whether the two boxes overlap or lie within `gap` points of each other.
    fn near(&self, other: &Rect, gap: f64) -> bool {
        self.left - gap <= other.right()
            && other.left - gap <= self.right()
            && self.top - gap <= other.bottom()
            && other.top - gap <= self.bottom()
    }

    fn contains(&self, other: &Rect, slack: f64) -> bool {
        other.left >= self.left - slack
            && other.top >= self.top - slack
            && other.right() <= self.right() + slack
            && other.bottom() <= self.bottom() + slack
    }

    /// Intersection over union, for telling whether two boxes are one figure.
    pub(crate) fn overlap(&self, other: &Rect) -> f64 {
        let shared = self.intersection_area(other);
        let total = self.area() + other.area() - shared;
        if total > 0.0 {
            shared / total
        } else {
            0.0
        }
    }

    /// Clipped to a page `width` by `height`; `None` when nothing is left.
    fn clipped(&self, width: f64, height: f64) -> Option<Rect> {
        let left = self.left.max(0.0);
        let top = self.top.max(0.0);
        let right = self.right().min(width);
        let bottom = self.bottom().min(height);
        (right > left && bottom > top).then_some(Rect {
            left,
            top,
            width: right - left,
            height: bottom - top,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FigureKind {
    Image,
    Drawing,
}

impl FigureKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Drawing => "drawing",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Caption {
    pub bounds: Rect,
    pub text: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PassageKind {
    Mention,
    Neighbour,
}

impl PassageKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Mention => "mention",
            Self::Neighbour => "neighbour",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Passage {
    pub page_number: usize,
    pub kind: PassageKind,
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DetectedFigure {
    pub figure_id: String,
    pub page_number: usize,
    pub page_width: f64,
    pub page_height: f64,
    pub kind: FigureKind,
    pub bounds: Rect,
    pub caption: Option<Caption>,
    pub passages: Vec<Passage>,
}

/// One page's candidates before furniture is removed across pages: all a
/// search keeps of a page, so a long document never holds every page's
/// graphics at once.
pub(crate) struct PageFigures {
    page_number: usize,
    width: f64,
    height: f64,
    figures: Vec<(FigureKind, Rect)>,
    lines: Vec<Line>,
    text: String,
}

/// Every figure in a document's page layouts, in page and reading order.
#[cfg(test)]
pub(crate) fn detect_layouts(pages: &[PageLayout]) -> Vec<DetectedFigure> {
    detect(pages.iter().map(PageFigures::from_layout).collect())
}

/// Every figure in a document, in page and reading order.
pub(crate) fn detect(mut candidates: Vec<PageFigures>) -> Vec<DetectedFigure> {
    drop_furniture(&mut candidates);

    let mut figures = Vec::new();
    for page in &candidates {
        let mut on_page = page
            .figures
            .iter()
            .map(|(kind, bounds)| (*kind, *bounds, find_caption(*bounds, &page.lines)))
            .collect::<Vec<_>>();
        on_page.sort_by(|a, b| {
            a.1.top
                .total_cmp(&b.1.top)
                .then_with(|| a.1.left.total_cmp(&b.1.left))
        });
        for (index, (kind, bounds, caption)) in
            on_page.into_iter().take(MAX_FIGURES_PER_PAGE).enumerate()
        {
            if figures.len() == MAX_FIGURES {
                break;
            }
            figures.push(DetectedFigure {
                figure_id: format!("p{:05}-{:02}", page.page_number, index + 1),
                page_number: page.page_number,
                page_width: page.width,
                page_height: page.height,
                kind,
                bounds,
                caption,
                passages: Vec::new(),
            });
        }
    }
    link_passages(&mut figures, &candidates);
    figures
}

impl PageFigures {
    pub(crate) fn from_layout(page: &PageLayout) -> PageFigures {
        let page_area = page.width * page.height;
        let big_enough = |rect: &Rect| {
            rect.area() >= page_area * MIN_AREA_SHARE
                && rect.width >= MIN_SIDE
                && rect.height >= MIN_SIDE
                && rect.area() < page_area * PAGE_COVER_SHARE
        };

        let mut figures = Vec::<(FigureKind, Rect)>::new();
        let mut paths = Vec::<Rect>::new();
        for graphic in &page.graphics {
            let Some(rect) = Rect::from_bounds(graphic.bounds).clipped(page.width, page.height)
            else {
                continue;
            };
            match graphic.kind {
                PageGraphicKind::Image | PageGraphicKind::Form => {
                    let kind = if graphic.kind == PageGraphicKind::Image {
                        FigureKind::Image
                    } else {
                        FigureKind::Drawing
                    };
                    if big_enough(&rect) {
                        figures.push((kind, rect));
                    }
                }
                PageGraphicKind::Path | PageGraphicKind::Shading => {
                    if rect.area() < page_area * PAGE_COVER_SHARE {
                        paths.push(rect);
                    }
                }
            }
        }

        let text_boxes = page
            .text_runs
            .iter()
            .map(|run| Rect::from_bounds(run.bounds))
            .collect::<Vec<_>>();
        let mut leftovers = Vec::<Rect>::new();
        for cluster in clusters(&paths, CLUSTER_GAP) {
            let bounds = cluster
                .iter()
                .map(|index| paths[*index])
                .reduce(|a, b| a.union(&b))
                .expect("a cluster has a member");
            let rules = cluster
                .iter()
                .filter(|index| {
                    let path = paths[**index];
                    path.width < RULE_THICKNESS || path.height < RULE_THICKNESS
                })
                .count();
            let text_area = text_boxes
                .iter()
                .map(|text| text.intersection_area(&bounds))
                .sum::<f64>();
            let is_drawing = cluster.len() >= MIN_DRAWING_PATHS
                && (rules as f64) < cluster.len() as f64 * 0.8
                && text_area < bounds.area() * 0.45
                && big_enough(&bounds);
            if is_drawing {
                figures.push((FigureKind::Drawing, bounds));
            } else {
                leftovers.push(bounds);
            }
        }
        // Leader lines and label boxes drawn over a picture's edge belong to
        // it, though they are too few to be a drawing of their own.
        loop {
            let before = leftovers.len();
            leftovers.retain(|rest| {
                let Some((_, figure)) = figures.iter_mut().find(|(_, figure)| {
                    figure.near(rest, MERGE_GAP) && rest.area() <= figure.area()
                }) else {
                    return true;
                };
                *figure = figure.union(rest);
                false
            });
            if leftovers.len() == before {
                break;
            }
        }

        let figures = merge_near(figures)
            .into_iter()
            .filter(|(_, rect)| {
                rect.bottom() > page.height * FURNITURE_BAND
                    && rect.top < page.height * (1.0 - FURNITURE_BAND)
            })
            .collect();
        PageFigures {
            page_number: page.page_number,
            width: page.width,
            height: page.height,
            figures,
            lines: lines(&page.text_runs),
            text: page.text.clone(),
        }
    }
}

/// Groups of indexes whose boxes chain together within `gap`.
fn clusters(rects: &[Rect], gap: f64) -> Vec<Vec<usize>> {
    let mut parent = (0..rects.len()).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
    // Sorted by left edge, a box can only touch boxes that start before its
    // right edge plus the gap, which keeps a page of thousands of paths quick.
    let mut order = (0..rects.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| rects[*a].left.total_cmp(&rects[*b].left));
    for (position, &a) in order.iter().enumerate() {
        for &b in &order[position + 1..] {
            if rects[b].left > rects[a].right() + gap {
                break;
            }
            if rects[a].near(&rects[b], gap) {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }
    let mut groups = HashMap::<usize, Vec<usize>>::new();
    for index in 0..rects.len() {
        let group = root(&mut parent, index);
        groups.entry(group).or_default().push(index);
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort();
    groups
}

/// Joins figures that overlap or nearly touch, until none do.
fn merge_near(mut figures: Vec<(FigureKind, Rect)>) -> Vec<(FigureKind, Rect)> {
    loop {
        let mut merged = false;
        'search: for a in 0..figures.len() {
            for b in a + 1..figures.len() {
                if figures[a].1.near(&figures[b].1, MERGE_GAP) {
                    let (kind_b, rect_b) = figures.remove(b);
                    let (kind_a, rect_a) = figures[a];
                    let kind = if kind_a == FigureKind::Image || kind_b == FigureKind::Image {
                        FigureKind::Image
                    } else {
                        FigureKind::Drawing
                    };
                    figures[a] = (kind, rect_a.union(&rect_b));
                    merged = true;
                    break 'search;
                }
            }
        }
        if !merged {
            return figures;
        }
    }
}

/// Removes figures whose box recurs on several pages, such as logos.
fn drop_furniture(pages: &mut [PageFigures]) {
    let key = |rect: &Rect| {
        [rect.left, rect.top, rect.width, rect.height].map(|value| (value / 4.0).round() as i64)
    };
    let mut counts = HashMap::<[i64; 4], usize>::new();
    for page in pages.iter() {
        let mut seen = page
            .figures
            .iter()
            .map(|(_, rect)| key(rect))
            .collect::<Vec<_>>();
        seen.sort_unstable();
        seen.dedup();
        for value in seen {
            *counts.entry(value).or_default() += 1;
        }
    }
    for page in pages.iter_mut() {
        page.figures
            .retain(|(_, rect)| counts.get(&key(rect)).copied().unwrap_or(0) < FURNITURE_PAGES);
    }
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    bounds: Rect,
}

/// Text runs joined into lines: runs whose vertical middles agree, left to right.
fn lines(runs: &[PageTextRun]) -> Vec<Line> {
    let mut runs = runs
        .iter()
        .map(|run| (run.text.as_str(), Rect::from_bounds(run.bounds)))
        .collect::<Vec<_>>();
    runs.sort_by(|a, b| {
        a.1.top
            .total_cmp(&b.1.top)
            .then_with(|| a.1.left.total_cmp(&b.1.left))
    });
    let mut lines = Vec::<Line>::new();
    for (text, rect) in runs {
        let middle = rect.top + rect.height / 2.0;
        let joined = lines.iter_mut().rev().take(4).find(|line| {
            let line_middle = line.bounds.top + line.bounds.height / 2.0;
            (middle - line_middle).abs() < line.bounds.height.max(rect.height) * 0.4
                && rect.left >= line.bounds.left
                && rect.left - line.bounds.right() < rect.height.max(line.bounds.height) * 2.0
        });
        match joined {
            Some(line) => {
                line.text.push(' ');
                line.text.push_str(text);
                line.bounds = line.bounds.union(&rect);
            }
            None => lines.push(Line {
                text: text.to_string(),
                bounds: rect,
            }),
        }
    }
    lines.sort_by(|a, b| {
        a.bounds
            .top
            .total_cmp(&b.bounds.top)
            .then_with(|| a.bounds.left.total_cmp(&b.bounds.left))
    });
    lines
}

/// Words a caption opens with, longest spelling first.
const CAPTION_WORDS: [&str; 9] = [
    "figure",
    "fig.",
    "fig",
    "diagram",
    "chart",
    "graph",
    "plate",
    "illustration",
    "exhibit",
];

/// A figure number at the start of `text`: digits, then any further parts
/// joined by a dot or dash ("3.2", "3 - 2", "12.4b"). Returns the number with
/// dashes normalized and spaces removed, and the bytes it took.
fn figure_number(text: &str) -> Option<(String, usize)> {
    let digits = |text: &str| -> usize {
        text.char_indices()
            .find(|(_, character)| !character.is_ascii_digit())
            .map(|(index, _)| index)
            .unwrap_or(text.len())
    };
    let first = digits(text);
    if first == 0 {
        return None;
    }
    let mut number = text[..first].to_string();
    let mut consumed = first;
    loop {
        let rest = &text[consumed..];
        let trimmed = rest.trim_start();
        let Some(separator) = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '.' | '-' | '–'))
        else {
            break;
        };
        let after_separator = &trimmed[separator.len_utf8()..];
        let next = after_separator.trim_start();
        let length = digits(next);
        if length == 0 {
            break;
        }
        number.push(if separator == '.' { '.' } else { '-' });
        number.push_str(&next[..length]);
        consumed = text.len() - next.len() + length;
    }
    let rest = &text[consumed..];
    let mut characters = rest.chars();
    if let Some(letter) = characters.next().filter(char::is_ascii_lowercase) {
        if !characters.next().is_some_and(char::is_alphanumeric) {
            number.push(letter);
            consumed += 1;
        }
    }
    if text[consumed..]
        .chars()
        .next()
        .is_some_and(char::is_alphanumeric)
    {
        return None;
    }
    Some((number, consumed))
}

/// The label a line opens with, written the usual way: "Figure 3.2".
fn caption_label(line: &str) -> Option<String> {
    let lowered = line.to_lowercase();
    // Lowercasing keeps these ASCII words and digits at the same byte offsets.
    if lowered.len() != line.len() {
        return None;
    }
    let word = CAPTION_WORDS
        .iter()
        .find(|word| lowered.starts_with(**word))?;
    let rest = &lowered[word.len()..];
    if !word.ends_with('.') && rest.chars().next().is_some_and(char::is_alphabetic) {
        return None;
    }
    let (number, _) = figure_number(rest.trim_start())?;
    let word = word.trim_end_matches('.');
    let word = if word == "fig" {
        "Figure".to_string()
    } else {
        let mut characters = word.chars();
        characters
            .next()
            .map(|first| first.to_uppercase().chain(characters).collect())
            .unwrap_or_default()
    };
    Some(truncate_chars(&format!("{word} {number}"), MAX_LABEL_CHARS))
}

fn find_caption(figure: Rect, lines: &[Line]) -> Option<Caption> {
    let overlaps = |line: &Line| {
        let shared = line.bounds.right().min(figure.right()) - line.bounds.left.max(figure.left);
        shared >= line.bounds.width.min(figure.width) * 0.3
    };
    let mut best: Option<(f64, usize, bool, String)> = None;
    for (index, line) in lines.iter().enumerate() {
        if !overlaps(line) {
            continue;
        }
        let Some(label) = caption_label(line.text.trim()) else {
            continue;
        };
        let below = line.bounds.top - figure.bottom();
        let above = figure.top - line.bounds.bottom();
        let (distance, is_below) = if (-2.0..=CAPTION_REACH).contains(&below) {
            (below.max(0.0), true)
        } else if (-2.0..=CAPTION_REACH).contains(&above) {
            (above.max(0.0), false)
        } else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(nearest, ..)| distance < *nearest)
        {
            best = Some((distance, index, is_below, label));
        }
    }
    let (_, first, is_below, label) = best?;

    // A caption below its figure runs on in the lines beneath it; one above
    // usually is a single line or two, read the same way.
    let mut bounds = lines[first].bounds;
    let mut text = lines[first].text.trim().to_string();
    let mut previous = lines[first].bounds;
    for line in lines[first + 1..].iter().take(MAX_CAPTION_LINES - 1) {
        let gap = line.bounds.top - previous.bottom();
        let aligned = line.bounds.left < bounds.right() && line.bounds.right() > bounds.left;
        if gap > previous.height * 0.8
            || gap < -previous.height * 0.5
            || !aligned
            || caption_label(line.text.trim()).is_some()
            || (!is_below && line.bounds.top >= figure.top)
        {
            break;
        }
        if text.ends_with('-')
            && line
                .text
                .trim_start()
                .starts_with(|character: char| character.is_lowercase())
        {
            text.pop();
        } else {
            text.push(' ');
        }
        text.push_str(line.text.trim());
        bounds = bounds.union(&line.bounds);
        previous = line.bounds;
    }
    let text = truncate_chars(&text, MAX_CAPTION_CHARS);
    Some(Caption {
        bounds,
        text,
        label,
    })
}

fn truncate_chars(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

/// Where running text names `label`: "Figure 3.2", "Fig. 3-2", "figures
/// 3.2", with any dash or spacing in the number. Byte offsets into `text`.
fn mentions(text: &str, label: &str) -> Vec<usize> {
    let Some((word, number)) = label.split_once(' ') else {
        return Vec::new();
    };
    let spellings: Vec<String> = if word == "Figure" {
        ["figures", "figure", "figs.", "figs", "fig.", "fig"]
            .map(str::to_string)
            .to_vec()
    } else {
        let word = word.to_lowercase();
        vec![format!("{word}s"), word]
    };
    let lowered = text.to_lowercase();
    if lowered.len() != text.len() {
        // Offsets would not line up; such text is rare enough to skip.
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(relative) = lowered[from..].find(&spellings[spellings.len() - 1][..3]) {
        let start = from + relative;
        from = start + 1;
        if lowered[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        let Some(spelling) = spellings
            .iter()
            .find(|spelling| lowered[start..].starts_with(spelling.as_str()))
        else {
            continue;
        };
        let rest = &lowered[start + spelling.len()..];
        if !spelling.ends_with('.') && rest.chars().next().is_some_and(char::is_alphabetic) {
            continue;
        }
        if figure_number(rest.trim_start()).is_some_and(|(value, _)| value == number) {
            found.push(start);
        }
    }
    found
}

/// The sentence around `position`, at most [`MAX_EXCERPT_CHARS`] characters.
fn excerpt(text: &str, position: usize) -> String {
    let ends = sentence_ends(text);
    let start = ends
        .iter()
        .copied()
        .rev()
        .find(|&end| end <= position)
        .unwrap_or(0);
    let end = ends
        .iter()
        .copied()
        .find(|&end| end > position)
        .unwrap_or(text.len());
    let sentence = text[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_chars(&sentence, MAX_EXCERPT_CHARS)
}

/// Words a full stop follows without ending the sentence.
const ABBREVIATIONS: [&str; 10] = [
    "fig", "figs", "eq", "eqs", "no", "vs", "cf", "p", "pp", "e.g",
];

/// Byte offsets just past each sentence's last character. A full stop ends
/// a sentence only before space or the end, and not after "Fig." or "3."
/// inside "3.2".
fn sentence_ends(text: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    for (index, character) in text.char_indices() {
        let next = index + character.len_utf8();
        let ends_here = match character {
            '!' | '?' | '\n' => true,
            '.' => {
                let followed_by_space = text[next..].chars().next().is_none_or(char::is_whitespace);
                let word = text[..index]
                    .rsplit(|c: char| c.is_whitespace() || c == '(')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                followed_by_space && !ABBREVIATIONS.contains(&word.as_str())
            }
            _ => false,
        };
        if ends_here {
            ends.push(next);
        }
    }
    ends
}

fn link_passages(figures: &mut [DetectedFigure], pages: &[PageFigures]) {
    let mut label_counts = HashMap::<String, usize>::new();
    for figure in figures.iter() {
        if let Some(caption) = &figure.caption {
            *label_counts.entry(caption.label.clone()).or_default() += 1;
        }
    }
    let page_count = pages.iter().map(|page| page.page_number).max().unwrap_or(0);
    for figure in figures.iter_mut() {
        let mut passages = Vec::new();

        // A label two figures share cannot tell their mentions apart.
        let label = figure
            .caption
            .as_ref()
            .filter(|caption| label_counts.get(&caption.label) == Some(&1))
            .map(|caption| caption.label.clone());
        if let Some(label) = label.as_deref() {
            let caption_opening = figure
                .caption
                .as_ref()
                .map(|caption| truncate_chars(&caption.text, 40))
                .unwrap_or_default();
            for page in pages {
                let mut on_page = 0;
                for position in mentions(&page.text, label) {
                    if on_page == MAX_MENTIONS_PER_PAGE || passages.len() == MAX_MENTIONS_PER_FIGURE
                    {
                        break;
                    }
                    // The caption names its own figure; that is not a mention.
                    let squashed =
                        |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
                    if page.page_number == figure.page_number
                        && squashed(&page.text[position..]).starts_with(&squashed(&caption_opening))
                    {
                        continue;
                    }
                    let sentence = excerpt(&page.text, position);
                    if sentence.is_empty() {
                        continue;
                    }
                    on_page += 1;
                    passages.push(Passage {
                        page_number: page.page_number,
                        kind: PassageKind::Mention,
                        excerpt: Some(sentence),
                    });
                }
            }
        }

        let Some(page) = pages
            .iter()
            .find(|page| page.page_number == figure.page_number)
        else {
            figure.passages = passages;
            continue;
        };
        let body_chars = page
            .lines
            .iter()
            .filter(|line| {
                !figure.bounds.contains(&line.bounds, 2.0)
                    && !figure
                        .caption
                        .as_ref()
                        .is_some_and(|caption| caption.bounds.contains(&line.bounds, 2.0))
            })
            .map(|line| line.text.chars().count())
            .sum::<usize>();
        let fills_page =
            figure.bounds.area() >= figure.page_width * figure.page_height * PAGE_FILLING_SHARE;
        if fills_page || body_chars < SPARSE_PAGE_CHARS {
            for neighbour in [
                figure.page_number.checked_sub(1),
                Some(figure.page_number + 1),
            ]
            .into_iter()
            .flatten()
            .filter(|number| (1..=page_count).contains(number))
            {
                passages.push(Passage {
                    page_number: neighbour,
                    kind: PassageKind::Neighbour,
                    excerpt: None,
                });
            }
        }
        figure.passages = passages;
    }
}
