//! A figure drawn from its page with or without its caption. The figure and
//! the caption under it are rendered as one image, which is segmented into
//! bands of ink parted by blank rows; the caption is the run of text-sized
//! bands that starts where the page's text says the caption does. The crop is
//! then made from what the image shows, so a scanned page whose text layer
//! sits lower than the printed letters never leaves part of a caption behind.

use std::path::Path;

use image::RgbaImage;

use super::{reading::crop_tile, DocumentFigureDto, FigureBoxDto};
use crate::pdf_render_backend::{self, PdfRenderRuntime};

/// The fewest blank rows, in points, between two bands.
const BAND_GAP_POINTS: f64 = 1.2;
/// The tallest band, in points, that is one line of caption text.
const TEXT_LINE_POINTS: f64 = 14.0;
/// How far, in points, a caption's printed letters may sit above where the
/// text layer puts them.
const CAPTION_SLACK_POINTS: f64 = 12.0;
/// The widest gap, in points, between two lines of one caption.
const CAPTION_LINE_GAP_POINTS: f64 = 8.0;
/// A short band at the image's top edge, parted from the figure, is a piece
/// of whatever lies above it (a header, the line before the figure).
const EDGE_FRAGMENT_POINTS: f64 = 20.0;
/// Blank paper kept around the figure, in points.
const PAD_POINTS: f64 = 3.0;

/// Rows (or columns) `start..end` of an image that hold ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Band {
    pub start: u32,
    pub end: u32,
}

impl Band {
    fn height(self) -> u32 {
        self.end - self.start
    }
}

/// Where the figure and its caption are in an image of both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Segments {
    /// Rows and columns of the figure alone.
    pub figure: (Band, Band),
    /// Rows of the caption under it, when it was found.
    pub caption: Option<Band>,
}

/// The figure as an image, with its caption under it when `with_caption` and
/// one was found, cropped to what the rendered page shows.
pub(crate) fn figure_image(
    runtime: &PdfRenderRuntime,
    source: &Path,
    figure: &DocumentFigureDto,
    with_caption: bool,
) -> Option<RgbaImage> {
    let page = usize::try_from(figure.page_number).ok()?;
    let below = figure
        .caption_bounds
        .filter(|caption| caption.top >= figure.bounds.top + figure.bounds.height / 2.0);
    let area = below.map_or(figure.bounds, |caption| union(figure.bounds, caption));
    let tile = crop_tile(figure.page_width, figure.page_height, area);
    let png = pdf_render_backend::render_page_tile(runtime, source, page, tile).ok()?;
    let pixels = image::load_from_memory(&png).ok()?.to_rgba8();
    let scale = f64::from(tile.page_width) / figure.page_width.max(1.0);
    let caption_top = below.map(|caption| caption.top * scale - f64::from(tile.top));
    let segments = segment(&pixels, scale, caption_top)?;
    let (rows, columns) = match (with_caption, segments.caption) {
        (true, Some(caption)) => (
            Band {
                start: segments.figure.0.start,
                end: caption.end,
            },
            Band {
                start: 0,
                end: pixels.width(),
            },
        ),
        _ => segments.figure,
    };
    let pad = (PAD_POINTS * scale).round() as u32;
    let top = rows.start.saturating_sub(pad);
    let bottom = (rows.end + pad).min(pixels.height());
    let left = columns.start.saturating_sub(pad);
    let right = (columns.end + pad).min(pixels.width());
    Some(image::imageops::crop_imm(&pixels, left, top, right - left, bottom - top).to_image())
}

fn union(a: FigureBoxDto, b: FigureBoxDto) -> FigureBoxDto {
    let left = a.left.min(b.left);
    let top = a.top.min(b.top);
    FigureBoxDto {
        left,
        top,
        width: (a.left + a.width).max(b.left + b.width) - left,
        height: (a.top + a.height).max(b.top + b.height) - top,
    }
}

/// Splits an image of a figure, and the caption under it when `caption_top`
/// (in pixels) says where the page's text puts it, into the figure and the
/// caption. `scale` is pixels per point. Nothing when the image is blank.
pub(crate) fn segment(
    pixels: &RgbaImage,
    scale: f64,
    caption_top: Option<f64>,
) -> Option<Segments> {
    let points = |value: f64| (value * scale).round().max(1.0) as u32;
    let rows = bands(&row_ink(pixels), points(BAND_GAP_POINTS));
    if rows.is_empty() {
        return None;
    }
    let line = points(TEXT_LINE_POINTS);
    // The caption: text-sized bands from about where the text layer puts it,
    // each close under the one before.
    let caption_from = caption_top.and_then(|top| {
        let earliest = top - CAPTION_SLACK_POINTS * scale;
        rows.iter()
            .position(|band| f64::from(band.end) > earliest && band.height() <= line)
            .filter(|&index| index > 0)
    });
    let caption = caption_from.map(|first| {
        let mut last = first;
        while last + 1 < rows.len()
            && rows[last + 1].height() <= line
            && rows[last + 1].start - rows[last].end <= points(CAPTION_LINE_GAP_POINTS)
        {
            last += 1;
        }
        Band {
            start: rows[first].start,
            end: rows[last].end,
        }
    });
    let mut figure_rows = &rows[..caption_from.unwrap_or(rows.len())];
    // A short band cut by the image's top edge, apart from the rest, is not
    // the figure's.
    if figure_rows.len() > 1
        && figure_rows[0].start == 0
        && figure_rows[0].height() <= points(EDGE_FRAGMENT_POINTS)
    {
        figure_rows = &figure_rows[1..];
    }
    let rows_band = Band {
        start: figure_rows.first()?.start,
        end: figure_rows.last()?.end,
    };
    let columns = bands(&column_ink(pixels, rows_band), points(BAND_GAP_POINTS));
    let columns_band = Band {
        start: columns.first().map_or(0, |band| band.start),
        end: columns.last().map_or(pixels.width(), |band| band.end),
    };
    Some(Segments {
        figure: (rows_band, columns_band),
        caption,
    })
}

fn is_ink(pixel: &image::Rgba<u8>) -> bool {
    let [r, g, b, a] = pixel.0;
    a > 128 && r.min(g).min(b) < 190
}

/// Whether each row holds more than a speck of ink.
fn row_ink(pixels: &RgbaImage) -> Vec<bool> {
    let speck = (pixels.width() as usize / 500).max(1);
    (0..pixels.height())
        .map(|y| {
            (0..pixels.width())
                .filter(|&x| is_ink(pixels.get_pixel(x, y)))
                .count()
                > speck
        })
        .collect()
}

/// Whether each column holds ink within `rows`.
fn column_ink(pixels: &RgbaImage, rows: Band) -> Vec<bool> {
    let speck = (rows.height() as usize / 500).max(1);
    (0..pixels.width())
        .map(|x| {
            (rows.start..rows.end)
                .filter(|&y| is_ink(pixels.get_pixel(x, y)))
                .count()
                > speck
        })
        .collect()
}

/// Runs of inked lines, parted by at least `gap` blank ones.
pub(crate) fn bands(inked: &[bool], gap: u32) -> Vec<Band> {
    let mut found = Vec::<Band>::new();
    let mut start = None;
    for (index, &ink) in inked.iter().enumerate() {
        let index = index as u32;
        match (ink, start) {
            (true, None) => start = Some(index),
            (false, Some(first)) => {
                found.push(Band {
                    start: first,
                    end: index,
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(first) = start {
        found.push(Band {
            start: first,
            end: inked.len() as u32,
        });
    }
    // Bands closer than `gap` are one.
    let mut merged = Vec::<Band>::new();
    for band in found {
        match merged.last_mut() {
            Some(last) if band.start - last.end < gap => last.end = band.end,
            _ => merged.push(band),
        }
    }
    merged
}
