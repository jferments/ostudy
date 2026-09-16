//! Bounded, process-local derivative cache. It owns no canonical files or SQLite
//! rows and cannot authorize access. Every caller rechecks its session and source.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};

use sha2::{Digest, Sha256};

use crate::cards::questions::{
    occlusion::{RenderPhase, RENDER_POLICY_VERSION},
    QuestionInstance,
};

const MAX_BYTES: usize = 24 * 1024 * 1024;
const MAX_ENTRIES: usize = 6;
static CACHE: OnceLock<Mutex<RasterCache>> = OnceLock::new();

#[derive(Default)]
struct RasterCache {
    entries: VecDeque<(String, Arc<Vec<u8>>)>,
    bytes: usize,
}

impl RasterCache {
    fn get(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let position = self.entries.iter().position(|entry| entry.0 == key)?;
        let entry = self.entries.remove(position)?;
        let bytes = entry.1.clone();
        self.entries.push_back(entry);
        Some(bytes)
    }

    fn insert(&mut self, key: String, bytes: Arc<Vec<u8>>) {
        if bytes.len() > MAX_BYTES || self.entries.iter().any(|entry| entry.0 == key) {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes.len() > MAX_BYTES {
            let Some((_, removed)) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= removed.len();
        }
        self.bytes += bytes.len();
        self.entries.push_back((key, bytes));
    }
}

pub(super) fn key(
    instance: &QuestionInstance,
    source: &[u8],
    phase: RenderPhase,
    maximum_edge: u32,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"ostudy-practice-occlusion\0");
    hash.update(RENDER_POLICY_VERSION.to_le_bytes());
    hash.update(instance.fingerprint().as_bytes());
    // A restored library or another library can reuse identities. Compressed
    // source bytes are hashed before cache lookup, not assumed equal by path/ID.
    hash.update(Sha256::digest(source));
    hash.update([u8::from(phase == RenderPhase::Revealed)]);
    hash.update(maximum_edge.to_le_bytes());
    format!("{:x}", hash.finalize())
}

pub(super) fn get(key: &str) -> Option<Arc<Vec<u8>>> {
    CACHE.get_or_init(Default::default).lock().ok()?.get(key)
}

pub(super) fn insert(key: String, bytes: Arc<Vec<u8>>) {
    if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
        cache.insert(key, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_is_bounded_and_preserves_recently_used_entries() {
        let mut cache = RasterCache::default();
        for n in 0..MAX_ENTRIES {
            cache.insert(n.to_string(), Arc::new(vec![n as u8; 5]));
        }
        assert!(cache.get("0").is_some());
        cache.insert("new".to_string(), Arc::new(vec![9; 5]));
        assert!(cache.get("0").is_some());
        assert!(cache.get("1").is_none());
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert_eq!(cache.bytes, MAX_ENTRIES * 5);
    }
}
