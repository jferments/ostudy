//! Synthetic source fixtures; never touch an installed learner library.

use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use rusqlite::params;

use crate::{
    database::Database, document_canonical::sha256_hex, error::CommandError, paths::AppPaths,
};

use super::{
    import::{self, ImageSelectionInput, ImportImageSetInput},
    manifest::{ImageManifest, InclusionCrop},
    repository,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture {
    pub root: PathBuf,
    pub paths: AppPaths,
    pub database: Database,
    sequence: Cell<u64>,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-image-processing-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        Self {
            root,
            paths,
            database: Database::open_in_memory().unwrap(),
            sequence: Cell::new(1),
        }
    }

    fn id(&self) -> String {
        let id = self.sequence.get();
        self.sequence.set(id + 1);
        format!("{id:032x}")
    }

    pub(crate) fn import(
        &self,
        count: usize,
        turns: u8,
        crop: Option<InclusionCrop>,
    ) -> ImageManifest {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&[17_u8; 8 * 6 * 4], 8, 6, ExtendedColorType::Rgba8)
            .unwrap();
        let mut members = Vec::new();
        for _ in 0..count {
            let id = self.id();
            let path = AppPaths::staged_import_relative_path(&id, "png");
            fs::write(self.paths.resolve_relative(&path), &bytes).unwrap();
            self.database
                .with_connection(|connection| {
                    connection
                        .execute(
                            "INSERT INTO staged_external_imports (
                         id, operation_id, purpose, detected_kind, display_name, media_type,
                         byte_size, sha256, relative_path, state, result_json,
                         created_at, updated_at, expires_at
                     ) VALUES (?1, ?1, 'image', 'png', 'fixture.png', 'image/png',
                               ?2, ?3, ?4, 'staged', '', 1, 1, 9223372036854775807)",
                            params![id, bytes.len() as i64, sha256_hex(&bytes), path],
                        )
                        .map_err(CommandError::database)?;
                    Ok(())
                })
                .unwrap();
            members.push(ImageSelectionInput {
                staged_file_id: id,
                quarter_turns: turns,
                inclusion: crop,
            });
        }
        let result = import::import(
            &self.database,
            &self.paths,
            &ImportImageSetInput {
                operation_id: self.id(),
                title: "Synthetic image source".to_string(),
                members,
            },
        )
        .unwrap();
        self.database
            .with_connection(|connection| repository::load(connection, &result.document_id))
            .unwrap()
    }

    pub(crate) fn count(&self, table: &str) -> i64 {
        self.database
            .with_connection(|connection| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)
            })
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
