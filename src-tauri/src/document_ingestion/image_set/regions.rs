//! Exact paragraph regions in admitted source coordinates. No fuzzy text,
//! guessed layout, or screenshot position participates in region identity.

use crate::{
    document_canonical::sha256_hex,
    documents::image_sets::{
        manifest::ImageMember,
        raster::{contains, Raster},
    },
    error::CommandError,
    pdf_ocr_backend::regions::{OcrLine, PixelBounds, RegionOcrResult},
};

use super::invalid;

#[derive(Clone)]
pub(super) struct Region {
    pub id: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub bounds: Option<PixelBounds>,
    pub strip: Option<PixelBounds>,
    pub signature: Option<String>,
    pub ordered: bool,
}

pub(super) struct Frame {
    pub identity: String,
    pub regions: Vec<Region>,
    pub unresolved: usize,
    pub available: bool,
}

impl Frame {
    pub(super) fn build(
        member: &ImageMember,
        image: &Raster,
        inclusion: PixelBounds,
        result: Option<&RegionOcrResult>,
    ) -> Result<Self, CommandError> {
        let identity = serde_json::to_vec(&(
            "image-frame-v1",
            &member.pixel_sha256,
            &member.pixel_contract,
            member.quarter_turns,
            inclusion,
        ))
        .map_err(CommandError::serialization)?;
        let mut frame = Self {
            identity: sha256_hex(&identity),
            regions: Vec::new(),
            unresolved: usize::from(result.is_none()),
            available: result.is_some(),
        };
        let Some(result) = result else {
            return Ok(frame);
        };
        result
            .validate()
            .map_err(|_| invalid("Invalid image-region source evidence."))?;
        if (result.width, result.height) != image.dimensions() {
            return Err(invalid("OCR coordinates do not match the source raster."));
        }
        // Each raster contributes at most two full-image passes of strip hashing.
        // Pathological overlapping OCR boxes remain unmerged instead of causing
        // quadratic pixel work. The exact retained text is never dropped.
        let (width, height) = image.dimensions();
        let mut pixel_budget = u64::from(width) * u64::from(height) * 2;
        let mut start = 0;
        while start < result.lines.len() {
            let mut end = start + 1;
            while end < result.lines.len() && result.lines[end].group == result.lines[start].group {
                end += 1;
            }
            let lines = &result.lines[start..end];
            let bounds = union(lines);
            let admitted = bounds.map_or(member.inclusion.is_none(), |b| contains(inclusion, b));
            if admitted {
                frame.regions.push(region(
                    member,
                    image,
                    inclusion,
                    lines,
                    start,
                    end,
                    &mut pixel_budget,
                )?);
            } else {
                // Never turn a clipped qualifier into a complete paragraph.
                frame.unresolved += lines.len();
            }
            start = end;
        }
        Ok(frame)
    }
}

fn region(
    member: &ImageMember,
    image: &Raster,
    inclusion: PixelBounds,
    lines: &[OcrLine],
    start: usize,
    end: usize,
    pixel_budget: &mut u64,
) -> Result<Region, CommandError> {
    let bounds = union(lines);
    let text = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let identity = serde_json::to_vec(&(
        "image-region-v1",
        &member.id,
        &member.pixel_sha256,
        &member.pixel_contract,
        member.quarter_turns,
        lines,
    ))
    .map_err(CommandError::serialization)?;
    let strip = bounds.map(|bounds| PixelBounds {
        left: inclusion.left,
        right: inclusion.right,
        ..bounds
    });
    let affordable_strip = strip.filter(|strip| {
        let cost = u64::from(strip.right - strip.left) * u64::from(strip.bottom - strip.top);
        if let Some(remaining) = pixel_budget.checked_sub(cost) {
            *pixel_budget = remaining;
            true
        } else {
            false
        }
    });
    let signature = match (bounds, affordable_strip) {
        (Some(bounds), Some(strip)) => {
            let relative = lines
                .iter()
                .filter_map(|line| line.bounds)
                .map(|line| {
                    (
                        line.left,
                        line.top - bounds.top,
                        line.right,
                        line.bottom - bounds.top,
                    )
                })
                .collect::<Vec<_>>();
            let proof = serde_json::to_vec(&(
                "image-region-pixels-v1",
                &text,
                relative,
                image.strip_fingerprint(strip)?,
            ))
            .map_err(CommandError::serialization)?;
            Some(sha256_hex(&proof))
        }
        _ => None,
    };
    Ok(Region {
        id: sha256_hex(&identity),
        text,
        start,
        end,
        bounds,
        strip,
        signature,
        ordered: bounds.is_some()
            && lines.windows(2).all(|pair| {
                pair[0]
                    .bounds
                    .zip(pair[1].bounds)
                    .is_some_and(|(a, b)| a.bottom <= b.top)
            }),
    })
}

fn union(lines: &[OcrLine]) -> Option<PixelBounds> {
    let mut result = None::<PixelBounds>;
    for line in lines {
        let bounds = line.bounds?;
        result = Some(match result {
            None => bounds,
            Some(previous) => PixelBounds {
                left: previous.left.min(bounds.left),
                top: previous.top.min(bounds.top),
                right: previous.right.max(bounds.right),
                bottom: previous.bottom.max(bounds.bottom),
            },
        });
    }
    result
}
