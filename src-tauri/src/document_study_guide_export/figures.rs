//! The source figures a document guide shows beside its text.
//!
//! A guide shows only the figures that best illustrate its key ideas, never
//! every image: at most two per section and each figure once. The choice is
//! made here from what is already known of each figure, so laying out a guide
//! never sends a page image to a model. A figure qualifies for a section when
//! it sits on one of the section's source pages or the text there explains
//! it, and then:
//!
//! - a figure that was read qualifies when a learner needs it (importance 4
//!   or 5), or when it is a helpful illustration (3) of words the section
//!   itself uses; one read as incidental or not worth studying never shows;
//! - a figure not read (no model that sees images, or reading has not run)
//!   qualifies only when it is a numbered figure whose caption shares at
//!   least two words with the section, and then one per section at most;
//!   when its caption cannot be read (a scanned book whose text layer prints
//!   other letters), it qualifies on the section's own pages by its number.
//!
//! Hidden figures never show. The figure is drawn from the PDF when the guide
//! is laid out, as a sharp crop of its page, so nothing new is stored.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    database::Database,
    document_annotations::page_tiles::pdf_source_path,
    document_figures::{self, DocumentFigureDto},
    document_study_guides::{DocumentStudyGuideSection, StudyGuideSectionContent},
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
};

use super::{guide_typst, ExportFigure};

/// More would crowd a section's page; Jay asked for one to three per page.
const MAX_PER_SECTION: usize = 2;
const MAX_UNREAD_PER_SECTION: usize = 1;
const MIN_IMPORTANCE: i64 = 3;
/// A figure this important shows wherever it belongs, words in common or not.
const NEEDED_IMPORTANCE: i64 = 4;
/// Ranks an unread figure with a helpful read one; it needs this many words
/// in common with the section.
const UNREAD_IMPORTANCE: i64 = 3;
const UNREAD_MIN_SHARED_WORDS: usize = 2;
/// A read figure's caption says why it matters, or else what it shows.
const MAX_SHOWS_CHARS: usize = 220;
const MAX_USE_CHARS: usize = 320;
/// A caption with at least this share of its words mixing other letters into
/// plain ones ("Sites îf acêiîñ") is a broken text layer, not words to match.
const GARBLED_WORD_SHARE: f64 = 0.5;

/// A figure chosen for one section, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChosenFigure {
    pub figure_id: String,
    pub page_number: i64,
    pub title: String,
    pub caption: String,
}

/// What the choice knows of one figure, from its reading or its caption.
struct Candidate {
    read: bool,
    importance: i64,
    /// Phrases matched against the section's words.
    concepts: Vec<String>,
    /// Pages whose text explains or names it.
    explained_on: Vec<i64>,
    title: String,
    shows: String,
    /// An unread figure whose caption is not readable text.
    unreadable: bool,
}

fn candidate(figure: &DocumentFigureDto) -> Option<Candidate> {
    if figure.hidden {
        return None;
    }
    if let Some(reading) = figure.reading.as_ref() {
        if !reading.worth_studying || reading.importance < MIN_IMPORTANCE {
            return None;
        }
        return Some(Candidate {
            read: true,
            importance: reading.importance,
            concepts: reading
                .concepts
                .iter()
                .chain(std::iter::once(&reading.title))
                .cloned()
                .collect(),
            explained_on: reading.explained_on.clone(),
            title: reading.title.trim().to_string(),
            // Why it matters says what a learner needs; a plain description
            // of what it shows stands in when the reading has no such note.
            shows: reading
                .use_in_text
                .as_deref()
                .filter(|text| !about_the_document(text))
                .map(|text| clipped(&about_the_subject(text), MAX_USE_CHARS))
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| clipped(&reading.shows, MAX_SHOWS_CHARS)),
            unreadable: false,
        });
    }
    // Unread: only a numbered figure with a caption says what it is.
    let label = figure.label.as_ref()?;
    let caption = strip_label(figure.caption.as_deref()?.trim());
    if caption.is_empty() {
        return None;
    }
    if garbled(caption) {
        return Some(Candidate {
            read: false,
            importance: UNREAD_IMPORTANCE,
            concepts: Vec::new(),
            explained_on: Vec::new(),
            title: format!("{label} in the source"),
            shows: String::new(),
            unreadable: true,
        });
    }
    let (title, rest) = match caption.find(". ") {
        Some(end) => (&caption[..end], caption[end + 2..].trim()),
        None => (caption.trim_end_matches('.'), ""),
    };
    Some(Candidate {
        read: false,
        importance: UNREAD_IMPORTANCE,
        concepts: vec![caption.to_string()],
        explained_on: figure
            .passages
            .iter()
            .map(|passage| passage.page_number)
            .collect(),
        title: title.trim().to_string(),
        shows: clipped(&caption_detail(rest), MAX_SHOWS_CHARS),
        unreadable: false,
    })
}

/// Whether many of `text`'s words mix letters outside ASCII into ASCII ones,
/// as a broken font map prints them.
fn garbled(text: &str) -> bool {
    let words = text
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect::<Vec<_>>();
    let mixed = words
        .iter()
        .filter(|word| {
            word.chars().any(|c| c.is_ascii_alphabetic())
                && word.chars().any(|c| c.is_alphabetic() && !c.is_ascii())
        })
        .count();
    !words.is_empty() && mixed as f64 >= words.len() as f64 * GARBLED_WORD_SHARE
}

/// The caption's sentences after its title, up to the abbreviation key
/// ("ACh, Acetylcholine; …") or a fragment the caption search cut short.
fn caption_detail(rest: &str) -> String {
    let mut kept = Vec::new();
    for sentence in rest.split(". ") {
        let sentence = sentence.trim().trim_end_matches('.');
        let key = sentence.split_once(", ").is_some_and(|(term, _)| {
            !term.contains(' ')
                && term.chars().any(|c| c.is_ascii_uppercase())
                && term.chars().skip(1).any(|c| c.is_ascii_uppercase())
        });
        if sentence.is_empty() || key || sentence.ends_with([',', ';', ':']) {
            break;
        }
        kept.push(sentence);
    }
    kept.join(". ")
}

/// "Fig. 8.12 Mechanism of …" reads "Mechanism of …".
fn strip_label(caption: &str) -> &str {
    let mut words = caption.splitn(3, ' ');
    match (words.next(), words.next(), words.next()) {
        (Some(first), Some(number), Some(rest))
            if matches!(first.to_lowercase().trim_end_matches('.'), "fig" | "figure")
                && number.chars().next().is_some_and(|c| c.is_ascii_digit()) =>
        {
            rest.trim()
        }
        _ => caption,
    }
}

/// Each section's figures, by topic id, in the source's order.
pub(super) fn choose(
    sections: &[DocumentStudyGuideSection],
    figures: &[DocumentFigureDto],
) -> HashMap<String, Vec<ChosenFigure>> {
    let places = sections
        .iter()
        .map(|section| FigurePlace {
            pages: section
                .source_locator
                .unit_numbers
                .iter()
                .copied()
                .collect(),
            text: section_text(&section.content),
        })
        .collect::<Vec<_>>();
    sections
        .iter()
        .zip(choose_for(&places, figures, MAX_PER_SECTION))
        .filter(|(_, chosen)| !chosen.is_empty())
        .map(|(section, chosen)| (section.topic_id.clone(), chosen))
        .collect()
}

/// Somewhere a figure can show: the source pages it was written from and
/// its words.
pub(crate) struct FigurePlace {
    pub pages: HashSet<i64>,
    pub text: String,
}

/// Up to `per_place` figures for each place, each figure used once, by the
/// rules above, in the source's order.
pub(crate) fn choose_for(
    places: &[FigurePlace],
    figures: &[DocumentFigureDto],
    per_place: usize,
) -> Vec<Vec<ChosenFigure>> {
    let candidates = figures.iter().map(candidate).collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (section_index, place) in places.iter().enumerate() {
        let pages = &place.pages;
        let words = significant_words(&place.text);
        for (figure_index, (figure, candidate)) in figures.iter().zip(&candidates).enumerate() {
            let Some(candidate) = candidate else {
                continue;
            };
            let on_page = pages.contains(&figure.page_number);
            if !on_page
                && !candidate
                    .explained_on
                    .iter()
                    .any(|page| pages.contains(page))
            {
                continue;
            }
            let shared = if candidate.unreadable {
                // Its number is all there is to go on: it shows on its own page.
                if !on_page {
                    continue;
                }
                0
            } else if candidate.read {
                candidate
                    .concepts
                    .iter()
                    .filter(|concept| {
                        significant_words(concept)
                            .iter()
                            .any(|word| words.contains(word))
                    })
                    .count()
            } else {
                let shared = candidate
                    .concepts
                    .iter()
                    .flat_map(|concept| significant_words(concept))
                    .filter(|word| words.contains(word))
                    .collect::<HashSet<_>>()
                    .len();
                if shared < UNREAD_MIN_SHARED_WORDS {
                    continue;
                }
                shared
            };
            if candidate.importance < NEEDED_IMPORTANCE && shared == 0 && !candidate.unreadable {
                continue;
            }
            let score = candidate.importance * 10
                + (shared.min(4) as i64) * 3
                + i64::from(on_page) * 2
                + i64::from(candidate.read);
            pairs.push((score, section_index, figure_index));
        }
    }
    // Best pairs first; a tie keeps the section and figure order.
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut used = HashSet::new();
    let mut per_section = vec![Vec::<usize>::new(); places.len()];
    for (_, section_index, figure_index) in pairs {
        let chosen = &per_section[section_index];
        let unread = chosen
            .iter()
            .filter(|index| candidates[**index].as_ref().is_some_and(|c| !c.read))
            .count();
        let is_unread = candidates[figure_index].as_ref().is_some_and(|c| !c.read);
        if chosen.len() == per_place
            || (is_unread && unread == MAX_UNREAD_PER_SECTION)
            || used.contains(&figure_index)
        {
            continue;
        }
        used.insert(figure_index);
        per_section[section_index].push(figure_index);
    }

    per_section
        .into_iter()
        .map(|mut chosen| {
            // In the order the source prints them.
            chosen.sort_by_key(|index| {
                let figure = &figures[*index];
                (figure.page_number, (figure.bounds.top * 10.0) as i64)
            });
            chosen
                .into_iter()
                .filter_map(|index| {
                    let figure = &figures[index];
                    let candidate = candidates[index].as_ref()?;
                    Some(ChosenFigure {
                        figure_id: figure.figure_id.clone(),
                        page_number: figure.page_number,
                        title: candidate.title.clone(),
                        caption: caption(&candidate.shows, figure.page_number),
                    })
                })
                .collect()
        })
        .collect()
}

/// A document's figures and the PDF they are drawn from, with each
/// section's choice.
pub(super) struct SourceFigures<'a> {
    runtime: &'a PdfRenderRuntime,
    source: PathBuf,
    figures: Vec<DocumentFigureDto>,
    chosen: HashMap<String, Vec<ChosenFigure>>,
}

impl SourceFigures<'_> {
    /// Nothing when the document is not a PDF, its images are off, or none of
    /// its figures has been read: the guide then shows its text alone.
    pub(super) fn load<'a>(
        database: &Database,
        paths: &AppPaths,
        runtime: &'a PdfRenderRuntime,
        document_id: &str,
        sections: &[DocumentStudyGuideSection],
    ) -> Option<SourceFigures<'a>> {
        let (relative, found) = database
            .with_connection(|connection| {
                Ok((
                    pdf_source_path(connection, document_id)?,
                    document_figures::figures(connection, document_id)?,
                ))
            })
            .ok()?;
        if !found.use_images {
            return None;
        }
        let chosen = choose(sections, &found.figures);
        (!chosen.is_empty()).then(|| SourceFigures {
            runtime,
            source: paths.resolve_relative(&relative),
            figures: found.figures,
            chosen,
        })
    }

    /// The section's figures, drawn from the PDF.
    pub(super) fn for_section(&self, topic_id: &str) -> Vec<ExportFigure> {
        self.chosen.get(topic_id).map_or_else(Vec::new, |chosen| {
            draw(self.runtime, &self.source, &self.figures, chosen)
        })
    }
}

/// Draws chosen figures from the PDF; one that cannot be drawn is left out.
fn draw(
    runtime: &PdfRenderRuntime,
    source: &Path,
    figures: &[DocumentFigureDto],
    chosen: &[ChosenFigure],
) -> Vec<ExportFigure> {
    chosen
        .iter()
        .filter_map(|choice| {
            let figure = figures
                .iter()
                .find(|figure| figure.figure_id == choice.figure_id)?;
            // The guide writes its own caption under the figure.
            let pixels = document_figures::segment::figure_image(runtime, source, figure, false)?;
            Some(ExportFigure {
                image: guide_typst::FigureImage::from_rgba(pixels)?,
                title: choice.title.clone(),
                caption: choice.caption.clone(),
            })
        })
        .collect()
}

/// One figure cropped from its PDF page as PNG bytes, with its width and
/// height in pixels; nothing when it cannot be drawn.
pub(crate) fn draw_png(
    runtime: &PdfRenderRuntime,
    source: &Path,
    figure: &DocumentFigureDto,
) -> Option<(Vec<u8>, u32, u32)> {
    let pixels = document_figures::segment::figure_image(runtime, source, figure, false)?;
    let (width, height) = pixels.dimensions();
    let mut encoded = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut encoded, image::ImageFormat::Png)
        .ok()?;
    Some((encoded.into_inner(), width, height))
}

/// Whether a reading's note on the figure talks about the document ("the
/// page text cites it…") rather than the subject; readings made before the
/// note was asked for in the learner's terms often do, and say nothing a
/// caption needs.
fn about_the_document(text: &str) -> bool {
    let lowered = text.to_lowercase();
    ["page", "caption", "does not discuss", "the text "]
        .iter()
        .any(|phrase| lowered.contains(phrase))
}

/// A note that opens by talking about the figure or the learner ("It lets
/// the learner see that …", "It shows how …"), from its first words about
/// the subject itself, which readings made before the caption was asked
/// for in the subject's terms often do.
fn about_the_subject(text: &str) -> String {
    const OPENINGS: [&str; 8] = [
        "it lets the learner see ",
        "it lets the learner ",
        "it helps the learner see ",
        "it helps the learner ",
        "this figure shows ",
        "the figure shows ",
        "it shows ",
        "it illustrates ",
    ];
    let text = text.trim();
    let lowered = text.to_lowercase();
    let Some(opening) = OPENINGS
        .iter()
        .find(|opening| lowered.starts_with(**opening))
    else {
        return text.to_string();
    };
    let mut rest = &text[opening.len()..];
    if rest.to_lowercase().starts_with("that ") {
        rest = &rest[5..];
    }
    let mut letters = rest.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => text.to_string(),
    }
}

/// What the figure shows and why it matters, and where it is from.
fn caption(shows: &str, page_number: i64) -> String {
    let text = shows.trim().trim_end_matches(['.', ' ']);
    if text.is_empty() {
        format!("Source page {page_number}")
    } else if text.ends_with('…') {
        format!("{text} Source page {page_number}")
    } else {
        format!("{text}. Source page {page_number}")
    }
}

/// `text` cut at a sentence when it is longer than `limit` characters,
/// without its final stop.
fn clipped(text: &str, limit: usize) -> String {
    let text = text.trim();
    let mut kept = if text.chars().count() <= limit {
        text.to_string()
    } else {
        let cut = text.chars().take(limit).collect::<String>();
        match cut.rfind(". ") {
            Some(end) if end > limit / 3 => cut[..=end].to_string(),
            // No sentence ends early enough: stop at a word.
            _ => match cut.rfind(' ') {
                Some(space) => format!("{}…", cut[..space].trim_end_matches([',', ';', ':'])),
                None => format!("{}…", cut.trim_end()),
            },
        }
    };
    let end = kept.trim_end_matches(['.', ' ']).len();
    kept.truncate(end);
    kept
}

fn section_text(content: &StudyGuideSectionContent) -> String {
    let mut text = content.overview.clone();
    for line in content
        .key_points
        .iter()
        .chain(&content.process_steps)
        .chain(&content.common_misconceptions)
    {
        text.push(' ');
        text.push_str(line);
    }
    for explanation in &content.explanations {
        text.push(' ');
        text.push_str(&explanation.heading);
        text.push(' ');
        text.push_str(&explanation.body);
    }
    for term in &content.important_terms {
        text.push(' ');
        text.push_str(&term.term);
    }
    text
}

/// Lowercase words of four or more letters, with a plural's final s dropped,
/// so "villi" and "Villi", or "receptor" and "receptors", meet.
fn significant_words(text: &str) -> HashSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.chars().count() >= 4)
        .map(|word| {
            let word = word.to_lowercase();
            match word.strip_suffix('s') {
                Some(stem) if stem.chars().count() >= 4 && !word.ends_with("ss") => {
                    stem.to_string()
                }
                _ => word,
            }
        })
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .collect()
}

const STOP_WORDS: &[&str] = &[
    "about",
    "after",
    "also",
    "between",
    "both",
    "each",
    "from",
    "into",
    "more",
    "most",
    "other",
    "show",
    "shown",
    "such",
    "than",
    "that",
    "their",
    "them",
    "then",
    "there",
    "these",
    "they",
    "this",
    "through",
    "when",
    "where",
    "which",
    "while",
    "with",
    "within",
    "figure",
    "diagram",
    "image",
    "illustration",
    "panel",
];

#[cfg(test)]
#[path = "figures_tests.rs"]
mod tests;
