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
pub(crate) const DETECTOR: &str = "pdfium-objects-v2";

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
/// Characters per thousand square points. A table or shaded box drawn only
/// with level and upright lines holds more text than the first, and boxed
/// prose of any shape more than the second; such a box is a figure only when
/// a figure caption names it, as for a flowchart or a bar chart.
const TABLE_TEXT_DENSITY: f64 = 1.0;
const PROSE_TEXT_DENSITY: f64 = 6.0;
/// The share of a cluster's paths that must be boxy for it to read as a table.
const TABLE_BOXY_SHARE: f64 = 0.9;
/// Header and footer bands, as a share of the page height.
const FURNITURE_BAND: f64 = 0.07;
/// The same box on this many pages is furniture.
const FURNITURE_PAGES: usize = 3;
/// The widest gap, in points, between pieces gathered above a caption.
const GATHER_GAP: f64 = 24.0;
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

    fn contains_centre_of(&self, other: &Rect) -> bool {
        let (x, y) = (
            other.left + other.width / 2.0,
            other.top + other.height / 2.0,
        );
        x >= self.left && x <= self.right() && y >= self.top && y <= self.bottom()
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
    figures: Vec<Candidate>,
    /// Groups of graphics too small or wordy to be figures by themselves,
    /// which a figure caption beneath them can still gather into one.
    pieces: Vec<Rect>,
    lines: Vec<Line>,
    text: String,
}

/// A box on one page that may be a figure.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    kind: FigureKind,
    bounds: Rect,
    /// Text-heavy boxes, such as a flowchart or a table, count only when a
    /// figure caption names them.
    needs_caption: bool,
}

/// Every figure in a document's page layouts, in page and reading order.
#[cfg(test)]
pub(crate) fn detect_layouts(pages: &[PageLayout]) -> Vec<DetectedFigure> {
    detect(pages.iter().map(PageFigures::from_layout).collect())
}

/// Figures drawn as separate boxes, arrows and labels, found from the figure
/// captions on `page` that no figure in `found` claimed: the pieces just
/// above such a caption, and the pieces just above those, are its figure.
fn gathered_figures(
    page: &PageFigures,
    found: &[(FigureKind, Rect, Option<Caption>)],
) -> Vec<(FigureKind, Rect, Option<Caption>)> {
    let claimed = |line: &Line| {
        found.iter().any(|(_, bounds, caption)| {
            bounds.intersection_area(&line.bounds) > 0.0
                || caption
                    .as_ref()
                    .is_some_and(|caption| caption.bounds.intersection_area(&line.bounds) > 0.0)
        })
    };
    let mut gathered = Vec::new();
    for line in &page.lines {
        if caption_label(line.text.trim()).is_none() || claimed(line) {
            continue;
        }
        let (left, right) = (
            line.bounds.left - MERGE_GAP,
            line.bounds.right() + MERGE_GAP,
        );
        let mut region: Option<Rect> = None;
        loop {
            let reach_top = region.map_or(line.bounds.top, |region| region.top);
            let next = page.pieces.iter().find(|piece| {
                piece.right() > left
                    && piece.left < right
                    && piece.bottom() <= line.bounds.top + 2.0
                    && piece.bottom() >= reach_top - GATHER_GAP
                    && region.is_none_or(|region| !region.contains(piece, 0.5))
                    && !found
                        .iter()
                        .any(|(_, bounds, _)| bounds.intersection_area(piece) > 0.0)
            });
            let Some(next) = next else {
                break;
            };
            region = Some(region.map_or(*next, |region| region.union(next)));
        }
        let Some(region) = region else {
            continue;
        };
        let big_enough = region.area() >= page.width * page.height * MIN_AREA_SHARE
            && region.width >= MIN_SIDE
            && region.height >= MIN_SIDE;
        let Some(caption) = find_caption(region, &page.lines) else {
            continue;
        };
        if big_enough && caption.bounds.top == line.bounds.top {
            gathered.push((FigureKind::Drawing, region, Some(caption)));
        }
    }
    gathered
}

/// Every figure in a document, in page and reading order.
pub(crate) fn detect(mut candidates: Vec<PageFigures>) -> Vec<DetectedFigure> {
    drop_furniture(&mut candidates);

    let mut figures = Vec::new();
    for page in &candidates {
        let on_page = page
            .figures
            .iter()
            .map(|candidate| {
                let caption = find_caption(candidate.bounds, &page.lines);
                (candidate, caption)
            })
            .filter(|(candidate, caption)| {
                caption.is_some()
                    || (!candidate.needs_caption && !labelled_table(candidate.bounds, &page.lines))
            })
            .map(|(candidate, caption)| (candidate.kind, candidate.bounds, caption))
            .collect::<Vec<_>>();
        // Panels set apart on the page but named by one caption are one figure.
        let mut joined = Vec::<(FigureKind, Rect, Option<Caption>)>::new();
        for (kind, bounds, caption) in on_page {
            let shared = caption.as_ref().and_then(|caption| {
                joined.iter_mut().find(|(_, _, other)| {
                    other
                        .as_ref()
                        .is_some_and(|other| other.bounds == caption.bounds)
                })
            });
            match shared {
                Some(figure) => {
                    figure.1 = figure.1.union(&bounds);
                    if kind == FigureKind::Image {
                        figure.0 = FigureKind::Image;
                    }
                }
                None => joined.push((kind, bounds, caption)),
            }
        }
        let mut on_page = joined;
        on_page.extend(gathered_figures(page, &on_page));
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

        let mut figures = Vec::<Candidate>::new();
        let mut pieces = Vec::<Rect>::new();
        let mut paths = Vec::<Rect>::new();
        let mut boxy = Vec::<bool>::new();
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
                        figures.push(Candidate {
                            kind,
                            bounds: rect,
                            needs_caption: false,
                        });
                    } else if rect.area() < page_area * PAGE_COVER_SHARE {
                        pieces.push(rect);
                    }
                }
                PageGraphicKind::Path | PageGraphicKind::Shading => {
                    if rect.area() < page_area * PAGE_COVER_SHARE {
                        paths.push(rect);
                        boxy.push(graphic.boxy);
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
            let characters = page
                .text_runs
                .iter()
                .filter(|run| bounds.contains_centre_of(&Rect::from_bounds(run.bounds)))
                .map(|run| run.text.chars().filter(|c| !c.is_whitespace()).count())
                .sum::<usize>();
            let density = characters as f64 * 1_000.0 / bounds.area();
            let boxy_share =
                cluster.iter().filter(|index| boxy[**index]).count() as f64 / cluster.len() as f64;
            let is_text_block = density >= PROSE_TEXT_DENSITY
                || (density >= TABLE_TEXT_DENSITY && boxy_share >= TABLE_BOXY_SHARE);
            let is_drawing = cluster.len() >= MIN_DRAWING_PATHS
                && (rules as f64) < cluster.len() as f64 * 0.8
                && text_area < bounds.area() * 0.45
                && big_enough(&bounds);
            if is_drawing {
                figures.push(Candidate {
                    kind: FigureKind::Drawing,
                    bounds,
                    needs_caption: is_text_block,
                });
            } else if !is_text_block {
                leftovers.push(bounds);
            } else {
                pieces.push(bounds);
            }
        }
        // Leader lines and label boxes drawn over a picture's edge belong to
        // it, though they are too few to be a drawing of their own.
        loop {
            let before = leftovers.len();
            leftovers.retain(|rest| {
                let Some(figure) = figures.iter_mut().find(|figure| {
                    figure.bounds.near(rest, MERGE_GAP) && rest.area() <= figure.bounds.area()
                }) else {
                    return true;
                };
                figure.bounds = figure.bounds.union(rest);
                false
            });
            if leftovers.len() == before {
                break;
            }
        }
        pieces.extend(leftovers);

        let figures = merge_near(figures)
            .into_iter()
            .filter(|candidate| {
                candidate.bounds.bottom() > page.height * FURNITURE_BAND
                    && candidate.bounds.top < page.height * (1.0 - FURNITURE_BAND)
            })
            .collect();
        PageFigures {
            page_number: page.page_number,
            width: page.width,
            height: page.height,
            figures,
            pieces,
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
fn merge_near(mut figures: Vec<Candidate>) -> Vec<Candidate> {
    loop {
        let mut merged = false;
        'search: for a in 0..figures.len() {
            for b in a + 1..figures.len() {
                if figures[a].bounds.near(&figures[b].bounds, MERGE_GAP) {
                    let other = figures.remove(b);
                    let first = figures[a];
                    let kind = if first.kind == FigureKind::Image || other.kind == FigureKind::Image
                    {
                        FigureKind::Image
                    } else {
                        FigureKind::Drawing
                    };
                    figures[a] = Candidate {
                        kind,
                        bounds: first.bounds.union(&other.bounds),
                        needs_caption: first.needs_caption && other.needs_caption,
                    };
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
            .map(|candidate| key(&candidate.bounds))
            .collect::<Vec<_>>();
        seen.sort_unstable();
        seen.dedup();
        for value in seen {
            *counts.entry(value).or_default() += 1;
        }
    }
    for page in pages.iter_mut() {
        page.figures.retain(|candidate| {
            counts.get(&key(&candidate.bounds)).copied().unwrap_or(0) < FURNITURE_PAGES
        });
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
        // A run can sit a fraction higher than the run before it on the same
        // line, as a bold "Fig. 1.2" does beside its caption, so it may join
        // a line on either side.
        let joined = lines.iter_mut().rev().take(4).find(|line| {
            let line_middle = line.bounds.top + line.bounds.height / 2.0;
            let reach = rect.height.max(line.bounds.height) * 2.0;
            (middle - line_middle).abs() < line.bounds.height.max(rect.height) * 0.4
                && if rect.left >= line.bounds.left {
                    let gap = rect.left - line.bounds.right();
                    // A caption across a column gutter starts its own line.
                    gap < reach && !(gap > rect.height && caption_label(text).is_some())
                } else {
                    // Only a run snug against the line, never one across a gutter.
                    line.bounds.left - rect.right() < rect.height.max(line.bounds.height)
                        && rect.right() <= line.bounds.left + 1.0
                }
        });
        match joined {
            Some(line) if rect.left < line.bounds.left => {
                line.text = format!("{text} {}", line.text);
                line.bounds = line.bounds.union(&rect);
            }
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
/// `line` without the bullet, square or rule some books set before a caption.
fn without_leading_marks(line: &str) -> &str {
    line.trim_start_matches(|character: char| !character.is_alphanumeric())
}

fn caption_label(line: &str) -> Option<String> {
    let line = without_leading_marks(line);
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
    let rest = rest.trim_start();
    let (number, consumed) = figure_number(rest)?;
    // "Fig. 8.18)" or "Fig. 8.18," closes a reference in running text.
    if rest[consumed..]
        .trim_start()
        .starts_with([')', ',', ';', ']'])
    {
        return None;
    }
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

/// Whether a "Table 6.1" label sits just above `bounds`: a ruled table,
/// not a figure.
fn labelled_table(bounds: Rect, lines: &[Line]) -> bool {
    lines.iter().any(|line| {
        let above = bounds.top - line.bounds.bottom();
        (-2.0..=CAPTION_REACH).contains(&above)
            && line.bounds.left < bounds.right()
            && line.bounds.right() > bounds.left
            && without_leading_marks(line.text.trim())
                .to_lowercase()
                .strip_prefix("table")
                .and_then(|rest| figure_number(rest.trim_start()))
                .is_some()
    })
}

fn find_caption(figure: Rect, lines: &[Line]) -> Option<Caption> {
    let overlaps = |line: &Line| {
        let shared = line.bounds.right().min(figure.right()) - line.bounds.left.max(figure.left);
        shared >= line.bounds.width.min(figure.width) * 0.3
    };
    let mut best: Option<((u8, f64), usize, bool, String)> = None;
    for (index, line) in lines.iter().enumerate() {
        if !overlaps(line) {
            continue;
        }
        let Some(label) = caption_label(line.text.trim()) else {
            continue;
        };
        let below = line.bounds.top - figure.bottom();
        let above = figure.top - line.bounds.bottom();
        // A caption beneath a figure comes first. Some books frame a figure
        // and its caption together, so one in the lower half of the frame
        // comes next. One above the figure comes last, since it often
        // belongs to the figure above. Within each, the nearest wins.
        let inside = figure.contains(&line.bounds, 2.0)
            && line.bounds.top >= figure.top + figure.height / 2.0;
        let (rank, is_below) = if (-2.0..=CAPTION_REACH).contains(&below) {
            ((0, below.max(0.0)), true)
        } else if inside {
            ((1, figure.bottom() - line.bounds.bottom()), true)
        } else if (-2.0..=CAPTION_REACH).contains(&above) {
            ((2, above.max(0.0)), false)
        } else {
            continue;
        };
        if best.as_ref().is_none_or(|(nearest, ..)| {
            rank.0 < nearest.0 || (rank.0 == nearest.0 && rank.1 < nearest.1)
        }) {
            best = Some((rank, index, is_below, label));
        }
    }
    let (_, first, is_below, label) = best?;

    // A caption below its figure runs on in the lines beneath it; one above
    // usually is a single line or two, read the same way.
    let mut bounds = lines[first].bounds;
    let mut text = without_leading_marks(lines[first].text.trim()).to_string();
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
    let text = text
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
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
