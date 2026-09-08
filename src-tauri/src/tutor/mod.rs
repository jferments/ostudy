//! Content-focused conversational tutoring. The tutor owns conversations and
//! fixed conversational quiz rubrics, not practice ratings or learner profiles.

pub(crate) mod commands;
mod context;
mod contracts;
mod conversations;
mod focus;
mod history;
mod output;
mod service;
mod turns;

#[cfg(test)]
mod tests;

pub(crate) fn reconcile_startup(
    database: &crate::database::Database,
) -> Result<(), crate::error::CommandError> {
    turns::reconcile_startup(database)
}
