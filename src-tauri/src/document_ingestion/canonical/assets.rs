use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use crate::{
    document_canonical::{CanonicalAsset, CanonicalAssetRole},
    error::CommandError,
    paths::AppPaths,
};

use crate::document_ingestion::support::{normalize_semantic_text, sha256_hex};

const MAX_ASSET_BYTES: usize = 32 * 1024 * 1024;
const MAX_RETAINED_ASSET_BYTES: usize = 128 * 1024 * 1024;

pub(crate) struct ManagedAssetInput<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) media_type: &'a str,
    pub(crate) role: CanonicalAssetRole,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) alt_text: Option<&'a str>,
    pub(crate) source_unit_number: Option<i64>,
}

pub(crate) struct ManagedAssetStore<'a> {
    paths: &'a AppPaths,
    document_id: &'a str,
    total_bytes: usize,
    next_ordinal: usize,
}

impl<'a> ManagedAssetStore<'a> {
    pub(crate) fn new(paths: &'a AppPaths, document_id: &'a str) -> Result<Self, CommandError> {
        fs::create_dir_all(paths.document_assets_dir(document_id)).map_err(CommandError::io)?;
        Ok(Self {
            paths,
            document_id,
            total_bytes: 0,
            next_ordinal: 1,
        })
    }

    pub(crate) fn persist(
        &mut self,
        input: ManagedAssetInput<'_>,
    ) -> Result<CanonicalAsset, CommandError> {
        if input.bytes.is_empty() || input.bytes.len() > MAX_ASSET_BYTES {
            return Err(CommandError::new(
                "document_asset_too_large",
                "An embedded document image is empty or exceeds oStudy's 32 MB asset limit.",
            ));
        }
        self.total_bytes = self.total_bytes.saturating_add(input.bytes.len());
        if self.total_bytes > MAX_RETAINED_ASSET_BYTES {
            return Err(CommandError::new(
                "document_assets_too_large",
                "The document's retained embedded images exceed oStudy's 128 MB limit.",
            ));
        }

        let ordinal = self.next_ordinal;
        self.next_ordinal = self.next_ordinal.saturating_add(1);
        let sha256 = sha256_hex(input.bytes);
        let relative_path = AppPaths::document_versioned_asset_relative_path(
            self.document_id,
            ordinal,
            asset_content_fingerprint(&sha256),
            image_extension(input.media_type),
        );
        write_immutable_file(&self.paths.resolve_relative(&relative_path), input.bytes)?;

        Ok(CanonicalAsset {
            asset_id: format!("source-asset:{ordinal:06}"),
            source_unit_number: input.source_unit_number,
            block_index: None,
            role: input.role,
            media_type: normalize_semantic_text(input.media_type, 160),
            relative_path,
            byte_size: i64::try_from(input.bytes.len()).unwrap_or(i64::MAX),
            sha256,
            width: input.width.map(i64::from),
            height: input.height.map(i64::from),
            alt_text: input
                .alt_text
                .map(|value| normalize_semantic_text(value, 2_000))
                .filter(|value| !value.is_empty()),
        })
    }
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        _ => "bin",
    }
}

fn asset_content_fingerprint(sha256: &str) -> u64 {
    sha256
        .get(..16)
        .and_then(|value| u64::from_str_radix(value, 16).ok())
        .unwrap_or_default()
}

fn write_immutable_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_asset_path",
            "The document asset path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    if destination.exists() {
        let existing = fs::read(destination).map_err(CommandError::io)?;
        if existing == bytes {
            return Ok(());
        }
        return Err(CommandError::new(
            "document_asset_identity_collision",
            "An embedded document image conflicted with an existing immutable asset path.",
        ));
    }

    let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}
