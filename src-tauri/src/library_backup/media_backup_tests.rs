use std::fs;

use rusqlite::params;

use super::*;

fn pcm_wav() -> Vec<u8> {
    let samples = [0_i16, 512, -512, 0];
    let data_bytes = samples.len() * std::mem::size_of::<i16>();
    let mut bytes = Vec::with_capacity(44 + data_bytes);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36_u32 + data_bytes as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&(44_100_u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[test]
fn complete_backup_round_trip_preserves_managed_audio_asset() {
    let paths = AppPaths::new(std::env::temp_dir().join(format!(
        "ostudy-backup-media-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock")
    )))
    .expect("paths should initialize");
    let database = Database::open(paths.database_path()).expect("database should initialize");
    let asset_id = "b".repeat(32);
    let wav = pcm_wav();
    let relative_path = AppPaths::image_relative_path(&asset_id, "wav");
    let asset_path = paths.resolve_relative(&relative_path);
    fs::create_dir_all(asset_path.parent().unwrap()).unwrap();
    fs::write(&asset_path, &wav).unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size,
                         relative_path, created_at, lifecycle_state,
                         ready_at, verified_at, updated_at
                     ) VALUES (?1, 'voice.wav', 'audio/wav', ?2, ?3, 10, 'ready', 10, 10, 10)",
                    params![asset_id, wav.len() as i64, relative_path],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();

    let destination = paths.app_data_dir().join("media.ostudybackup");
    let exported = export_backup(&database, &paths, &destination).expect("backup should export");
    assert_eq!(exported.asset_count, 1);
    let summary = validate_backup_package(&destination, None).expect("backup should validate");
    assert_eq!(summary.asset_count, 1);

    let restore_root = paths.app_data_dir().join("restore");
    let restored_database = restore_root.join("library.sqlite3");
    let restored_assets = restore_root.join("assets");
    let restored = extract_backup_package(&destination, &restored_database, &restored_assets)
        .expect("backup should extract");
    assert_eq!(restored.asset_count, 1);
    assert_eq!(fs::read(restore_root.join(&relative_path)).unwrap(), wav);

    let _ = fs::remove_dir_all(paths.app_data_dir());
}
