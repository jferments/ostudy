use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    pub time_zone_id: String,
    pub utc_offset_minutes: i32,
    pub logical_cpu_count: u32,
    pub available_memory_bytes: Option<u64>,
    pub low_memory: bool,
    pub thermal_state: String,
    pub battery_saver: bool,
    pub charging: Option<bool>,
    pub online: bool,
    pub metered: bool,
    pub validated_internet: bool,
    pub transport: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimestampRequest {
    pub utc_millis: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeZoneSnapshotResult {
    pub identifier: String,
    pub utc_offset_minutes: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDateRequest {
    pub utc_millis: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDateResult {
    pub date: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstValidInstantRequest {
    pub date: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstValidInstantResult {
    pub utc_millis: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireImportRequest {
    pub title: String,
    pub mime_types: Vec<String>,
    pub target_path: String,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireImportResult {
    pub cancelled: bool,
    pub display_name: Option<String>,
    pub reported_media_type: Option<String>,
    pub copied_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishExportRequest {
    pub title: String,
    pub suggested_filename: String,
    pub mime_type: String,
    pub source_path: String,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishExportResult {
    pub cancelled: bool,
    pub display_name: Option<String>,
    pub copied_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFileRequest {
    pub title: String,
    pub display_name: String,
    pub mime_type: String,
    pub source_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFileResult {
    pub launched: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackInterceptionRequest {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRequest {
    pub locator: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSecretRequest {
    pub locator: String,
    pub value_base64: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretAvailabilityResult {
    pub state: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSecretResult {
    pub state: String,
    pub value_base64: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingShareSummary {
    pub token: String,
    pub display_name: String,
    pub reported_media_type: Option<String>,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingShareListResult {
    pub items: Vec<IncomingShareSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingShareRequest {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakenIncomingShare {
    pub token: String,
    pub display_name: String,
    pub reported_media_type: Option<String>,
    pub byte_size: u64,
    pub path: String,
}
