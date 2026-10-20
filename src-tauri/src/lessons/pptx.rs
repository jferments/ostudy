//! Writes a lecture as an editable PowerPoint (.pptx) package.
//!
//! A .pptx is a zip of Office Open XML parts, so the lecture is written here
//! with the pinned `zip` crate: no Office, converter or provider is involved,
//! and it works the same on every platform. Every slide uses one "Title and
//! Content" layout whose title and body are real placeholders, so the text
//! stays editable and shows in PowerPoint's outline. Each slide's speaker
//! notes hold its notes from the lecture.

use std::{
    fs,
    io::{BufWriter, Seek, Write},
};

use serde::{Deserialize, Serialize};
use tauri::State;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use super::{
    lecture::{read_lecture, LectureDto, LectureSlideKind},
    GetLessonInput,
};
use crate::{
    database::Database,
    error::CommandError,
    external_files::{
        load_prepared_export_summary, register_prepared_export, PreparedExportDto,
        RegisterPreparedExport,
    },
    paths::AppPaths,
    util::validate_entity_id,
};

const EXPORT_PURPOSE: &str = "lecture_pptx";
const PPTX_MEDIA_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// 16:9 at PowerPoint's default 13.333 by 7.5 inches, in EMU.
const SLIDE_WIDTH: i64 = 12_192_000;
const SLIDE_HEIGHT: i64 = 6_858_000;
const NOTES_WIDTH: i64 = 6_858_000;
const NOTES_HEIGHT: i64 = 9_144_000;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareLessonLecturePptxInput {
    pub lesson_id: String,
    /// An earlier revision to export; the latest when absent.
    pub revision_number: Option<i64>,
    pub operation_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LecturePptxSummary {
    revision_number: i64,
    slide_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedLecturePptxDto {
    pub prepared_export: PreparedExportDto,
    pub revision_number: i64,
    pub slide_count: usize,
}

/// Writes a lesson's lecture as a .pptx ready to publish. Repeating the same
/// operation returns the export it already prepared.
#[tauri::command]
pub(crate) fn prepare_lesson_lecture_pptx(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareLessonLecturePptxInput,
) -> Result<PreparedLecturePptxDto, CommandError> {
    prepare(database.inner(), paths.inner(), input)
}

pub(crate) fn prepare(
    database: &Database,
    paths: &AppPaths,
    input: PrepareLessonLecturePptxInput,
) -> Result<PreparedLecturePptxDto, CommandError> {
    validate_entity_id(&input.operation_id, "lecture export operation")?;
    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        load_prepared_export_summary::<LecturePptxSummary>(
            connection,
            paths,
            &input.operation_id,
            EXPORT_PURPOSE,
        )
    })? {
        return Ok(PreparedLecturePptxDto {
            prepared_export,
            revision_number: summary.revision_number,
            slide_count: summary.slide_count,
        });
    }

    let lecture = database.with_connection(|connection| {
        read_lecture(
            connection,
            GetLessonInput {
                lesson_id: input.lesson_id.clone(),
                revision_number: input.revision_number,
            },
        )
    })?;
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "pptx");
    let destination = paths.resolve_relative(&relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let file = fs::File::create(&destination).map_err(CommandError::io)?;
    let mut written = write_lecture_pptx(&lecture, BufWriter::new(file))?;
    written.flush().map_err(CommandError::io)?;
    written
        .into_inner()
        .map_err(|error| CommandError::io(error.into_error()))?
        .sync_all()
        .map_err(CommandError::io)?;

    let summary = LecturePptxSummary {
        revision_number: lecture.revision_number,
        slide_count: lecture.slides.len(),
    };
    let suggested_filename = format!("{}.pptx", portable_filename(&lecture.title));
    let prepared_export = database.with_connection(|connection| {
        register_prepared_export(
            connection,
            paths,
            RegisterPreparedExport {
                id: &input.operation_id,
                purpose: EXPORT_PURPOSE,
                suggested_filename: &suggested_filename,
                media_type: PPTX_MEDIA_TYPE,
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedLecturePptxDto {
        prepared_export,
        revision_number: summary.revision_number,
        slide_count: summary.slide_count,
    })
}

/// A file name every desktop and phone file system accepts.
fn portable_filename(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let result = result.trim().trim_matches('.').trim();
    if result.is_empty() {
        "lecture".to_string()
    } else {
        result.chars().take(120).collect()
    }
}

/// Writes `lecture` as a .pptx package to `out`.
pub(crate) fn write_lecture_pptx<W: Write + Seek>(
    lecture: &LectureDto,
    out: W,
) -> Result<W, CommandError> {
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    let count = lecture.slides.len();
    let mut part = |name: &str, body: String| -> Result<(), CommandError> {
        zip.start_file(name, options).map_err(export_failed)?;
        zip.write_all(body.as_bytes()).map_err(CommandError::io)
    };

    part("[Content_Types].xml", content_types(count))?;
    part("_rels/.rels", package_rels())?;
    part("docProps/app.xml", app_properties(count))?;
    part("docProps/core.xml", core_properties(&lecture.title))?;
    part("ppt/presentation.xml", presentation(count))?;
    part("ppt/_rels/presentation.xml.rels", presentation_rels(count))?;
    part("ppt/presProps.xml", pres_props())?;
    part("ppt/viewProps.xml", view_props())?;
    part("ppt/tableStyles.xml", table_styles())?;
    part("ppt/theme/theme1.xml", theme("oStudy"))?;
    part("ppt/theme/theme2.xml", theme("oStudy notes"))?;
    part("ppt/slideMasters/slideMaster1.xml", slide_master())?;
    part(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels(&[
            (1, "slideLayout", "../slideLayouts/slideLayout1.xml"),
            (2, "theme", "../theme/theme1.xml"),
        ]),
    )?;
    part("ppt/slideLayouts/slideLayout1.xml", slide_layout())?;
    part(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        rels(&[(1, "slideMaster", "../slideMasters/slideMaster1.xml")]),
    )?;
    part("ppt/notesMasters/notesMaster1.xml", notes_master())?;
    part(
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        rels(&[(1, "theme", "../theme/theme2.xml")]),
    )?;
    for (index, slide) in lecture.slides.iter().enumerate() {
        let number = index + 1;
        let title_slide = slide.kind == LectureSlideKind::Title;
        part(
            &format!("ppt/slides/slide{number}.xml"),
            slide_part(&slide.title, &slide.points, title_slide),
        )?;
        part(
            &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            rels(&[
                (1, "slideLayout", "../slideLayouts/slideLayout1.xml"),
                (
                    2,
                    "notesSlide",
                    &format!("../notesSlides/notesSlide{number}.xml"),
                ),
            ]),
        )?;
        part(
            &format!("ppt/notesSlides/notesSlide{number}.xml"),
            notes_slide(&slide.notes),
        )?;
        part(
            &format!("ppt/notesSlides/_rels/notesSlide{number}.xml.rels"),
            rels(&[
                (1, "notesMaster", "../notesMasters/notesMaster1.xml"),
                (2, "slide", &format!("../slides/slide{number}.xml")),
            ]),
        )?;
    }
    zip.finish().map_err(export_failed)
}

fn export_failed(error: zip::result::ZipError) -> CommandError {
    CommandError::new(
        "lecture_export_failed",
        format!("The PowerPoint file could not be written: {error}"),
    )
}

/// Text as XML character data. Characters XML 1.0 cannot carry are dropped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(character),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

fn xml(body: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{body}")
}

fn rels(entries: &[(usize, &str, &str)]) -> String {
    let mut body = format!("<Relationships xmlns=\"{PKG_REL}\">");
    for (id, kind, target) in entries {
        body.push_str(&format!(
            "<Relationship Id=\"rId{id}\" Type=\"{REL}/{kind}\" Target=\"{target}\"/>"
        ));
    }
    body.push_str("</Relationships>");
    xml(&body)
}

fn content_types(count: usize) -> String {
    let mut body = String::from(
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/>\
         <Override PartName=\"/ppt/presProps.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presProps+xml\"/>\
         <Override PartName=\"/ppt/viewProps.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml\"/>\
         <Override PartName=\"/ppt/tableStyles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml\"/>\
         <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\
         <Override PartName=\"/ppt/theme/theme2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\
         <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/>\
         <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>\
         <Override PartName=\"/ppt/notesMasters/notesMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml\"/>\
         <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
         <Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>",
    );
    for number in 1..=count {
        body.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{number}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\
             <Override PartName=\"/ppt/notesSlides/notesSlide{number}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml\"/>"
        ));
    }
    body.push_str("</Types>");
    xml(&body)
}

fn package_rels() -> String {
    let body = format!(
        "<Relationships xmlns=\"{PKG_REL}\">\
         <Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"ppt/presentation.xml\"/>\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
         <Relationship Id=\"rId3\" Type=\"{REL}/extended-properties\" Target=\"docProps/app.xml\"/>\
         </Relationships>"
    );
    xml(&body)
}

fn app_properties(count: usize) -> String {
    xml(&format!(
        "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\
         <Application>oStudy</Application><Slides>{count}</Slides><Notes>{count}</Notes>\
         <PresentationFormat>Widescreen</PresentationFormat></Properties>"
    ))
}

fn core_properties(title: &str) -> String {
    xml(&format!(
        "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\
         <dc:title>{}</dc:title><dc:creator>oStudy</dc:creator></cp:coreProperties>",
        escape(title)
    ))
}

/// Slide ids start at 256, as PowerPoint requires.
fn presentation(count: usize) -> String {
    let mut slides = String::new();
    for index in 0..count {
        slides.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
            256 + index,
            index + 10
        ));
    }
    xml(&format!(
        "<p:presentation xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\" saveSubsetFonts=\"1\">\
         <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
         <p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst>\
         <p:sldIdLst>{slides}</p:sldIdLst>\
         <p:sldSz cx=\"{SLIDE_WIDTH}\" cy=\"{SLIDE_HEIGHT}\"/>\
         <p:notesSz cx=\"{NOTES_WIDTH}\" cy=\"{NOTES_HEIGHT}\"/>\
         <p:defaultTextStyle><a:lvl1pPr><a:defRPr lang=\"en-US\"/></a:lvl1pPr></p:defaultTextStyle>\
         </p:presentation>"
    ))
}

fn presentation_rels(count: usize) -> String {
    let mut entries: Vec<(usize, &str, String)> = vec![
        (
            1,
            "slideMaster",
            "slideMasters/slideMaster1.xml".to_string(),
        ),
        (
            2,
            "notesMaster",
            "notesMasters/notesMaster1.xml".to_string(),
        ),
        (3, "theme", "theme/theme1.xml".to_string()),
        (4, "presProps", "presProps.xml".to_string()),
        (5, "viewProps", "viewProps.xml".to_string()),
        (6, "tableStyles", "tableStyles.xml".to_string()),
    ];
    for index in 0..count {
        entries.push((
            index + 10,
            "slide",
            format!("slides/slide{}.xml", index + 1),
        ));
    }
    let borrowed: Vec<(usize, &str, &str)> = entries
        .iter()
        .map(|(id, kind, target)| (*id, *kind, target.as_str()))
        .collect();
    rels(&borrowed)
}

fn pres_props() -> String {
    xml(&format!(
        "<p:presentationPr xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\"/>"
    ))
}

fn view_props() -> String {
    xml(&format!(
        "<p:viewPr xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:normalViewPr><p:restoredLeft sz=\"15620\"/><p:restoredTop sz=\"80000\"/></p:normalViewPr>\
         <p:gridSpacing cx=\"76200\" cy=\"76200\"/></p:viewPr>"
    ))
}

fn table_styles() -> String {
    xml(&format!(
        "<a:tblStyleLst xmlns:a=\"{NS_A}\" def=\"{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}\"/>"
    ))
}

fn theme(name: &str) -> String {
    let fill_styles = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>".repeat(3);
    let line_styles =
        "<a:ln w=\"9525\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>".repeat(3);
    let effect_styles = "<a:effectStyle><a:effectLst/></a:effectStyle>".repeat(3);
    xml(&format!(
        "<a:theme xmlns:a=\"{NS_A}\" name=\"{name}\"><a:themeElements>\
         <a:clrScheme name=\"oStudy\">\
         <a:dk1><a:srgbClr val=\"1F2328\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>\
         <a:dk2><a:srgbClr val=\"24343A\"/></a:dk2><a:lt2><a:srgbClr val=\"F4F6F6\"/></a:lt2>\
         <a:accent1><a:srgbClr val=\"2F6F7E\"/></a:accent1><a:accent2><a:srgbClr val=\"C2703D\"/></a:accent2>\
         <a:accent3><a:srgbClr val=\"5B8C5A\"/></a:accent3><a:accent4><a:srgbClr val=\"7A5C99\"/></a:accent4>\
         <a:accent5><a:srgbClr val=\"B5485D\"/></a:accent5><a:accent6><a:srgbClr val=\"8A7A3C\"/></a:accent6>\
         <a:hlink><a:srgbClr val=\"2F6F7E\"/></a:hlink><a:folHlink><a:srgbClr val=\"7A5C99\"/></a:folHlink>\
         </a:clrScheme>\
         <a:fontScheme name=\"oStudy\">\
         <a:majorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>\
         <a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>\
         </a:fontScheme>\
         <a:fmtScheme name=\"oStudy\">\
         <a:fillStyleLst>{fill_styles}</a:fillStyleLst>\
         <a:lnStyleLst>{line_styles}</a:lnStyleLst>\
         <a:effectStyleLst>{effect_styles}</a:effectStyleLst>\
         <a:bgFillStyleLst>{fill_styles}</a:bgFillStyleLst>\
         </a:fmtScheme></a:themeElements></a:theme>"
    ))
}

fn color_map() -> &'static str {
    "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" \
     accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>"
}

fn group_properties() -> &'static str {
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
     <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>\
     <a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>"
}

/// A placeholder shape. `frame` is its position in EMU, or none to inherit
/// the layout's.
fn placeholder(
    id: usize,
    name: &str,
    kind: Option<&str>,
    index: Option<usize>,
    frame: Option<(i64, i64, i64, i64)>,
    body: &str,
) -> String {
    let mut ph = String::from("<p:ph");
    if let Some(kind) = kind {
        ph.push_str(&format!(" type=\"{kind}\""));
    }
    if let Some(index) = index {
        ph.push_str(&format!(" idx=\"{index}\""));
    }
    ph.push_str("/>");
    let geometry = match frame {
        Some((x, y, cx, cy)) => format!(
            "<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>"
        ),
        None => "<p:spPr/>".to_string(),
    };
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr>{ph}</p:nvPr></p:nvSpPr>\
         {geometry}<p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp>"
    )
}

fn empty_paragraph() -> &'static str {
    "<a:p><a:endParaRPr lang=\"en-US\"/></a:p>"
}

fn run_paragraph(text: &str, properties: &str) -> String {
    format!(
        "<a:p>{properties}<a:r><a:rPr lang=\"en-US\" dirty=\"0\"/><a:t>{}</a:t></a:r></a:p>",
        escape(text)
    )
}

const TITLE_FRAME: (i64, i64, i64, i64) = (838_200, 365_125, 10_515_600, 1_325_563);
const BODY_FRAME: (i64, i64, i64, i64) = (838_200, 1_825_625, 10_515_600, 4_351_338);

fn slide_master() -> String {
    let shapes = format!(
        "{}{}",
        placeholder(
            2,
            "Title",
            Some("title"),
            None,
            Some(TITLE_FRAME),
            empty_paragraph()
        ),
        placeholder(
            3,
            "Body",
            Some("body"),
            Some(1),
            Some(BODY_FRAME),
            empty_paragraph()
        ),
    );
    xml(&format!(
        "<p:sldMaster xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
         <p:spTree>{}{shapes}</p:spTree></p:cSld>{}\
         <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
         <p:txStyles>\
         <p:titleStyle><a:lvl1pPr algn=\"l\"><a:defRPr sz=\"4000\" b=\"1\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill>\
         <a:latin typeface=\"+mj-lt\"/></a:defRPr></a:lvl1pPr></p:titleStyle>\
         <p:bodyStyle><a:lvl1pPr marL=\"342900\" indent=\"-342900\"><a:spcBef><a:spcPts val=\"1000\"/></a:spcBef>\
         <a:buFont typeface=\"Arial\"/><a:buChar char=\"&#8226;\"/>\
         <a:defRPr sz=\"2600\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/></a:defRPr>\
         </a:lvl1pPr></p:bodyStyle>\
         <p:otherStyle><a:lvl1pPr><a:defRPr sz=\"1800\"><a:latin typeface=\"+mn-lt\"/></a:defRPr></a:lvl1pPr></p:otherStyle>\
         </p:txStyles></p:sldMaster>",
        group_properties(),
        color_map(),
    ))
}

fn slide_layout() -> String {
    let shapes = format!(
        "{}{}",
        placeholder(2, "Title", Some("title"), None, None, empty_paragraph()),
        placeholder(3, "Content", None, Some(1), None, empty_paragraph()),
    );
    xml(&format!(
        "<p:sldLayout xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\" type=\"obj\" preserve=\"1\">\
         <p:cSld name=\"Title and Content\"><p:spTree>{}{shapes}</p:spTree></p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>",
        group_properties(),
    ))
}

fn slide_part(title: &str, points: &[String], title_slide: bool) -> String {
    // A title slide's points are the lesson's objectives: plain lines, not
    // bullets, under a larger title.
    let title_body = if title_slide {
        format!(
            "<a:p><a:r><a:rPr lang=\"en-US\" sz=\"4800\" dirty=\"0\"/><a:t>{}</a:t></a:r></a:p>",
            escape(title)
        )
    } else {
        run_paragraph(title, "")
    };
    let point_properties = if title_slide {
        "<a:pPr marL=\"0\" indent=\"0\"><a:buNone/></a:pPr>"
    } else {
        ""
    };
    let body = if points.is_empty() {
        empty_paragraph().to_string()
    } else {
        points
            .iter()
            .map(|point| run_paragraph(point, point_properties))
            .collect()
    };
    let mut shapes = placeholder(2, "Title", Some("title"), None, None, &title_body);
    if !points.is_empty() {
        shapes.push_str(&placeholder(3, "Content", None, Some(1), None, &body));
    }
    xml(&format!(
        "<p:sld xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:cSld><p:spTree>{}{shapes}</p:spTree></p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        group_properties(),
    ))
}

const NOTES_IMAGE_FRAME: (i64, i64, i64, i64) = (685_800, 1_143_000, 5_486_400, 3_086_100);
const NOTES_BODY_FRAME: (i64, i64, i64, i64) = (685_800, 4_400_550, 5_486_400, 3_600_450);

fn notes_master() -> String {
    let image = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Slide Image\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\" noRot=\"1\" noChangeAspect=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"sldImg\" idx=\"2\"/></p:nvPr></p:nvSpPr>\
         <p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:sp>",
        NOTES_IMAGE_FRAME.0, NOTES_IMAGE_FRAME.1, NOTES_IMAGE_FRAME.2, NOTES_IMAGE_FRAME.3
    );
    let body = placeholder(
        3,
        "Notes",
        Some("body"),
        Some(1),
        Some(NOTES_BODY_FRAME),
        empty_paragraph(),
    );
    xml(&format!(
        "<p:notesMaster xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
         <p:spTree>{}{image}{body}</p:spTree></p:cSld>{}\
         <p:notesStyle><a:lvl1pPr><a:defRPr sz=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill>\
         <a:latin typeface=\"+mn-lt\"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>",
        group_properties(),
        color_map(),
    ))
}

fn notes_slide(notes: &str) -> String {
    let paragraphs: String = if notes.trim().is_empty() {
        empty_paragraph().to_string()
    } else {
        notes
            .split('\n')
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| run_paragraph(line, ""))
            .collect()
    };
    let image = "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Slide Image\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\" noRot=\"1\" noChangeAspect=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"sldImg\"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>";
    let body = placeholder(3, "Notes", Some("body"), Some(1), None, &paragraphs);
    xml(&format!(
        "<p:notes xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:cSld><p:spTree>{}{image}{body}</p:spTree></p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>",
        group_properties(),
    ))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;
    use crate::lessons::lecture::LectureSlideDto;

    fn lecture() -> LectureDto {
        LectureDto {
            lesson_id: "1".repeat(32),
            revision_number: 1,
            title: "Loop of Henle".to_string(),
            slides: vec![
                LectureSlideDto {
                    key: "title".to_string(),
                    kind: LectureSlideKind::Title,
                    step_key: None,
                    title: "Loop of Henle".to_string(),
                    points: vec!["Explain the gradient".to_string()],
                    notes: "How the loop works.".to_string(),
                },
                LectureSlideDto {
                    key: "step:two-limbs".to_string(),
                    kind: LectureSlideKind::Step,
                    step_key: Some("two-limbs".to_string()),
                    title: "Salt & water <in> the \"loop\"".to_string(),
                    points: vec![
                        "Water out\u{1} going down".to_string(),
                        "Salt pumped up".to_string(),
                    ],
                    notes: "Picture two tubes.\nGoing down, water slips out.".to_string(),
                },
                LectureSlideDto {
                    key: "sources".to_string(),
                    kind: LectureSlideKind::Sources,
                    step_key: None,
                    title: "Sources".to_string(),
                    points: vec![],
                    notes: String::new(),
                },
            ],
        }
    }

    fn package() -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let bytes = write_lecture_pptx(&lecture(), Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        zip::ZipArchive::new(Cursor::new(bytes)).unwrap()
    }

    fn read(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap_or_else(|_| panic!("{name} is missing"))
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    fn texts(xml: &str) -> Vec<String> {
        let document = roxmltree::Document::parse(xml).unwrap();
        document
            .descendants()
            .filter(|node| node.has_tag_name((NS_A, "t")))
            .map(|node| node.text().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn every_part_is_well_formed_and_every_relationship_resolves() {
        let mut archive = package();
        let names: Vec<String> = archive.file_names().map(str::to_string).collect();
        for name in &names {
            let text = read(&mut archive, name);
            roxmltree::Document::parse(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        }
        for name in names.iter().filter(|name| name.ends_with(".rels")) {
            let text = read(&mut archive, name);
            let document = roxmltree::Document::parse(&text).unwrap();
            // "a/_rels/b.xml.rels" describes "a/b.xml"; targets are relative to "a".
            let base = name
                .strip_suffix(".rels")
                .and_then(|rest| rest.rsplit_once("_rels/"))
                .map(|(folder, _)| folder.to_string())
                .unwrap();
            for target in document
                .descendants()
                .filter_map(|node| node.attribute("Target"))
            {
                let mut parts: Vec<&str> =
                    base.split('/').filter(|part| !part.is_empty()).collect();
                for piece in target.split('/') {
                    if piece == ".." {
                        parts.pop();
                    } else {
                        parts.push(piece);
                    }
                }
                let resolved = parts.join("/");
                assert!(
                    names.contains(&resolved),
                    "{name} points at missing {resolved}"
                );
            }
        }
        let types = read(&mut archive, "[Content_Types].xml");
        for name in names
            .iter()
            .filter(|name| name.starts_with("ppt/") && name.ends_with(".xml"))
        {
            assert!(
                types.contains(&format!("PartName=\"/{name}\"")),
                "{name} has no content type"
            );
        }
    }

    #[test]
    fn slides_keep_their_text_editable_with_the_notes_beside_them() {
        let mut archive = package();
        let presentation = read(&mut archive, "ppt/presentation.xml");
        assert_eq!(presentation.matches("<p:sldId ").count(), 3);
        assert!(presentation.contains(&format!("cx=\"{SLIDE_WIDTH}\" cy=\"{SLIDE_HEIGHT}\"")));

        assert_eq!(
            texts(&read(&mut archive, "ppt/slides/slide1.xml")),
            ["Loop of Henle", "Explain the gradient"]
        );
        let step = read(&mut archive, "ppt/slides/slide2.xml");
        assert!(step.contains("<p:ph type=\"title\"/>") && step.contains("<p:ph idx=\"1\"/>"));
        assert_eq!(
            texts(&step),
            [
                "Salt & water <in> the \"loop\"",
                "Water out going down",
                "Salt pumped up"
            ]
        );
        assert_eq!(
            texts(&read(&mut archive, "ppt/notesSlides/notesSlide2.xml")),
            ["Picture two tubes.", "Going down, water slips out."]
        );
        assert_eq!(
            texts(&read(&mut archive, "ppt/slides/slide3.xml")),
            ["Sources"]
        );
        assert!(texts(&read(&mut archive, "ppt/notesSlides/notesSlide3.xml")).is_empty());
        assert_eq!(
            texts(&read(&mut archive, "docProps/core.xml")).len(),
            0,
            "core properties use Dublin Core, not DrawingML text"
        );
        assert!(
            read(&mut archive, "docProps/core.xml").contains("<dc:title>Loop of Henle</dc:title>")
        );
    }

    #[test]
    fn the_same_lecture_writes_the_same_bytes() {
        let first = write_lecture_pptx(&lecture(), Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        let second = write_lecture_pptx(&lecture(), Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        assert_eq!(first, second);
    }
}
