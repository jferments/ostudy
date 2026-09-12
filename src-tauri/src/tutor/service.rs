//! User-requested tutor turns. This service orchestrates native context and
//! provider calls; it cannot execute model tools or submit practice reviews.

use crate::{
    ai::{
        document_intelligence::{
            load_tutor_task_context, request_json_cancellable, DocumentTaskContext, JsonTaskRequest,
        },
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    knowledge::runtime::TaskGuard,
    paths::AppPaths,
};

use super::{
    context,
    contracts::{TutorMode, TutorTurn, TutorTurnInput},
    conversations, focus, history, output, turns,
};

pub(crate) async fn send(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: TutorTurnInput,
) -> Result<TutorTurn, CommandError> {
    input.validate()?;
    if let Some(saved) = turns::replay(database, &input)? {
        return Ok(saved);
    }
    let guard = TaskGuard::begin(
        &input.operation_id,
        &format!("tutor:{}", input.conversation_id),
        false,
    )?;
    let task = match &input.profile_id {
        Some(id) => DocumentTaskContext::for_tutor_profile(database, id)?,
        None => load_tutor_task_context(database)?,
    };
    let conversation = database.with_connection(|connection| {
        conversations::load_summary(connection, &input.conversation_id)
    })?;
    let focused = focus::resolve(
        database,
        &conversation.scope,
        input.focus.as_ref(),
        input.mode,
        &input.conversation_id,
    )?;
    // Read the private rubric before admission marks an answer as running.
    // Admission independently verifies that the same question is pending.
    let quiz = input
        .question_id
        .as_ref()
        .map(|id| {
            database.with_connection(|connection| {
                turns::load_question(connection, &input.conversation_id, id)
            })
        })
        .transpose()?;
    let identity = turns::begin(database, &input, &task)?;
    let outcome = async {
        guard.progress("reading conversation", 0, 4);
        let remembered = if input.mode == TutorMode::Hint {
            history::HistorySnapshot::default()
        } else {
            if let Err(error) = history::refresh_summary(
                database,
                runtime,
                &task,
                &input.conversation_id,
                &guard.signal,
            )
            .await
            {
                guard.check()?;
                task.ensure_current(database)?;
                crate::logging::debug_target(
                    "ai.tutor.history",
                    "tutor_summary_not_updated",
                    "The tutor will use bounded existing history; no message content was logged.",
                    serde_json::json!({"code": error.code}),
                );
            }
            history::load(database, &input.conversation_id)?
        };
        guard.check()?;
        guard.progress("resolving study context", 1, 4);
        let context = context::build(
            database,
            paths,
            runtime,
            &task,
            context::StudyContextInput {
                scope: &conversation.scope,
                input: &input,
                focus: &focused,
                quiz: quiz.as_ref(),
                history: remembered,
            },
            &guard.signal,
        )
        .await?;
        task.ensure_current(database)?;
        guard.check()?;
        guard.progress("writing response", 2, 4);
        let schema = output::schema();
        let prompt = output::system_prompt(input.mode);
        let value = request_json_cancellable(
            runtime,
            &task,
            JsonTaskRequest {
                system_prompt: &prompt,
                user_text: &context.request_text,
                schema: &schema,
                max_tokens: context.completion_tokens,
                images: &context.images,
            },
            &guard.signal,
        )
        .await?;
        let mut answer = output::validate(
            value,
            input.mode,
            &conversation.scope,
            &context,
            quiz.as_ref(),
        )?;
        if input.mode == TutorMode::Hint {
            guard.progress("checking hint for spoilers", 3, 4);
            answer.text =
                output::guard_hint(runtime, &task, &focused, &answer.text, &guard.signal).await?;
        }
        task.ensure_current(database)?;
        guard.check()?;
        let current_focus = focus::resolve(
            database,
            &conversation.scope,
            input.focus.as_ref(),
            input.mode,
            &input.conversation_id,
        )?;
        if current_focus.identity != focused.identity {
            return Err(focus::changed());
        }
        let result = turns::complete(
            database,
            &identity,
            &answer,
            &context,
            task.usage_summary().0,
        )?;
        guard.progress("response saved", 4, 4);
        Ok(result)
    }
    .await;
    match outcome {
        Ok(turn) => Ok(turn),
        Err(error) => turns::fail(database, &identity, &error, task.usage_summary().0),
    }
}
