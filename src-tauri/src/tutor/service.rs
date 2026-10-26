//! User-requested tutor turns. This service orchestrates native context and
//! provider calls; it cannot execute model tools or submit practice reviews.

use std::sync::Arc;

use crate::{
    ai::{
        document_intelligence::{
            load_verified_tutor_task_context, request_json_cancellable, DocumentTaskContext,
            JsonTaskRequest,
        },
        observe_output, AiRuntimeState, OutputObserver,
    },
    database::Database,
    error::CommandError,
    knowledge::runtime::TaskGuard,
    paths::AppPaths,
};

use super::{
    context,
    contracts::{QuizSeries, TutorMode, TutorTurn, TutorTurnInput},
    focus, history, output, turns,
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
        Some(id) => DocumentTaskContext::for_verified_tutor_profile(database, runtime, id).await?,
        None => load_verified_tutor_task_context(database, runtime).await?,
    };
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
    // A choose-one answer must be one of its question's options, word for word.
    let picked = match (&quiz, input.choice) {
        (Some(fixed), Some(chosen)) if !fixed.choices.is_empty() => {
            if fixed.choices.get(chosen).map(|choice| choice.trim()) != Some(input.text.trim()) {
                return Err(crate::knowledge::contracts::invalid(
                    "Pick one of the question's options.",
                ));
            }
            Some((fixed, chosen))
        }
        (Some(fixed), None) if !fixed.choices.is_empty() => {
            return Err(crate::knowledge::contracts::invalid(
                "Pick one of the question's options.",
            ))
        }
        (_, Some(_)) => {
            return Err(crate::knowledge::contracts::invalid(
                "This question has no options to pick from.",
            ))
        }
        _ => None,
    };
    // Next question takes the place after the one it follows; a quiz of a
    // set length ends with its last question.
    let previous = input
        .after_question_id
        .as_ref()
        .map(|id| {
            database.with_connection(|connection| {
                turns::load_asked(connection, &input.conversation_id, id)
            })
        })
        .transpose()?;
    let series = previous
        .as_ref()
        .map(|previous| QuizSeries::next(Some(previous), None))
        .transpose()?;
    let identity = turns::begin(database, &input, &task)?;
    if let Some((fixed, chosen)) = picked {
        guard.progress("checking answer", 3, 4);
        let graded = output::grade_choice(fixed, chosen);
        let question_id = input.question_id.as_deref().unwrap_or_default();
        return match turns::complete(
            database,
            &identity,
            &graded,
            &context::StudyContext::graded(question_id),
            None,
        ) {
            Ok(turn) => Ok(turn),
            Err(error) => turns::fail(database, &identity, &error, None),
        };
    }
    let outcome = async {
        let focused = focus::resolve(
            database,
            &identity.captured_scope,
            input.focus.as_ref(),
            input.mode,
            &input.conversation_id,
        )?;
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
                scope: &identity.captured_scope,
                input: &input,
                focus: &focused,
                quiz: quiz.as_ref(),
                series,
                history: remembered,
                project_instructions: identity.project_instructions.as_deref(),
            },
            &guard.signal,
        )
        .await?;
        task.ensure_current(database)?;
        guard.check()?;
        guard.progress("writing response", 2, 4);
        let schema = output::schema();
        let prompt = output::system_prompt(input.mode);
        let request = request_json_cancellable(
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
        );
        // An explanation shows while it is written. Hints, quizzes and grading
        // carry private answers, so they wait for the checks below.
        let value = if input.mode == TutorMode::Explain {
            let operation_id = input.operation_id.clone();
            let observer: OutputObserver = Arc::new(move |partial: &str| {
                if let Some(text) = output::partial_text(partial) {
                    TaskGuard::draft(&operation_id, text);
                }
            });
            observe_output(observer, request).await?
        } else {
            request.await?
        };
        guard.progress("checking response", 3, 4);
        let mut answer = output::validate(
            value,
            input.mode,
            &identity.captured_scope,
            &context,
            quiz.as_ref(),
        )?;
        if let Some(asked) = answer.quiz.as_mut() {
            asked.series = Some(match &previous {
                Some(previous) => QuizSeries::next(Some(previous), None)?,
                None => QuizSeries::next(None, asked.question_count)?,
            });
        }
        if input.mode == TutorMode::Hint {
            guard.progress("checking hint for spoilers", 3, 4);
            answer.text = if focused.question_hint.is_some() {
                context::guard_typed_hint(runtime, &task, &focused, &answer.text, &guard.signal)
                    .await?
            } else {
                output::guard_hint(runtime, &task, &focused, &answer.text, &guard.signal).await?
            };
        }
        task.ensure_current(database)?;
        guard.check()?;
        let current_focus = focus::resolve(
            database,
            &identity.captured_scope,
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
