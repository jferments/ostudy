//! Exact OCR groups, source-role decisions, and bounded native pixel proofs.

use crate::{
    document_canonical::sha256_hex,
    documents::image_sets::{
        filter::{self, RegionDecision, RegionRole},
        manifest::ImageMember,
        raster::{contains, Raster},
        storage::FilterSettings,
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
    pub decisions: Vec<RegionDecision>,
    pub masked: Vec<PixelBounds>,
    pub withhold_visual: bool,
    pub unresolved: usize,
    pub available: bool,
}

impl Frame {
    #[cfg(test)]
    pub(super) fn build(
        member: &ImageMember,
        image: &Raster,
        inclusion: PixelBounds,
        result: Option<&RegionOcrResult>,
    ) -> Result<Self, CommandError> {
        Self::with_settings(member, image, inclusion, result, &FilterSettings::default())
    }

    pub(super) fn with_settings(
        member: &ImageMember,
        image: &Raster,
        inclusion: PixelBounds,
        result: Option<&RegionOcrResult>,
        settings: &FilterSettings,
    ) -> Result<Self, CommandError> {
        let mut frame = Self {
            identity: String::new(),
            regions: Vec::new(),
            decisions: Vec::new(),
            masked: Vec::new(),
            withhold_visual: false,
            unresolved: usize::from(result.is_none()),
            available: result.is_some(),
        };
        if let Some(result) = result {
            result
                .validate()
                .map_err(|_| invalid("Invalid image-region source evidence."))?;
            if (result.width, result.height) != image.dimensions() {
                return Err(invalid("OCR coordinates do not match the source raster."));
            }
            let (width, height) = image.dimensions();
            let mut budget = u64::from(width) * u64::from(height) * 2;
            let mut start = 0;
            while start < result.lines.len() {
                let mut end = start + 1;
                while end < result.lines.len()
                    && result.lines[end].group == result.lines[start].group
                {
                    end += 1;
                }
                let lines = &result.lines[start..end];
                let region = region(member, image, inclusion, lines, start, end, &mut budget)?;
                let mut decision = filter::decide(
                    region.id.clone(),
                    lines,
                    region.bounds,
                    height,
                    settings.include_interface,
                    settings.overrides.get(&region.id).copied(),
                );
                let included = region.bounds.map_or(member.inclusion.is_none(), |bounds| {
                    contains(inclusion, bounds)
                });
                if !included {
                    decision.role = RegionRole::OutsideSelection;
                    decision.reason = "This complete OCR group is outside or intersects the source inclusion boundary.".to_string();
                } else if !decision.admitted()
                    && decision.override_action.is_none()
                    && region.bounds.is_some_and(|bounds| {
                        result.lines.iter().enumerate().any(|(index, line)| {
                            !(start..end).contains(&index)
                                && line.bounds.is_some_and(|other| overlaps(bounds, other))
                        })
                    })
                {
                    decision.role = RegionRole::Unresolved;
                    decision.reason =
                        "The proposed exclusion overlaps other source text, so it is retained."
                            .to_string();
                }
                if matches!(
                    decision.role,
                    RegionRole::Unresolved | RegionRole::OutsideSelection
                ) || (!decision.admitted() && region.bounds.is_none())
                {
                    frame.unresolved += 1;
                }
                if decision.admitted() {
                    frame.regions.push(region);
                } else if let Some(bounds) = region.bounds {
                    // Mask the entire excluded group, including a cropped qualifier.
                    // This prevents a partial statement re-entering through pixels.
                    frame.masked.push(bounds);
                } else {
                    frame.withhold_visual = true;
                }
                frame.decisions.push(decision);
                start = end;
            }
        }
        let identity = serde_json::to_vec(&(
            "image-frame-v2-filter",
            &member.pixel_sha256,
            &member.pixel_contract,
            member.quarter_turns,
            inclusion,
            &frame.masked,
            frame.withhold_visual,
        ))
        .map_err(CommandError::serialization)?;
        frame.identity = sha256_hex(&identity);
        Ok(frame)
    }
}

fn overlaps(a: PixelBounds, b: PixelBounds) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
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

#[cfg(test)]
mod counting_tests {
    use super::*;
    use crate::documents::image_sets::{filter::RegionOverride, raster, test_support::Fixture};

    #[test]
    fn document_image_unlocated_exclusions_count_regions_not_withheld_attachments() {
        let fixture = Fixture::new();
        let manifest = fixture.import(1, 0, None);
        let member = &manifest.members[0];
        let raster = Raster::read(&fixture.paths, member).unwrap();
        let inclusion = raster::inclusion(member).unwrap();
        let result = RegionOcrResult {
            version: 1,
            text: "First unread position\nSecond unread position".to_string(),
            confidence: 0.9,
            lines: ["First unread position", "Second unread position"]
                .into_iter()
                .enumerate()
                .map(|(index, text)| OcrLine {
                    text: text.to_string(),
                    bounds: None,
                    group: index.to_string(),
                    confidence: Some(0.9),
                })
                .collect(),
            duration_millis: 1,
            engine: "fixture".to_string(),
            language: "eng".to_string(),
            width: 8,
            height: 6,
        };
        let retained = Frame::build(member, &raster, inclusion, Some(&result)).unwrap();
        assert_eq!(retained.unresolved, 2);
        assert_eq!(retained.regions.len(), 2);
        assert!(!retained.withhold_visual);
        let mut settings = FilterSettings::default();
        for region in &retained.decisions {
            settings
                .overrides
                .insert(region.id.clone(), RegionOverride::Exclude);
        }
        let excluded =
            Frame::with_settings(member, &raster, inclusion, Some(&result), &settings).unwrap();
        assert_eq!(excluded.unresolved, 2);
        assert!(excluded.regions.is_empty());
        assert!(excluded.withhold_visual);
        assert_eq!(excluded.decisions.len(), 2);
        assert_eq!(excluded.decisions[0].text, "First unread position");
        assert_eq!(excluded.decisions[1].text, "Second unread position");
    }
}
