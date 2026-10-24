use super::*;
use crate::{
    document_domain::{SourceLocator, SourceUnitKind},
    document_figures::{FigureBoxDto, FigureReadingDto},
};

fn section(
    topic: char,
    pages: Vec<i64>,
    overview: &str,
    key_points: &[&str],
) -> DocumentStudyGuideSection {
    DocumentStudyGuideSection {
        topic_id: topic.to_string().repeat(32),
        parent_topic_id: None,
        sort_index: 0,
        title: "Section".to_string(),
        topic_summary: String::new(),
        content: StudyGuideSectionContent {
            overview: overview.to_string(),
            key_points: key_points.iter().map(ToString::to_string).collect(),
            ..StudyGuideSectionContent::default()
        },
        content_text: String::new(),
        content_hash: "b".repeat(64),
        source_locator: SourceLocator::new(SourceUnitKind::Page, pages, None).unwrap(),
    }
}

fn figure(id: &str, page: i64, importance: i64, concepts: &[&str]) -> DocumentFigureDto {
    DocumentFigureDto {
        figure_id: id.to_string(),
        page_number: page,
        page_width: 612.0,
        page_height: 792.0,
        kind: "image".to_string(),
        bounds: FigureBoxDto {
            left: 72.0,
            top: 100.0,
            width: 300.0,
            height: 200.0,
        },
        caption_bounds: None,
        label: None,
        caption: None,
        hidden: false,
        passages: Vec::new(),
        reading: Some(FigureReadingDto {
            title: format!("Figure {id}"),
            shows: "Arrows trace the path. Labels name each part.".to_string(),
            use_in_text: None,
            concepts: concepts.iter().map(ToString::to_string).collect(),
            explained_on: vec![page],
            importance,
            worth_studying: true,
            model: "test".to_string(),
        }),
    }
}

fn ids(chosen: &HashMap<String, Vec<ChosenFigure>>, topic: char) -> Vec<&str> {
    chosen
        .get(&topic.to_string().repeat(32))
        .map(|figures| {
            figures
                .iter()
                .map(|figure| figure.figure_id.as_str())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_section_shows_the_figures_its_ideas_need_and_no_more_than_two() {
    let sections = [section(
        'a',
        vec![3, 4],
        "Gastrin stimulates acid secretion.",
        &["Parietal cells secrete HCl."],
    )];
    let figures = [
        figure("p00003-01", 3, 5, &["vagal reflex"]),
        figure("p00003-02", 3, 3, &["parietal cell"]),
        figure("p00004-01", 4, 4, &["gastric gland"]),
        figure("p00004-02", 4, 3, &["bile duct"]),
    ];
    let chosen = choose(&sections, &figures);
    // The needed figures win; a helpful one only shows for words the section uses,
    // and two is the most a section shows, in the source's order.
    assert_eq!(ids(&chosen, 'a'), ["p00003-01", "p00004-01"]);
}

#[test]
fn hidden_unread_minor_and_unworthy_figures_never_show() {
    let sections = [section('a', vec![5], "Secretin releases bicarbonate.", &[])];
    let mut hidden = figure("p00005-01", 5, 5, &["secretin"]);
    hidden.hidden = true;
    let mut unread = figure("p00005-02", 5, 5, &["secretin"]);
    unread.reading = None;
    let minor = figure("p00005-03", 5, 2, &["secretin"]);
    let mut unworthy = figure("p00005-04", 5, 5, &["secretin"]);
    unworthy.reading.as_mut().unwrap().worth_studying = false;
    let unrelated = figure("p00005-05", 5, 3, &["liver lobule"]);
    let chosen = choose(&sections, &[hidden, unread, minor, unworthy, unrelated]);
    assert!(chosen.is_empty());
}

#[test]
fn each_figure_shows_once_in_the_section_it_fits_best() {
    let sections = [
        section('a', vec![7], "Motility overview.", &[]),
        section(
            'b',
            vec![8],
            "Migrating motor complex.",
            &["The migrating motor complex clears the gut."],
        ),
    ];
    // Page 7's figure is explained on page 8 as well and shares words with the second section.
    let mut shared = figure("p00007-01", 7, 4, &["migrating motor complex"]);
    shared.reading.as_mut().unwrap().explained_on = vec![7, 8];
    let chosen = choose(&sections, &[shared]);
    assert_eq!(ids(&chosen, 'a'), Vec::<&str>::new());
    assert_eq!(ids(&chosen, 'b'), ["p00007-01"]);
}

#[test]
fn a_figure_from_another_part_of_the_document_stays_out() {
    let sections = [section(
        'a',
        vec![1, 2],
        "Salivary glands.",
        &["Saliva is hypotonic."],
    )];
    let chosen = choose(&sections, &[figure("p00009-01", 9, 5, &["saliva"])]);
    assert!(chosen.is_empty());
}

#[test]
fn an_unread_numbered_figure_shows_once_when_its_caption_matches_the_section() {
    let sections = [section(
        'a',
        vec![23],
        "Gastric parietal cells use a mechanism for HCl secretion.",
        &["The proton pump exchanges H+ for K+."],
    )];
    let unread = |id: &str, caption: &str| {
        let mut figure = figure(id, 23, 5, &[]);
        figure.reading = None;
        figure.label = Some("Figure 8.17".to_string());
        figure.caption = Some(caption.to_string());
        figure
    };
    let chosen = choose(
        &sections,
        &[
            unread("p00023-01", "Fig. 8.17 Mechanism of HCl secretion by gastric parietal cells. ATP, adenosine triphosphate."),
            unread("p00023-02", "Fig. 8.18 Agents that stimulate parietal cells to secrete HCl."),
            unread("p00023-03", "Fig. 8.19 Blood flow in the splanchnic circulation."),
        ],
    );
    let figures = chosen.get(&"a".repeat(32)).unwrap();
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].figure_id, "p00023-01");
    assert_eq!(
        figures[0].title,
        "Mechanism of HCl secretion by gastric parietal cells"
    );
    // The abbreviation key is left out.
    assert_eq!(figures[0].caption, "Source page 23");

    assert_eq!(
        caption_detail("Initial saliva is produced by acinar cells. ACh, Acetylcholine; ATP,"),
        "Initial saliva is produced by acinar cells"
    );
    assert_eq!(
        caption_detail("Efferent neurons synapse in the plexuses. ACh,"),
        "Efferent neurons synapse in the plexuses"
    );

    let mut unlabelled = unread(
        "p00023-04",
        "Mechanism of HCl secretion by gastric parietal cells.",
    );
    unlabelled.label = None;
    assert!(choose(&sections, &[unlabelled]).is_empty());
}

#[test]
fn captions_are_short_and_name_the_source_page() {
    assert_eq!(
        caption("Arrows trace the path.", 12),
        "Arrows trace the path. Source page 12"
    );
    assert_eq!(caption("", 3), "Source page 3");
    let long = format!("{}. {}", "a".repeat(120), "b".repeat(200));
    let cut = caption(&long, 4);
    assert!(cut.chars().count() < MAX_CAPTION_CHARS + 20, "{cut}");
    assert!(cut.ends_with("Source page 4"));
}
