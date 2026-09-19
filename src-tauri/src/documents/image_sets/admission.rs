//! Bounded read-only admission of an ordered set of opaque staged images.
//! No OCR, provider calls, source publication, or changes to staged-file leases.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::PathBuf,
};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    cards::questions::occlusion::{CanonicalImage, MAX_IMAGE_BYTES},
    database::Database,
    document_canonical::sha256_hex,
    error::CommandError,
    external_files::{self, ExternalFileKind, ExternalImportPurpose},
    paths::AppPaths,
    util::validate_entity_id,
};

pub(crate) const MAX_MEMBERS: usize = 128;
pub(crate) const MAX_SET_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_SET_PIXELS: u64 = 256_000_000;
pub(crate) const PIXEL_CONTRACT: &str = "image-0.25.10:exif-upright-rgba8:v1";
const ALLOWED_IMAGES: &[ExternalFileKind] = &[
    ExternalFileKind::Png,
    ExternalFileKind::Jpeg,
    ExternalFileKind::Webp,
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectImageSetInput {
    pub staged_file_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedImage {
    pub original_filename: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub pixel_sha256: String,
    /// Region coordinates use these EXIF-upright pixels, not raw JPEG storage.
    pub pixel_contract: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSelectionInspection {
    pub id: String,
    pub image: Option<InspectedImage>,
    pub error: Option<SelectionIssue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSetInspection {
    pub maximum_count: usize,
    pub maximum_set_bytes: u64,
    pub maximum_set_pixels: u64,
    pub members: Vec<ImageSelectionInspection>,
    pub can_import: bool,
    pub decoded_images: usize,
    pub reused_images: usize,
}

pub(crate) fn validate_selection(ids: &[String]) -> Result<(), CommandError> {
    if ids.is_empty() || ids.len() > MAX_MEMBERS {
        return Err(invalid("Select between 1 and 128 images for one document."));
    }
    let mut unique = HashSet::new();
    for id in ids {
        validate_entity_id(id, "image selection")?;
        if !unique.insert(id) {
            return Err(invalid("An image selection identity cannot appear twice."));
        }
    }
    Ok(())
}

struct StagedImage {
    path: PathBuf,
    name: String,
    kind: ExternalFileKind,
    bytes: u64,
    sha256: String,
}

fn resolve(database: &Database, paths: &AppPaths, id: &str) -> Result<StagedImage, CommandError> {
    database.with_connection(|connection| {
        let row: Option<(String, String, i64, String)> = connection
            .query_row(
                "SELECT state, detected_kind, byte_size, sha256
                 FROM staged_external_imports WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let (state, kind, bytes, sha256) = row.ok_or_else(|| {
            invalid("This image selection is no longer available. Select it again.")
        })?;
        if state != "staged" {
            return Err(invalid("This image selection was already used or released."));
        }
        let bytes = u64::try_from(bytes).map_err(|_| invalid("Invalid image size."))?;
        if bytes == 0 || bytes > MAX_IMAGE_BYTES as u64 {
            return Err(invalid("Use a nonempty still image no larger than 25 MB."));
        }
        let kind = ExternalFileKind::parse(&kind)?;
        let staged = external_files::resolve_staged_import(
            connection,
            paths,
            id,
            ExternalImportPurpose::Image,
            ALLOWED_IMAGES,
        )?;
        Ok(StagedImage {
            path: staged.path,
            name: staged.display_name,
            kind,
            bytes,
            sha256,
        })
    })
}

#[derive(Default)]
struct Budget {
    bytes: u64,
    pixels: u64,
}

impl Budget {
    fn include(&mut self, bytes: u64, pixels: u64) -> Result<(), CommandError> {
        if bytes > MAX_SET_BYTES.saturating_sub(self.bytes)
            || pixels > MAX_SET_PIXELS.saturating_sub(self.pixels)
        {
            return Err(CommandError::new(
                "image_set_budget_exceeded",
                "The set exceeds 512 MB or 256 million decoded pixels. Remove images or split it into smaller sets.",
            ));
        }
        self.bytes += bytes;
        self.pixels += pixels;
        Ok(())
    }
}

pub(crate) fn inspect(
    database: &Database,
    paths: &AppPaths,
    ids: &[String],
) -> Result<ImageSetInspection, CommandError> {
    validate_selection(ids)?;
    let mut budget = Budget::default();
    let mut cache = HashMap::<String, InspectedImage>::new();
    let mut decoded_images = 0;
    let mut reused_images = 0;
    let mut members = Vec::with_capacity(ids.len());
    let mut exhausted = false;
    for id in ids {
        let result: Result<InspectedImage, CommandError> = (|| {
            if exhausted {
                return Err(CommandError::new(
                    "image_set_budget_exceeded",
                    "This selection was not inspected because the set reached its work budget. Remove images and inspect the remaining set again.",
                ));
            }
            let staged = resolve(database, paths, id)?;
            // Recheck the bytes read OUTSIDE the SQL lock against the admitted
            // ledger snapshot. Releasing/replacing a file cannot swap evidence.
            let mut bytes = Vec::new();
            File::open(&staged.path)
                .map_err(|_| unreadable())?
                .take(MAX_IMAGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| unreadable())?;
            if bytes.len() as u64 != staged.bytes || sha256_hex(&bytes) != staged.sha256 {
                return Err(invalid("The staged image changed. Select the original again."));
            }
            let actual_format = image::guess_format(&bytes).map_err(|_| unreadable())?;
            let expected_format = match staged.kind {
                ExternalFileKind::Png => image::ImageFormat::Png,
                ExternalFileKind::Jpeg => image::ImageFormat::Jpeg,
                ExternalFileKind::Webp => image::ImageFormat::WebP,
                _ => return Err(invalid("Select a still PNG, JPEG or WebP image.")),
            };
            if actual_format != expected_format {
                return Err(invalid("The image bytes do not match the admitted file format."));
            }
            let image = if let Some(previous) = cache.get(&staged.sha256) {
                reused_images += 1;
                InspectedImage {
                    original_filename: staged.name,
                    ..previous.clone()
                }
            } else {
                // Reuse the existing native still-image admission, including
                // animation refusal, bounded decoding and EXIF normalization.
                // No question, masking, answer or scheduler behavior is invoked.
                let decoded = CanonicalImage::decode(&bytes)?;
                decoded_images += 1;
                let (width, height) = decoded.dimensions();
                let image = InspectedImage {
                    original_filename: staged.name,
                    media_type: staged.kind.media_type().to_string(),
                    byte_size: staged.bytes,
                    sha256: staged.sha256,
                    width,
                    height,
                    pixel_sha256: decoded.fingerprint().to_string(),
                    pixel_contract: PIXEL_CONTRACT,
                };
                cache.insert(image.sha256.clone(), image.clone());
                image
            };
            budget.include(
                image.byte_size,
                u64::from(image.width) * u64::from(image.height),
            )?;
            Ok(image)
        })();
        if result
            .as_ref()
            .is_err_and(|error| error.code == "image_set_budget_exceeded")
        {
            exhausted = true;
        }
        let (image, error) = match result {
            Ok(image) => (Some(image), None),
            Err(error) => (
                None,
                Some(SelectionIssue {
                    code: error.code,
                    message: error.message,
                }),
            ),
        };
        members.push(ImageSelectionInspection {
            id: id.clone(),
            image,
            error,
        });
    }
    Ok(ImageSetInspection {
        maximum_count: MAX_MEMBERS,
        maximum_set_bytes: MAX_SET_BYTES,
        maximum_set_pixels: MAX_SET_PIXELS,
        can_import: members.iter().all(|member| member.image.is_some()),
        members,
        decoded_images,
        reused_images,
    })
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_image_set", message)
}

fn unreadable() -> CommandError {
    invalid("The selected image could not be read. Select a supported original image again.")
}

#[cfg(test)]
mod tests;
