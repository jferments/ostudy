//! Bounded APKG archive and media access. No archive path is ever extracted.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

use serde::de::{MapAccess, Visitor};
use serde::Deserializer;
use serde_json::{json, Value};
use zip::ZipArchive;

use super::super::PackageAsset;
use super::{
    failure,
    media_markup::{AUDIO_PREFIX, SOUND_PREFIX, VIDEO_PREFIX},
    source_id,
};
use crate::{
    error::CommandError,
    media::{inspect_media_bytes, MediaKind},
};

const MAX_ARCHIVE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 768 * 1024 * 1024;
const MAX_COLLECTION_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MEDIA_MAP_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 25 * 1024 * 1024;
const MAX_ASSETS_BYTES: usize = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_004;

pub(super) const COMPATIBILITY_EXPORT: &str = "This Anki package version is not supported. In Anki, export an Anki Deck Package (.apkg) with Support older Anki versions and Include media enabled; then import that file.";

pub(super) struct Archive {
    zip: ZipArchive<File>,
    entries: BTreeMap<String, usize>,
    media: BTreeMap<String, String>,
    collection_name: String,
    assets: BTreeMap<String, PackageAsset>,
    asset_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedMediaKind {
    Any,
    Audio,
    Video,
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self, CommandError> {
        let file = File::open(path).map_err(CommandError::io)?;
        if file.metadata().map_err(CommandError::io)?.len() > MAX_ARCHIVE_BYTES {
            return Err(failure("Anki package exceeds the 640 MiB limit."));
        }
        let mut zip = ZipArchive::new(file).map_err(|_| failure("Invalid Anki ZIP package."))?;
        if zip.len() > MAX_ENTRIES {
            return Err(failure("Anki package contains too many archive entries."));
        }
        let mut entries = BTreeMap::new();
        let mut expanded = 0_u64;
        for index in 0..zip.len() {
            let entry = zip
                .by_index(index)
                .map_err(|_| failure("Unreadable or encrypted Anki archive entry."))?;
            let name = entry.name();
            let valid_name = matches!(
                name,
                "collection.anki2" | "collection.anki21" | "collection.anki21b" | "media" | "meta"
            ) || (!name.is_empty()
                && name.len() <= 20
                && name.bytes().all(|byte| byte.is_ascii_digit()));
            if !valid_name
                || !entry.is_file()
                || entry.encrypted()
                || entry.enclosed_name().is_none()
                || entries.insert(name.to_string(), index).is_some()
            {
                return Err(failure(
                    "Anki archive contains an unsafe, duplicate, or unsupported entry.",
                ));
            }
            expanded = expanded
                .checked_add(entry.size())
                .ok_or_else(|| failure("Invalid archive size."))?;
            if expanded > MAX_EXPANDED_BYTES {
                return Err(failure("Expanded Anki package exceeds the 768 MiB limit."));
            }
        }
        // Modern exports can contain a dummy legacy collection. Never fall back
        // to it when the actual collection requires an unsupported decoder.
        if entries.contains_key("collection.anki21b") {
            return Err(failure(COMPATIBILITY_EXPORT));
        }
        let collection_name = if entries.contains_key("collection.anki21") {
            "collection.anki21"
        } else if entries.contains_key("collection.anki2") {
            "collection.anki2"
        } else {
            return Err(failure(
                "The archive does not contain an Anki deck collection.",
            ));
        }
        .to_string();
        let mut archive = Self {
            zip,
            entries,
            media: BTreeMap::new(),
            collection_name,
            assets: BTreeMap::new(),
            asset_bytes: 0,
        };
        if archive.entries.contains_key("meta") {
            let metadata = archive.read_entry("meta", 64)?;
            if metadata != [8, 1] && metadata != [8, 2] {
                return Err(failure(COMPATIBILITY_EXPORT));
            }
        }
        if archive.entries.contains_key("media") {
            let bytes = archive.read_entry("media", MAX_MEDIA_MAP_BYTES)?;
            let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
            let mapping = deserializer
                .deserialize_map(MediaMapVisitor)
                .map_err(|_| failure("Invalid or duplicate entries in the Anki JSON media map."))?;
            deserializer
                .end()
                .map_err(|_| failure("Unexpected data after the Anki media map."))?;
            for (entry, filename) in mapping {
                if filename.len() > 1024
                    || filename.is_empty()
                    || filename.chars().any(char::is_control)
                    || filename.contains('/')
                    || filename.contains('\\')
                    || filename == "."
                    || filename == ".."
                    || !entry.bytes().all(|byte| byte.is_ascii_digit())
                    || entry.is_empty()
                    || !archive.entries.contains_key(&entry)
                    || archive.media.insert(filename, entry).is_some()
                {
                    return Err(failure(
                        "Anki media map has unsafe, missing, or ambiguous filenames.",
                    ));
                }
            }
        }
        Ok(archive)
    }

    pub fn collection(&mut self) -> Result<Vec<u8>, CommandError> {
        let name = self.collection_name.clone();
        let bytes = self.read_entry(&name, MAX_COLLECTION_BYTES)?;
        if !bytes.starts_with(b"SQLite format 3\0") {
            return Err(failure(COMPATIBILITY_EXPORT));
        }
        Ok(bytes)
    }

    fn read_entry(&mut self, name: &str, maximum: u64) -> Result<Vec<u8>, CommandError> {
        let index = *self
            .entries
            .get(name)
            .ok_or_else(|| failure("Missing Anki archive entry."))?;
        let mut entry = self
            .zip
            .by_index(index)
            .map_err(|_| failure("Unreadable Anki archive entry."))?;
        let expected = entry.size();
        if expected > maximum {
            return Err(failure(
                "An Anki archive entry exceeds its decompressed size limit.",
            ));
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut entry)
            .take(maximum + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("Corrupt or truncated Anki archive entry."))?;
        if bytes.len() as u64 != expected || bytes.len() as u64 > maximum {
            return Err(failure(
                "Anki entry did not match its declared decompressed size.",
            ));
        }
        Ok(bytes)
    }

    pub fn asset(&mut self, reference: &str) -> Result<Value, String> {
        if let Some(encoded) = reference.strip_prefix(SOUND_PREFIX) {
            return self.media_asset(encoded, ExpectedMediaKind::Any);
        }
        if let Some(encoded) = reference.strip_prefix(AUDIO_PREFIX) {
            return self.media_asset(encoded, ExpectedMediaKind::Audio);
        }
        if let Some(encoded) = reference.strip_prefix(VIDEO_PREFIX) {
            return self.media_asset(encoded, ExpectedMediaKind::Video);
        }
        self.image(reference)
    }

    fn image(&mut self, filename: &str) -> Result<Value, String> {
        // Prefer an exact filename; percent-decoding is only a fallback for an
        // encoded HTML URL. Neither spelling is ever used as a filesystem path.
        let filename = self.resolve_filename(filename)?;
        let entry = self
            .media
            .get(&filename)
            .cloned()
            .ok_or("A referenced local image is missing, external, or unsupported.")?;
        let id = source_id("asset", &entry);
        if !self.assets.contains_key(&id) {
            let bytes = self
                .read_entry(&entry, MAX_ASSET_BYTES)
                .map_err(|_| "An image is corrupt or too large.")?;
            self.reserve_asset_bytes(bytes.len())?;
            let (mime, extension) = crate::assets::detect_image_bytes(&bytes)
                .map_err(|_| "An image uses an unsupported format; use PNG, JPEG, GIF, or WebP.")?;
            self.asset_bytes += bytes.len();
            self.assets.insert(
                id.clone(),
                PackageAsset {
                    source_id: id.clone(),
                    original_filename: format!("anki-{entry}.{extension}"),
                    mime_type: mime.to_string(),
                    byte_size: bytes.len() as i64,
                    data: bytes,
                },
            );
        }
        Ok(json!({"type": "image", "attrs": {"assetId": id, "src": format!("study-asset://{id}")}}))
    }

    fn media_asset(
        &mut self,
        encoded_filename: &str,
        expected: ExpectedMediaKind,
    ) -> Result<Value, String> {
        let filename = decode_filename(encoded_filename)?;
        let entry =
            self.media.get(&filename).cloned().ok_or(
                "A referenced local audio/video file is missing, external, or unsupported.",
            )?;
        let id = source_id("asset", &entry);
        if !self.assets.contains_key(&id) {
            let bytes = self
                .read_entry(&entry, MAX_ASSET_BYTES)
                .map_err(|_| "An Anki audio/video file is corrupt or exceeds 25 MB.")?;
            self.reserve_asset_bytes(bytes.len())?;
            let inspected = inspect_media_bytes(&bytes).map_err(|_| {
                "An Anki audio/video file uses an unsupported or invalid media format."
            })?;
            ensure_expected_kind(inspected.kind, expected)?;
            self.asset_bytes += bytes.len();
            self.assets.insert(
                id.clone(),
                PackageAsset {
                    source_id: id.clone(),
                    original_filename: format!("anki-{entry}.{}", inspected.extension),
                    mime_type: inspected.mime_type.to_string(),
                    byte_size: bytes.len() as i64,
                    data: bytes,
                },
            );
        }
        let stored = self
            .assets
            .get(&id)
            .ok_or("An imported media asset could not be prepared.")?;
        let kind = if stored.mime_type.starts_with("audio/") {
            MediaKind::Audio
        } else if stored.mime_type.starts_with("video/") {
            MediaKind::Video
        } else {
            return Err("An Anki media reference resolved to a non-media asset.".into());
        };
        ensure_expected_kind(kind, expected)?;
        let node = match kind {
            MediaKind::Audio => "audio",
            MediaKind::Video => "video",
        };
        Ok(json!({"type": node, "attrs": {"assetId": id, "src": format!("study-asset://{id}")}}))
    }

    fn resolve_filename(&self, value: &str) -> Result<String, String> {
        if self.media.contains_key(value) {
            Ok(value.to_string())
        } else {
            decode_filename(value)
        }
    }

    fn reserve_asset_bytes(&self, incoming: usize) -> Result<(), String> {
        if incoming > MAX_ASSETS_BYTES.saturating_sub(self.asset_bytes) {
            return Err("Imported Anki assets exceed the 256 MiB memory limit.".into());
        }
        Ok(())
    }

    pub fn take_assets(self, used: &BTreeSet<String>) -> Vec<PackageAsset> {
        self.assets
            .into_values()
            .filter(|asset| used.contains(&asset.source_id))
            .collect()
    }
}

fn ensure_expected_kind(kind: MediaKind, expected: ExpectedMediaKind) -> Result<(), String> {
    if matches!(expected, ExpectedMediaKind::Any)
        || matches!(
            (kind, expected),
            (MediaKind::Audio, ExpectedMediaKind::Audio)
        )
        || matches!(
            (kind, expected),
            (MediaKind::Video, ExpectedMediaKind::Video)
        )
    {
        Ok(())
    } else {
        Err("An Anki audio/video element's declared type does not match its packaged media.".into())
    }
}

struct MediaMapVisitor;

impl<'de> Visitor<'de> for MediaMapVisitor {
    type Value = BTreeMap<String, String>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an Anki filename map with unique entries")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
        let mut result = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<String, String>()? {
            if result.len() >= MAX_ENTRIES || result.insert(key, value).is_some() {
                return Err(serde::de::Error::custom(
                    "duplicate or excessive media entries",
                ));
            }
        }
        Ok(result)
    }
}

fn decode_filename(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position] == b'%' {
            let high = bytes
                .get(position + 1)
                .and_then(|byte| (*byte as char).to_digit(16));
            let low = bytes
                .get(position + 2)
                .and_then(|byte| (*byte as char).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("An Anki media filename contains invalid URL encoding.".into());
            };
            decoded.push((high * 16 + low) as u8);
            position += 3;
        } else {
            decoded.push(bytes[position]);
            position += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "An Anki media filename is not UTF-8.".into())
}
