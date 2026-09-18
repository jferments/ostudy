//! One bounded native-only decoded source. Authorization and file reads still
//! happen for every request; a cache entry is never a source-asset capability.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::cards::questions::{occlusion::CanonicalImage, QuestionError};

const MAX_BYTES: u64 = 32 * 1024 * 1024;
const LIFETIME: Duration = Duration::from_secs(120);
static CACHE: OnceLock<Mutex<DecodedCache>> = OnceLock::new();

#[derive(Default)]
struct DecodedCache {
    entry: Option<([u8; 32], Instant, Arc<CanonicalImage>)>,
}

impl DecodedCache {
    fn get(&mut self, key: &[u8; 32]) -> Option<Arc<CanonicalImage>> {
        if let Some((stored, time, image)) = &self.entry {
            if stored == key && time.elapsed() < LIFETIME {
                return Some(image.clone());
            }
        }
        self.entry = None;
        None
    }

    fn remember(&mut self, key: [u8; 32], image: Arc<CanonicalImage>) {
        let (width, height) = image.dimensions();
        self.entry = if u64::from(width) * u64::from(height) * 4 <= MAX_BYTES {
            Some((key, Instant::now(), image))
        } else {
            None
        };
    }
}

pub(super) fn decode(source: &[u8]) -> Result<Arc<CanonicalImage>, QuestionError> {
    let key: [u8; 32] = Sha256::digest(source).into();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(mut cache) = cache.lock() {
        if let Some(image) = cache.get(&key) {
            return Ok(image);
        }
    }
    // No cache mutex or SQLite lock is held during decoding.
    let image = Arc::new(CanonicalImage::decode(source)?);
    if let Ok(mut cache) = cache.lock() {
        cache.remember(key, image.clone());
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::io::Cursor;

    #[test]
    fn decoded_source_is_reused_but_never_by_identity_alone() {
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(2, 2))
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let image = Arc::new(CanonicalImage::decode(png.get_ref()).unwrap());
        let mut cache = DecodedCache::default();
        cache.remember([1; 32], image.clone());
        assert!(Arc::ptr_eq(&cache.get(&[1; 32]).unwrap(), &image));
        assert!(cache.get(&[2; 32]).is_none());
        assert!(cache.entry.is_none());
        cache.remember([1; 32], image);
        cache.entry.as_mut().unwrap().1 = Instant::now() - LIFETIME;
        assert!(cache.get(&[1; 32]).is_none());
    }
}
