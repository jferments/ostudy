//! Typed, purpose-specific IPC. Provider credentials and private quiz criteria
//! never cross this boundary.

use tauri::State;

use crate::{ai::AiRuntimeState, database::Database, error::CommandError, paths::AppPaths};

use super::{
    contracts::{
        ConversationDetail, ConversationPageInput, ConversationSummary, CreateConversationInput,
        CreateProjectConversationInput, DeleteConversationInput, MoveConversationInput,
        RenameConversationInput, SetConversationArchivedInput, TutorTurn, TutorTurnInput,
    },
    conversations, memory, projects, service,
};

#[tauri::command]
pub fn list_tutor_projects(
    database: State<'_, Database>,
) -> Result<Vec<projects::TutorProject>, CommandError> {
    projects::list(database.inner())
}

#[tauri::command]
pub fn create_tutor_project(
    database: State<'_, Database>,
    input: projects::CreateTutorProjectInput,
) -> Result<projects::TutorProject, CommandError> {
    projects::create(database.inner(), input)
}

#[tauri::command]
pub fn update_tutor_project(
    database: State<'_, Database>,
    input: projects::UpdateTutorProjectInput,
) -> Result<projects::TutorProject, CommandError> {
    projects::update(database.inner(), input)
}

#[tauri::command]
pub fn set_tutor_project_archived(
    database: State<'_, Database>,
    input: projects::SetTutorProjectArchivedInput,
) -> Result<projects::TutorProject, CommandError> {
    projects::set_archived(database.inner(), input)
}

#[tauri::command]
pub fn delete_tutor_project(
    database: State<'_, Database>,
    input: projects::DeleteTutorProjectInput,
) -> Result<(), CommandError> {
    projects::delete(database.inner(), input)
}

#[tauri::command]
pub fn list_tutor_conversations(
    database: State<'_, Database>,
) -> Result<Vec<ConversationSummary>, CommandError> {
    conversations::list(database.inner())
}

#[tauri::command]
pub fn create_tutor_conversation(
    database: State<'_, Database>,
    input: CreateConversationInput,
) -> Result<ConversationSummary, CommandError> {
    conversations::create(database.inner(), input)
}

#[tauri::command]
pub fn create_tutor_project_conversation(
    database: State<'_, Database>,
    input: CreateProjectConversationInput,
) -> Result<ConversationSummary, CommandError> {
    conversations::create_in_project(database.inner(), input)
}

#[tauri::command]
pub fn rename_tutor_conversation(
    database: State<'_, Database>,
    input: RenameConversationInput,
) -> Result<ConversationSummary, CommandError> {
    conversations::rename(database.inner(), input)
}

#[tauri::command]
pub fn move_tutor_conversation(
    database: State<'_, Database>,
    input: MoveConversationInput,
) -> Result<ConversationSummary, CommandError> {
    conversations::move_to_project(database.inner(), input)
}

#[tauri::command]
pub fn set_tutor_conversation_archived(
    database: State<'_, Database>,
    input: SetConversationArchivedInput,
) -> Result<ConversationSummary, CommandError> {
    conversations::set_archived(database.inner(), input)
}

#[tauri::command]
pub fn get_tutor_conversation(
    database: State<'_, Database>,
    input: ConversationPageInput,
) -> Result<ConversationDetail, CommandError> {
    conversations::get(database.inner(), input)
}

#[tauri::command]
pub fn delete_tutor_conversation(
    database: State<'_, Database>,
    input: DeleteConversationInput,
) -> Result<(), CommandError> {
    let _maintenance = crate::knowledge::runtime::MaintenanceGuard::begin()?;
    conversations::delete(database.inner(), input)
}

#[tauri::command]
pub async fn send_tutor_turn(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: TutorTurnInput,
) -> Result<TutorTurn, CommandError> {
    service::send(database.inner(), paths.inner(), runtime.inner(), input).await
}

#[tauri::command]
pub fn get_tutor_memory(
    database: State<'_, Database>,
    conversation_id: String,
) -> Result<memory::TutorMemory, CommandError> {
    memory::get(database.inner(), &conversation_id)
}

#[tauri::command]
pub fn change_tutor_memory(
    database: State<'_, Database>,
    input: memory::ChangeTutorMemoryInput,
) -> Result<memory::TutorMemory, CommandError> {
    let _maintenance = crate::knowledge::runtime::MaintenanceGuard::begin()?;
    memory::change(database.inner(), input)
}
