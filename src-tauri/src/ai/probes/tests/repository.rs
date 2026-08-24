use rusqlite::params;
use serde_json::json;

use crate::{database::Database, error::CommandError};

use super::super::super::{
    catalog::{AuthMode, ProviderKind},
    profiles::ProfileSnapshot,
};
use super::super::{
    contracts::{
        AiCapabilityDto, AiProviderProbeResultDto, CapabilitySource, CapabilityStatus,
        ProbeOverallStatus, ADAPTER_REVISION, RESULT_SCHEMA_VERSION,
    },
    phases::passed_phase,
    repository,
};

fn profile() -> ProfileSnapshot {
    ProfileSnapshot {
        id: "0123456789abcdef0123456789abcdef".to_string(),
        provider_kind: ProviderKind::LlamaCpp,
        base_url: "http://127.0.0.1:8080/".to_string(),
        model_id: "model".to_string(),
        auth_mode: AuthMode::None,
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 60_000,
        credential_epoch: 2,
        revision: 3,
    }
}

fn insert_profile(database: &Database, profile: &ProfileSnapshot) {
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind, base_url,
                         model_id, auth_mode, enabled, allow_insecure_http,
                         require_zero_data_retention, request_timeout_ms,
                         credential_storage, credential_epoch, revision,
                         created_at, updated_at, deleted_at
                     ) VALUES (?1, 'Probe', 'probe', ?2, ?3, ?4, ?5,
                               1, 0, 0, ?6, 'none', ?7, ?8, 1, 1, NULL)",
                    params![
                        profile.id,
                        profile.provider_kind.as_str(),
                        profile.base_url,
                        profile.model_id,
                        profile.auth_mode.as_str(),
                        profile.request_timeout_ms,
                        profile.credential_epoch,
                        profile.revision,
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("profile should insert");
}

#[test]
fn result_and_capabilities_are_replaced_atomically() {
    let database = Database::open_in_memory().expect("database should open");
    let profile = profile();
    insert_profile(&database, &profile);
    let first = repository::finish_and_persist(
        &database,
        &profile,
        ProbeOverallStatus::Verified,
        vec![passed_phase("text", "Text", "Passed.", Some(4))],
        vec![AiCapabilityDto {
            key: "image_input".to_string(),
            status: CapabilityStatus::Unsupported,
            source: CapabilitySource::Declared,
            value: None,
        }],
        10,
    )
    .expect("first result should persist");
    assert!(first.persisted);

    let second = repository::finish_and_persist(
        &database,
        &profile,
        ProbeOverallStatus::Partial,
        vec![passed_phase("text", "Text", "Passed again.", Some(3))],
        vec![AiCapabilityDto {
            key: "image_input".to_string(),
            status: CapabilityStatus::Supported,
            source: CapabilitySource::Probed,
            value: Some(json!(true)),
        }],
        20,
    )
    .expect("replacement should persist");
    assert!(second.persisted);
    let loaded = repository::load_probe_result(&database, &profile.id)
        .expect("result should load")
        .expect("result should exist");
    assert_eq!(loaded.overall_status, ProbeOverallStatus::Partial);
    assert_eq!(loaded.tested_at, 20);
    let capabilities = repository::load_capability_records(&database, &profile.id)
        .expect("capabilities should load");
    assert_eq!(capabilities.len(), 1);
    assert_eq!(capabilities[0].key, "image_input");
    assert_eq!(capabilities[0].status, CapabilityStatus::Supported);
    assert_eq!(capabilities[0].value, Some(json!(true)));
}

#[test]
fn revision_model_and_credential_epoch_bind_freshness() {
    let profile = profile();
    let mut result = AiProviderProbeResultDto {
        profile_id: profile.id.clone(),
        profile_revision: 3,
        credential_epoch: 2,
        tested_model_id: "model".to_string(),
        adapter_revision: ADAPTER_REVISION,
        result_schema_version: RESULT_SCHEMA_VERSION,
        overall_status: ProbeOverallStatus::Verified,
        phases: Vec::new(),
        capabilities: Vec::new(),
        tested_at: 1,
        persisted: true,
        stale: false,
    };
    assert!(repository::snapshot_matches_result(&profile, &result));
    result.credential_epoch = 1;
    assert!(!repository::snapshot_matches_result(&profile, &result));
}

#[test]
fn probe_result_serialization_excludes_raw_provider_payloads() {
    let profile = profile();
    let result = AiProviderProbeResultDto {
        profile_id: profile.id,
        profile_revision: 1,
        credential_epoch: 0,
        tested_model_id: "model".to_string(),
        adapter_revision: ADAPTER_REVISION,
        result_schema_version: RESULT_SCHEMA_VERSION,
        overall_status: ProbeOverallStatus::Verified,
        phases: vec![passed_phase("text", "Text", "Passed.", Some(4))],
        capabilities: Vec::new(),
        tested_at: 1,
        persisted: false,
        stale: false,
    };
    let serialized = serde_json::to_string(&result).expect("result should serialize");
    assert!(!serialized.contains("authorization"));
    assert!(!serialized.contains("reasoning_content"));
    assert!(serialized.len() < 32 * 1024);
}

#[test]
fn invalid_saved_capability_status_is_rejected() {
    let database = Database::open_in_memory().expect("database should open");
    let profile = profile();
    insert_profile(&database, &profile);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_provider_capabilities (
                         profile_id, capability_key, status, source_kind, value_json, checked_at
                     ) VALUES (?1, 'image_input', 'invalid', 'probed', 'null', 1)",
                    [profile.id.as_str()],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .expect("invalid fixture should insert");
    let error = repository::load_capability_records(&database, &profile.id)
        .expect_err("invalid capability status must not escape persistence");
    assert_eq!(error.code, "invalid_ai_capability_status");
}
