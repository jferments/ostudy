//! Typed, purpose-specific IPC. Provider credentials and private quiz criteria
//! never cross this boundary.

use tauri::State;

use crate::{ai::AiRuntimeState, database::Database, error::CommandError, paths::AppPaths};

use super::{
    contracts::{ConversationDetail, ConversationPageInput, ConversationSummary, CreateConversationInput,
        DeleteConversationInput, TutorTurn, TutorTurnInput},
    conversations, service,
};

#[tauri::command]
pub fn list_tutor_conversations(database: State<'_, Database>) -> Result<Vec<ConversationSummary>, CommandError> {
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
