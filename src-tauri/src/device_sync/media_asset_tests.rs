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
fn device_sync_round_trip_preserves_managed_audio_asset() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-device-sync-media-{}-{}",
        std::process::id(),
        crate::platform::now_utc_millis().unwrap_or_default()
    ));
    let source_root = root.join("source");
    let extracted_root = root.join("extracted");
    let installed_root = root.join("installed");
    let source_paths = AppPaths::new(source_root).expect("source paths should initialize");
    let database = Database::open_in_memory().expect("database should initialize");
    let asset_id = "a".repeat(32);
    let wav = pcm_wav();
    let relative_path = AppPaths::image_relative_path(&asset_id, "wav");
    let source_path = source_paths.resolve_relative(&relative_path);
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    fs::write(&source_path, &wav).unwrap();
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

    let package_path = root.join("media.syncpkg");
    let selection = SyncSelection {
        flashcards: true,
        documents: false,
        study_guides: false,
        notes: false,
        lessons: false,
        api_settings: false,
        api_credentials: false,
    };
    let summary = create_package(&database, &source_paths, &package_path, selection)
        .expect("audio package should build");
    assert_eq!(summary.asset_count, 1);
    assert_eq!(summary.asset_bytes, wav.len() as u64);

    let extracted = extract_package(&package_path, &extracted_root)
        .expect("audio package should validate and extract");
    assert_eq!(extracted.assets.len(), 1);
    assert_eq!(extracted.assets[0].mime_type, "audio/wav");
    assert_eq!(fs::read(&extracted.assets[0].staged_path).unwrap(), wav);

    let installed_paths = AppPaths::new(installed_root).expect("installed paths should initialize");
    install_assets(&installed_paths, &extracted.assets).expect("audio asset should install");
    assert_eq!(
        fs::read(installed_paths.resolve_relative(&relative_path)).unwrap(),
        pcm_wav()
    );

    let _ = fs::remove_dir_all(root);
}
