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
use super::{failure, source_id};
use crate::{
    assets::{detect_managed_asset_bytes, ManagedAssetKind},
    error::CommandError,
};

const MAX_ARCHIVE_BYTES: u64 = 640 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 768 * 1024 * 1024;
const MAX_COLLECTION_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MEDIA_MAP_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SINGLE_MEDIA_BYTES: usize = 25 * 1024 * 1024;
const MAX_ASSET_BYTES: usize = 512 * 1024 * 1024;
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

    /// Resolves one media-map filename into an application-owned native node.
    /// The HTML converter, not the archive reader, enforces whether the source
    /// context expected image/audio/video.
    pub fn image(&mut self, filename: &str) -> Result<Value, String> {
        let filename = if self.media.contains_key(filename) {
            filename.to_string()
        } else {
            decode_filename(filename)?
        };
        let entry = self
            .media
            .get(&filename)
            .cloned()
            .ok_or("A referenced packaged media file is missing or external.")?;
        let id = source_id("asset", &entry);
        if !self.assets.contains_key(&id) {
            let bytes = self
                .read_entry(&entry, MAX_SINGLE_MEDIA_BYTES as u64)
                .map_err(|_| "An Anki media file is corrupt or exceeds its 25 MiB size limit.")?;
            if bytes.len() > MAX_ASSET_BYTES.saturating_sub(self.asset_bytes) {
                return Err(
                    "Imported Anki media exceed the 512 MiB in-memory conversion limit.".into(),
                );
            }
            let detected = detect_managed_asset_bytes(&bytes, &filename).map_err(|_| {
                "An Anki media file uses an unsupported image, audio, video, or container format."
                    .to_string()
            })?;
            self.asset_bytes += bytes.len();
            self.assets.insert(
                id.clone(),
                PackageAsset {
                    source_id: id.clone(),
                    original_filename: format!("anki-{entry}.{}", detected.extension),
                    mime_type: detected.mime_type.to_string(),
                    byte_size: bytes.len() as i64,
                    data: bytes,
                },
            );
        }
        let asset = self.assets.get(&id).expect("asset was inserted");
        let src = format!("study-asset://{id}");
        Ok(if asset.mime_type.starts_with("image/") {
            json!({"type": "image", "attrs": {"assetId": id, "src": src}})
        } else if asset.mime_type.starts_with("audio/") {
            json!({
                "type": "audio",
                "attrs": {"assetId": id, "src": src, "title": filename}
            })
        } else if asset.mime_type.starts_with("video/") {
            json!({
                "type": "video",
                "attrs": {"assetId": id, "src": src, "title": filename, "width": null, "height": null}
            })
        } else {
            return Err("An imported media asset has an unsupported MIME type.".into());
        })
    }

    pub fn take_assets(self, used: &BTreeSet<String>) -> Vec<PackageAsset> {
        self.assets
            .into_values()
            .filter(|asset| used.contains(&asset.source_id))
            .collect()
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
                return Err("A media filename contains invalid URL encoding.".into());
            };
            decoded.push((high * 16 + low) as u8);
            position += 3;
        } else {
            decoded.push(bytes[position]);
            position += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "A media filename is not UTF-8.".into())
}
