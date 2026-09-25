//! Native source inspection exposes controlled views, never arbitrary paths.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{database::Database, error::CommandError, paths::AppPaths, util::validate_entity_id};

use super::{
    manifest::InclusionCrop,
    raster::Raster,
    receipt::{invalid, MemberReceipt},
    repository, storage,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSetDetails {
    pub document_id: String,
    pub title: String,
    pub revision: i64,
    pub filter_revision: i64,
    pub include_interface: bool,
    pub prepared: bool,
    pub members: Vec<ImageMemberDetails>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMemberDetails {
    pub id: String,
    pub original_filename: String,
    pub media_type: String,
    pub byte_size: u64,
    pub width: u32,
    pub height: u32,
    pub quarter_turns: u8,
    pub inclusion: Option<InclusionCrop>,
    pub processing: Option<MemberReceipt>,
}

pub(crate) fn details(
    connection: &Connection,
    document_id: &str,
) -> Result<ImageSetDetails, CommandError> {
    validate_entity_id(document_id, "image document")?;
    let manifest = repository::load(connection, document_id)?;
    let settings = storage::settings(connection, document_id)?;
    let receipt = storage::load(connection, document_id)?;
    let title = connection
        .query_row(
            "SELECT title FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let members = manifest
        .members
        .iter()
        .map(|member| ImageMemberDetails {
            id: member.id.clone(),
            original_filename: member.original_filename.clone(),
            media_type: member.media_type.clone(),
            byte_size: member.byte_size,
            width: member.width,
            height: member.height,
            quarter_turns: member.quarter_turns,
            inclusion: member.inclusion,
            processing: receipt
                .as_ref()
                .and_then(|receipt| {
                    receipt
                        .members
                        .iter()
                        .find(|saved| saved.member_id == member.id)
                })
                .cloned(),
        })
        .collect();
    Ok(ImageSetDetails {
        document_id: document_id.to_string(),
        title,
        revision: manifest.revision,
        filter_revision: settings.revision,
        include_interface: settings.include_interface,
        prepared: storage::ready(connection, document_id)?,
        members,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSetInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImagePreviewInput {
    pub document_id: String,
    pub member_id: String,
    pub expected_revision: i64,
    /// Display-only viewport in the rotated upright image. Not source inclusion.
    pub viewport: Option<InclusionCrop>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreview {
    pub revision: i64,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

pub(crate) fn preview(
    database: &Database,
    paths: &AppPaths,
    input: &ImagePreviewInput,
) -> Result<ImagePreview, CommandError> {
    validate_entity_id(&input.document_id, "image document")?;
    validate_entity_id(&input.member_id, "image member")?;
    let manifest =
        database.with_connection(|connection| repository::load(connection, &input.document_id))?;
    if manifest.revision != input.expected_revision {
        return Err(invalid(
            "The source changed before its image could be opened.",
        ));
    }
    let member = manifest
        .members
        .iter()
        .find(|member| member.id == input.member_id)
        .ok_or_else(|| invalid("That image does not belong to this document."))?;
    let raster = Raster::read(paths, member)?;
    let pixels = image::load_from_memory_with_format(&raster.png()?, image::ImageFormat::Png)
        .map_err(|_| invalid("The retained image view could not be decoded."))?
        .into_rgba8();
    let viewport = input.viewport.unwrap_or(InclusionCrop {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    });
    viewport.validate()?;
    let left = (viewport.left * f64::from(pixels.width())).floor() as u32;
    let top = (viewport.top * f64::from(pixels.height())).floor() as u32;
    let right = (viewport.right * f64::from(pixels.width())).ceil() as u32;
    let bottom = (viewport.bottom * f64::from(pixels.height())).ceil() as u32;
    if left >= right || top >= bottom || right > pixels.width() || bottom > pixels.height() {
        return Err(invalid("The display viewport has no source pixels."));
    }
    let mut view =
        image::imageops::crop_imm(&pixels, left, top, right - left, bottom - top).to_image();
    let edge = view.width().max(view.height());
    if edge > 2048 {
        view = image::imageops::resize(
            &view,
            (u64::from(view.width()) * 2048 / u64::from(edge)).max(1) as u32,
            (u64::from(view.height()) * 2048 / u64::from(edge)).max(1) as u32,
            image::imageops::FilterType::Triangle,
        );
    }
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            view.as_raw(),
            view.width(),
            view.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| invalid("The source view could not be encoded."))?;
    Ok(ImagePreview {
        revision: manifest.revision,
        width: view.width(),
        height: view.height(),
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
    })
}
