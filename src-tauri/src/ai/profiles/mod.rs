//! AI provider profile aggregate, task defaults, credential metadata, and
//! write-only credential coordination.

mod commands;
mod contracts;
mod defaults;
mod policy;
mod projections;
mod repository;
mod service;

pub(crate) use commands::{
    clear_ai_provider_credential, create_ai_provider_profile, delete_ai_provider_profile,
    get_ai_provider_catalog, get_ai_provider_profile, get_ai_task_defaults,
    list_ai_provider_profiles, set_ai_provider_credential, set_ai_task_default,
    update_ai_provider_profile,
};
pub(crate) use contracts::{
    AiProviderProfileDetailDto, AiProviderProfileDetailDto as ProviderProfileDetailDto,
    AiProviderProfileSummaryDto, AiProviderProfileSummaryDto as ProviderProfileSummaryDto,
    AiTaskDefaultsDto, ProfileRequestSnapshot, ProfileSnapshot,
};
pub(crate) use repository::load_profile_snapshot;
pub(crate) use service::{load_default_profile_request, load_profile_request};

#[cfg(test)]
mod tests;
