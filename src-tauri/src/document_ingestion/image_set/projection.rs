//! Projection, not a second source representation. Originals and exact OCR
//! occurrences remain retained when verified overlap shares a learning passage.

use std::collections::BTreeMap;

use crate::{
    document_canonical::{
        sha256_hex, CanonicalAsset, CanonicalAssetRole, CanonicalBlock, CanonicalBlockRole,
        CanonicalDocument, CanonicalExtractionReport, CanonicalIngestion, CanonicalSemanticChunk,
        CanonicalSourceFile, CanonicalSourceUnit, DocumentMetadata, NormalizedBoundingBox,
        CANONICAL_DOCUMENT_SCHEMA_VERSION,
    },
    document_domain::{DocumentFormat, DocumentSourceDescriptor, SourceLocator, SourceUnitKind},
    documents::image_sets::{
        manifest::{self, ImageManifest, ImageMember, MANIFEST_MEDIA_TYPE},
        raster::{self, Raster},
    },
    error::CommandError,
    paths::AppPaths,
    pdf_ocr_backend::regions::{PixelBounds, RegionOcrResult},
};

use super::{
    overlap::{Alignment, CONTRACT},
    passages::{add_occurrence, Passages},
    regions::Frame,
    AdapterExtraction, IDENTITY,
};

pub(super) struct Projection {
    document_id: String,
    canonical: CanonicalIngestion,
    passages: Passages,
    previous: Option<Frame>,
    previous_targets: Vec<usize>,
    previous_visual: Option<usize>,
    reused: usize,
    unresolved: usize,
    duplicates: usize,
    ambiguous: usize,
    confidence_sum: f64,
    recognized: usize,
}

impl Projection {
    pub(super) fn new(manifest: &ImageManifest, bytes: &[u8]) -> Result<Self, CommandError> {
        let hash = sha256_hex(bytes);
        Ok(Self {
            document_id: manifest.document_id.clone(),
            canonical: CanonicalIngestion {
                document: CanonicalDocument {
                    schema_version: CANONICAL_DOCUMENT_SCHEMA_VERSION,
                    source: DocumentSourceDescriptor {
                        format: DocumentFormat::ImageBundle,
                        unit_kind: SourceUnitKind::Image,
                        unit_count: manifest.members.len(),
                    },
                    source_files: vec![CanonicalSourceFile {
                        file_ordinal: 1,
                        original_filename: "Ordered images".to_string(),
                        media_type: MANIFEST_MEDIA_TYPE.to_string(),
                        relative_path: manifest::manifest_path(&manifest.document_id, &hash)?,
                        byte_size: bytes.len() as i64,
                        sha256: hash,
                    }],
                    metadata: DocumentMetadata::default(),
                    units: Vec::new(),
                    blocks: Vec::new(),
                    assets: Vec::new(),
                    extraction: CanonicalExtractionReport {
                        adapter_backend: IDENTITY.adapter_backend.to_string(),
                        extraction_backend: IDENTITY.extraction_backend.to_string(),
                        render_backend: IDENTITY.render_backend.map(str::to_string),
                        extraction_mode: "ocr_fallback".to_string(),
                        semantic_analysis_degraded: false,
                        ocr_unit_count: manifest.members.len(),
                        ocr_mean_confidence: None,
                        ocr_engine: None,
                    },
                },
                semantic_chunks: Vec::new(),
            },
            passages: Passages::default(),
            previous: None,
            previous_targets: Vec::new(),
            previous_visual: None,
            reused: 0,
            unresolved: 0,
            duplicates: 0,
            ambiguous: 0,
            confidence_sum: 0.0,
            recognized: 0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn add(
        &mut self,
        paths: &AppPaths,
        member: &ImageMember,
        index: usize,
        image: &Raster,
        inclusion: PixelBounds,
        result: Option<&RegionOcrResult>,
        reused: bool,
    ) -> Result<(), CommandError> {
        let unit = index as i64 + 1;
        let label = format!("Image {unit}");
        let frame = Frame::build(member, image, inclusion, result)?;
        let alignment = Alignment::between(self.previous.as_ref(), &frame);
        self.unresolved += frame.unresolved;
        self.ambiguous += usize::from(alignment.ambiguous);
        let mut targets = Vec::new();
        let mut excluded = Vec::new();
        let document = &mut self.canonical.document;
        if let Some(result) = result {
            self.recognized += 1;
            self.confidence_sum += result.confidence;
            for (position, region) in frame.regions.iter().enumerate() {
                let target = alignment.targets[position]
                    .and_then(|previous| self.previous_targets.get(previous).copied());
                let (target, equivalent) = self.passages.admit(
                    &frame,
                    region,
                    unit,
                    target,
                    alignment.whole_image,
                );
                targets.push(target);
                if equivalent.is_some() {
                    self.duplicates += 1;
                    if !alignment.whole_image {
                        excluded.extend(region.strip);
                    }
                }
                for ordinal in region.start..region.end {
                    let line = &result.lines[ordinal];
                    let geometry = line.bounds.and_then(|bounds| {
                        NormalizedBoundingBox::new(
                            f64::from(bounds.left) / f64::from(result.width),
                            f64::from(bounds.top) / f64::from(result.height),
                            f64::from(bounds.right - bounds.left) / f64::from(result.width),
                            f64::from(bounds.bottom - bounds.top) / f64::from(result.height),
                        )
                    });
                    let mut attributes = BTreeMap::from([
                        ("imageMemberId".to_string(), member.id.clone()),
                        ("imageRegionId".to_string(), region.id.clone()),
                        ("imageFrameIdentity".to_string(), frame.identity.clone()),
                        ("ocrGroup".to_string(), line.group.clone()),
                        ("alignmentContract".to_string(), CONTRACT.to_string()),
                        (
                            "coordinateSpace".to_string(),
                            "upright-rotated-normalized".to_string(),
                        ),
                    ]);
                    if let Some(signature) = &region.signature {
                        attributes.insert("imageRegionSignature".to_string(), signature.clone());
                    }
                    if let Some(representative) = &equivalent {
                        attributes.insert(
                            "equivalentToRegionId".to_string(),
                            representative.clone(),
                        );
                    }
                    document.blocks.push(CanonicalBlock::new(
                        document.blocks.len() as i64,
                        ordinal as i64,
                        unit,
                        CanonicalBlockRole::Paragraph,
                        &line.text,
                        None,
                        Vec::new(),
                        geometry,
                        line.confidence,
                        Vec::new(),
                        attributes,
                    ));
                }
            }
        }
        let plain_text = frame
            .regions
            .iter()
            .map(|region| region.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        document.units.push(CanonicalSourceUnit::new(
            unit,
            SourceUnitKind::Image,
            &label,
            None,
            &plain_text,
            1,
        ));
        let asset_id = format!("image-member:{}", member.id);
        let (png, width, height) = image.filtered_png(inclusion, &excluded)?;
        let path = raster::retain_derivative(paths, &self.document_id, &member.id, &png)?;
        let block_index = document.blocks.len() as i64;
        let attributes = BTreeMap::from([
            ("imageMemberId".to_string(), member.id.clone()),
            ("quarterTurns".to_string(), member.quarter_turns.to_string()),
            ("pixelContract".to_string(), member.pixel_contract.clone()),
            ("alignmentContract".to_string(), CONTRACT.to_string()),
            ("ambiguousOverlap".to_string(), alignment.ambiguous.to_string()),
        ]);
        document.blocks.push(CanonicalBlock::new(
            block_index,
            result.map_or(0, |value| value.lines.len()) as i64,
            unit,
            CanonicalBlockRole::Image,
            "",
            None,
            Vec::new(),
            None,
            None,
            vec![asset_id.clone()],
            attributes,
        ));
        document.assets.push(CanonicalAsset {
            asset_id: asset_id.clone(),
            source_unit_number: Some(unit),
            block_index: Some(block_index),
            role: CanonicalAssetRole::EmbeddedImage,
            media_type: "image/png".to_string(),
            relative_path: path,
            byte_size: png.len() as i64,
            sha256: sha256_hex(&png),
            width: Some(i64::from(width)),
            height: Some(i64::from(height)),
            alt_text: Some(label),
        });
        let visual = alignment.whole_image.then_some(self.previous_visual).flatten();
        self.previous_visual = if let Some(index) = visual {
            add_occurrence(&mut self.canonical.semantic_chunks[index], unit)?;
            Some(index)
        } else {
            let index = self.canonical.semantic_chunks.len();
            let context = "[Image evidence; exact OCR passages are retained separately.]";
            let locator = SourceLocator::with_evidence(
                SourceUnitKind::Image,
                vec![unit],
                None,
                vec![asset_id],
                None,
            )?;
            self.canonical
                .semantic_chunks
                .push(CanonicalSemanticChunk::new(
                    index as i64,
                    context,
                    context,
                    locator,
                    vec!["paragraph".to_string(), "image".to_string()],
                    None,
                    context.chars().count().div_ceil(4),
                    false,
                ));
            Some(index)
        };
        self.previous = Some(frame);
        self.previous_targets = targets;
        self.reused += usize::from(reused);
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<AdapterExtraction, CommandError> {
        let document = &mut self.canonical.document;
        document.extraction.semantic_analysis_degraded = self.unresolved > 0 || self.ambiguous > 0;
        document.extraction.ocr_mean_confidence =
            (self.recognized > 0).then(|| self.confidence_sum / self.recognized as f64);
        document.extraction.ocr_engine = Some("native region OCR".to_string());
        for (key, value) in [
            ("reusedImageExtractions", self.reused),
            ("unresolvedImageRegions", self.unresolved),
            ("verifiedImageOverlapRegions", self.duplicates),
            ("ambiguousImageOverlaps", self.ambiguous),
        ] {
            document
                .metadata
                .attributes
                .insert(key.to_string(), value.to_string());
        }
        let mut chunks = self.passages.finish()?;
        chunks.append(&mut self.canonical.semantic_chunks);
        for (index, chunk) in chunks.iter_mut().enumerate() {
            chunk.chunk_index = index as i64;
        }
        self.canonical.semantic_chunks = chunks;
        self.canonical.validate()?;
        Ok(AdapterExtraction {
            identity: IDENTITY,
            canonical: self.canonical,
            compatibility: None,
        })
    }
}
