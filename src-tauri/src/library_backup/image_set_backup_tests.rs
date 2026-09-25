use super::*;

use crate::{
    document_canonical::sha256_hex,
    documents::image_sets::{
        amend::{self, AmendImageSetInput, MemberEdit},
        filter::{RegionDecision, RegionOverride, RegionRole},
        receipt::{ImageReceipt, MemberReceipt},
        storage,
        test_support::Fixture,
    },
    pdf_ocr_backend::regions::PixelBounds,
};

#[test]
fn image_set_pending_amendment_and_filters_round_trip_through_complete_backup() {
    let fixture = Fixture::new();
    let original = fixture.import(1, 0, None);
    let region_id = "f".repeat(64);
    let receipt = ImageReceipt {
        version: 1,
        document_id: original.document_id.clone(),
        manifest_sha256: original.fingerprint().unwrap(),
        filter_revision: 2,
        members: vec![MemberReceipt {
            member_id: original.members[0].id.clone(),
            width: original.members[0].width,
            height: original.members[0].height,
            ocr_available: true,
            visual_withheld: false,
            regions: vec![RegionDecision {
                id: region_id.clone(),
                text: "Exclude this exact region".to_string(),
                bounds: Some(PixelBounds {
                    left: 0,
                    top: 0,
                    right: original.members[0].width,
                    bottom: 1,
                }),
                automatic_role: RegionRole::Learning,
                role: RegionRole::Interface,
                reason: "You excluded this region from learning inputs.".to_string(),
                override_action: Some(RegionOverride::Exclude),
                equivalent_to: None,
            }],
        }],
        chunks: Vec::new(),
        source_manifest: None,
    };
    let json = serde_json::to_string(&receipt).unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_image_settings (document_id, revision, include_interface)
                     VALUES (?1, 2, 0)",
                    [&original.document_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_image_overrides (document_id, member_id, region_id, action)
                     VALUES (?1, ?2, ?3, 'exclude')",
                    params![original.document_id, original.members[0].id, region_id],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_image_receipts (
                         document_id, manifest_sha256, filter_revision, version,
                         payload_json, payload_sha256
                     ) VALUES (?1, ?2, 2, 1, ?3, ?4)",
                    params![
                        original.document_id,
                        original.fingerprint()?,
                        json,
                        sha256_hex(json.as_bytes()),
                    ],
                )
                .map_err(CommandError::database)?;
            crate::database::validate_backup_lineage(connection)
        })
        .unwrap();

    amend::apply(
        &fixture.database,
        &fixture.paths,
        &AmendImageSetInput {
            operation_id: "a".repeat(32),
            document_id: original.document_id.clone(),
            expected_revision: original.revision,
            expected_filter_revision: 2,
            members: vec![MemberEdit {
                member_id: original.members[0].id.clone(),
                quarter_turns: 1,
                inclusion: None,
            }],
            append: Vec::new(),
        },
    )
    .unwrap();
    let current = fixture
        .database
        .with_connection(|connection| {
            let current =
                crate::documents::image_sets::repository::load(connection, &original.document_id)?;
            let retained = storage::load(connection, &original.document_id)?.unwrap();
            assert_eq!(retained.source_manifest.as_ref(), Some(&original));
            assert!(!storage::ready(connection, &original.document_id)?);
            Ok(current)
        })
        .unwrap();
    assert_eq!(current.revision, original.revision + 1);
    assert_eq!(current.members[0].quarter_turns, 1);

    let backup = fixture.root.join("images.ostudybackup");
    export_backup(&fixture.database, &fixture.paths, &backup).unwrap();
    validate_backup_package(&backup, None).unwrap();

    let restored_root = fixture.root.join("restored");
    fs::create_dir_all(&restored_root).unwrap();
    let restored_database = restored_root.join("library.sqlite3");
    let restored_assets = restored_root.join("assets");
    extract_backup_package(&backup, &restored_database, &restored_assets).unwrap();

    let restored =
        Connection::open_with_flags(&restored_database, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    validate_snapshot_database(&restored).unwrap();
    let restored_manifest =
        crate::documents::image_sets::repository::load(&restored, &original.document_id).unwrap();
    assert_eq!(restored_manifest, current);
    let restored_receipt = storage::load(&restored, &original.document_id)
        .unwrap()
        .unwrap();
    assert_eq!(restored_receipt.source_manifest.as_ref(), Some(&original));
    let restored_settings = storage::settings(&restored, &original.document_id).unwrap();
    assert_eq!(restored_settings.revision, 2);
    assert_eq!(
        restored_settings.overrides.get(&region_id),
        Some(&RegionOverride::Exclude)
    );
    assert_eq!(
        restored_settings.action_for(
            &original.members[0].id,
            &region_id,
            "Exclude this exact region",
            true,
        ),
        Some(RegionOverride::Exclude)
    );

    for member in &restored_manifest.members {
        let restored_bytes = fs::read(restored_root.join(&member.relative_path)).unwrap();
        let original_bytes =
            fs::read(fixture.paths.resolve_relative(&member.relative_path)).unwrap();
        assert_eq!(restored_bytes, original_bytes);
    }
    let restored_manifest_path = crate::documents::image_sets::manifest::manifest_path(
        &restored_manifest.document_id,
        &restored_manifest.fingerprint().unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs::read(restored_root.join(&restored_manifest_path)).unwrap(),
        restored_manifest.bytes().unwrap()
    );
}
