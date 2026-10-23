//! The study guide as the reader's canvas: every page the PDF export lays out,
//! with where each chapter, section and Practice button landed, and a stable
//! key for each page that marks are saved under.
//!
//! A page's key names the part of the guide it belongs to and its place in
//! that part: `front:1` is the cover, `chapter:<id>:2` a chapter's second
//! page, `cram:1` and `test:1` the cram sheet and self-test. Chapters start a
//! page, so remaking a section moves only its own chapter's pages.

use serde::Serialize;

use super::{
    guide_typst::{self, CanvasAnchor, CanvasPage, StudyGuidePdfTheme},
    reading_order, EffectiveGuideExport,
};
use crate::error::CommandError;

/// The room a section heading keeps for its Practice button, in points; the
/// template's `practice-width` and `practice-height`.
const PRACTICE_WIDTH: f64 = 24.0 * 72.0 / 25.4;
const PRACTICE_HEIGHT: f64 = 6.5 * 72.0 / 25.4;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideCanvasDto {
    pub document_id: String,
    pub guide_revision: i64,
    pub theme: StudyGuidePdfTheme,
    pub pages: Vec<StudyGuideCanvasPageDto>,
    /// Chapters, sections, the cram sheet and the self-test, in page order.
    pub contents: Vec<StudyGuideCanvasEntryDto>,
    /// Where each section's Practice button goes.
    pub practice: Vec<StudyGuideCanvasButtonDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideCanvasPageDto {
    pub key: String,
    pub width: f64,
    pub height: f64,
    pub svg: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StudyGuideCanvasEntryKind {
    Chapter,
    Section,
    Cram,
    SelfTest,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideCanvasEntryDto {
    pub kind: StudyGuideCanvasEntryKind,
    /// The section's topic id, the chapter's id, or the part's name.
    pub key: String,
    pub title: String,
    /// The number the page prints beside the title, such as "2.1".
    pub number: String,
    /// The 0-based page the entry starts on.
    pub page_index: usize,
    /// Points from the top of that page.
    pub y: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideCanvasButtonDto {
    pub topic_id: String,
    pub page_index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Lays out `export` for the canvas.
pub(super) fn guide_canvas(
    document_id: &str,
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
) -> Result<StudyGuideCanvasDto, CommandError> {
    let (pages, anchors) = guide_typst::canvas(export, theme)?;
    Ok(assemble(document_id, export, theme, pages, anchors))
}

fn assemble(
    document_id: &str,
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
    pages: Vec<CanvasPage>,
    mut anchors: Vec<CanvasAnchor>,
) -> StudyGuideCanvasDto {
    anchors.retain(|anchor| anchor.page >= 1 && anchor.page <= pages.len());
    anchors.sort_by(|a, b| (a.page, a.y).partial_cmp(&(b.page, b.y)).unwrap_or(std::cmp::Ordering::Equal));
    let keys = page_keys(pages.len(), &anchors, export.chapters.is_empty());
    let numbers = section_numbers(export);
    let contents = anchors
        .iter()
        .filter_map(|anchor| {
            let (kind, title, number) = match anchor.kind.as_str() {
                "chapter" => {
                    let (index, chapter) = export
                        .chapters
                        .iter()
                        .enumerate()
                        .find(|(_, chapter)| chapter.key == anchor.key)?;
                    (StudyGuideCanvasEntryKind::Chapter, chapter.title.clone(), (index + 1).to_string())
                }
                "section" => {
                    let section = export.sections.iter().find(|section| section.key == anchor.key)?;
                    let number = numbers
                        .iter()
                        .find(|(key, _)| *key == anchor.key)
                        .map(|(_, number)| number.clone())
                        .unwrap_or_default();
                    (StudyGuideCanvasEntryKind::Section, section.title.clone(), number)
                }
                "cram" => (StudyGuideCanvasEntryKind::Cram, "Cram sheet".to_string(), String::new()),
                "selfTest" => (StudyGuideCanvasEntryKind::SelfTest, "Self-test".to_string(), String::new()),
                _ => return None,
            };
            Some(StudyGuideCanvasEntryDto {
                kind,
                key: anchor.key.clone(),
                title,
                number,
                page_index: anchor.page - 1,
                y: anchor.y,
            })
        })
        .collect();
    let practice = anchors
        .iter()
        .filter(|anchor| anchor.kind == "practice")
        .map(|anchor| StudyGuideCanvasButtonDto {
            topic_id: anchor.key.clone(),
            page_index: anchor.page - 1,
            x: anchor.x,
            y: anchor.y,
            width: PRACTICE_WIDTH,
            height: PRACTICE_HEIGHT,
        })
        .collect();
    StudyGuideCanvasDto {
        document_id: document_id.to_string(),
        guide_revision: export.guide_revision,
        theme,
        pages: pages
            .into_iter()
            .zip(keys)
            .map(|(page, key)| StudyGuideCanvasPageDto {
                key,
                width: page.width,
                height: page.height,
                svg: page.svg,
            })
            .collect(),
        contents,
        practice,
    }
}

/// Each page's key: the part that starts on or before it, and its place in
/// that part. A guide without chapters is one `body` part from its first
/// section.
fn page_keys(page_count: usize, anchors: &[CanvasAnchor], flowing: bool) -> Vec<String> {
    let mut starts = Vec::<(usize, String)>::new();
    for anchor in anchors {
        let part = match anchor.kind.as_str() {
            "chapter" => format!("chapter:{}", anchor.key),
            "section" if flowing => "body".to_string(),
            "cram" => "cram".to_string(),
            "selfTest" => "test".to_string(),
            _ => continue,
        };
        if starts.iter().any(|(_, known)| *known == part) {
            continue;
        }
        // Two parts starting on one page share it; the first names it.
        if starts.last().is_some_and(|(page, _)| *page == anchor.page) {
            continue;
        }
        starts.push((anchor.page, part));
    }
    (1..=page_count)
        .map(|page| {
            let (start, part) = starts
                .iter()
                .rev()
                .find(|(start, _)| *start <= page)
                .map(|(start, part)| (*start, part.as_str()))
                .unwrap_or((1, "front"));
            format!("{part}:{}", page - start + 1)
        })
        .collect()
}

/// The number the template prints for each section, by topic id.
fn section_numbers(export: &EffectiveGuideExport) -> Vec<(String, String)> {
    if export.chapters.is_empty() {
        return reading_order(export)
            .into_iter()
            .enumerate()
            .map(|(position, index)| (export.sections[index].key.clone(), (position + 1).to_string()))
            .collect();
    }
    export
        .chapters
        .iter()
        .enumerate()
        .flat_map(|(chapter, entry)| {
            entry.section_indices.iter().enumerate().map(move |(position, index)| {
                (index, format!("{}.{}", chapter + 1, position + 1))
            })
        })
        .filter_map(|(index, number)| export.sections.get(*index).map(|section| (section.key.clone(), number)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(kind: &str, key: &str, page: usize, y: f64) -> CanvasAnchor {
        CanvasAnchor {
            kind: kind.to_string(),
            key: key.to_string(),
            page,
            x: 0.0,
            y,
        }
    }

    #[test]
    fn pages_are_named_by_the_part_they_belong_to() {
        let anchors = [
            anchor("chapter", "c1", 2, 60.0),
            anchor("section", "t1", 2, 120.0),
            anchor("chapter", "c2", 4, 60.0),
            anchor("cram", "cram", 5, 60.0),
            anchor("selfTest", "selfTest", 6, 60.0),
        ];
        assert_eq!(
            page_keys(6, &anchors, false),
            ["front:1", "chapter:c1:1", "chapter:c1:2", "chapter:c2:1", "cram:1", "test:1"]
        );
    }

    #[test]
    fn the_whole_guide_lays_out_with_contents_and_practice_buttons() {
        let mut export = crate::document_study_guide_export::tests::test_export();
        let mut second = export.sections[0].clone();
        second.key = "topic-afterload".into();
        second.title = "Afterload".into();
        export.sections.push(second);
        export.chapters = vec![super::super::EffectiveExportChapter {
            key: "chapter-loads".into(),
            title: "Loads".into(),
            introduction: "Both loads shape stroke volume.".into(),
            section_indices: vec![0, 1],
        }];
        let canvas = guide_canvas("document-1", &export, StudyGuidePdfTheme::Sage).unwrap();
        assert!(canvas.pages.len() >= 2);
        assert_eq!(canvas.pages[0].key, "front:1");
        assert!(canvas.pages.iter().all(|page| page.svg.starts_with("<svg")));
        assert!(canvas.pages[1..]
            .iter()
            .any(|page| page.key == "chapter:chapter-loads:1"));
        let chapter = canvas
            .contents
            .iter()
            .find(|entry| entry.kind == StudyGuideCanvasEntryKind::Chapter)
            .unwrap();
        assert_eq!((chapter.title.as_str(), chapter.number.as_str()), ("Loads", "1"));
        let sections: Vec<_> = canvas
            .contents
            .iter()
            .filter(|entry| entry.kind == StudyGuideCanvasEntryKind::Section)
            .map(|entry| (entry.title.as_str(), entry.number.as_str()))
            .collect();
        assert_eq!(sections, [(export.sections[0].title.as_str(), "1.1"), ("Afterload", "1.2")]);
        assert!(canvas.contents.iter().all(|entry| entry.page_index >= chapter.page_index));
        assert_eq!(canvas.practice.len(), 2);
        for button in &canvas.practice {
            let page = &canvas.pages[button.page_index];
            assert!(button.x > page.width / 2.0 && button.x + button.width <= page.width);
            assert!(button.y > 0.0 && button.y + button.height <= page.height);
        }
    }

    #[test]
    fn a_guide_without_chapters_flows_from_its_first_section() {
        let anchors = [
            anchor("section", "t1", 2, 60.0),
            anchor("section", "t2", 2, 400.0),
            anchor("section", "t3", 3, 100.0),
        ];
        assert_eq!(page_keys(3, &anchors, true), ["front:1", "body:1", "body:2"]);
    }
}
