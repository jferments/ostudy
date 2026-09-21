use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    knowledge::{
        adapters::scope_documents,
        contracts::{bounded, invalid, StudyScope},
    },
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::organization;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TutorProject {
    pub id: String,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub default_scope: StudyScope,
    pub revision: i64,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateTutorProjectInput {
    pub operation_id: String,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub default_scope: StudyScope,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateTutorProjectInput {
    pub operation_id: String,
    pub project_id: String,
    pub expected_revision: i64,
    pub title: String,
    pub description: String,
    pub instructions: String,
    pub default_scope: StudyScope,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetTutorProjectArchivedInput {
    pub operation_id: String,
    pub project_id: String,
    pub expected_revision: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeleteTutorProjectInput {
    pub operation_id: String,
    pub project_id: String,
    pub expected_revision: i64,
}

pub(crate) fn list(database: &Database) -> Result<Vec<TutorProject>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id,title,description,instructions,default_scope_json,revision,
                        archived_at,created_at,updated_at
                 FROM tutor_projects
                 ORDER BY archived_at IS NOT NULL, title COLLATE NOCASE, id
                 LIMIT 500",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], raw_project)
            .map_err(CommandError::database)?;
        let projects = rows
            .map(|row| decode_project(row.map_err(CommandError::database)?))
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(projects)
    })
}

pub(crate) fn create(
    database: &Database,
    input: CreateTutorProjectInput,
) -> Result<TutorProject, CommandError> {
    validate_create(&input)?;
    let request_hash = organization::request_hash("create_project", &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) =
            organization::replay(&tx, &input.operation_id, "create_project", &request_hash)?
        {
            return Ok(saved);
        }
        scope_documents(&tx, &input.default_scope)?;
        let id = generate_id(&tx)?;
        let now = unix_time_millis()?;
        let default_scope =
            serde_json::to_string(&input.default_scope).map_err(CommandError::serialization)?;
        tx.execute(
            "INSERT INTO tutor_projects(
                 id,title,description,instructions,default_scope_json,revision,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,1,?6,?6)",
            params![
                id,
                input.title.trim(),
                input.description.trim(),
                input.instructions.trim(),
                default_scope,
                now
            ],
        )
        .map_err(CommandError::database)?;
        let result = load(&tx, &id)?;
        organization::record(
            &tx,
            &input.operation_id,
            "create_project",
            "project",
            &id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn update(
    database: &Database,
    input: UpdateTutorProjectInput,
) -> Result<TutorProject, CommandError> {
    validate_entity_id(&input.project_id, "Tutor project")?;
    validate_revision(input.expected_revision)?;
    validate_text(&input.title, &input.description, &input.instructions)?;
    input.default_scope.validate()?;
    let request_hash = organization::request_hash("update_project", &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) =
            organization::replay(&tx, &input.operation_id, "update_project", &request_hash)?
        {
            return Ok(saved);
        }
        scope_documents(&tx, &input.default_scope)?;
        let scope =
            serde_json::to_string(&input.default_scope).map_err(CommandError::serialization)?;
        let now = unix_time_millis()?;
        let changed = tx
            .execute(
                "UPDATE tutor_projects
                 SET title=?1,description=?2,instructions=?3,default_scope_json=?4,
                     revision=revision+1,updated_at=?5
                 WHERE id=?6 AND revision=?7",
                params![
                    input.title.trim(),
                    input.description.trim(),
                    input.instructions.trim(),
                    scope,
                    now,
                    input.project_id,
                    input.expected_revision
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(changed_project());
        }
        let result = load(&tx, &input.project_id)?;
        organization::record(
            &tx,
            &input.operation_id,
            "update_project",
            "project",
            &input.project_id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn set_archived(
    database: &Database,
    input: SetTutorProjectArchivedInput,
) -> Result<TutorProject, CommandError> {
    validate_entity_id(&input.project_id, "Tutor project")?;
    validate_revision(input.expected_revision)?;
    let action = if input.archived {
        "archive_project"
    } else {
        "restore_project"
    };
    let request_hash = organization::request_hash(action, &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(saved) = organization::replay(&tx, &input.operation_id, action, &request_hash)?
        {
            return Ok(saved);
        }
        let current = load(&tx, &input.project_id)?;
        if current.revision != input.expected_revision {
            return Err(changed_project());
        }
        let desired = if input.archived {
            Some(unix_time_millis()?)
        } else {
            None
        };
        if current.archived_at.is_some() != input.archived {
            let now = unix_time_millis()?;
            tx.execute(
                "UPDATE tutor_projects
                 SET archived_at=?1,revision=revision+1,updated_at=?2
                 WHERE id=?3 AND revision=?4",
                params![desired, now, input.project_id, input.expected_revision],
            )
            .map_err(CommandError::database)?;
        }
        let result = load(&tx, &input.project_id)?;
        organization::record(
            &tx,
            &input.operation_id,
            action,
            "project",
            &input.project_id,
            &request_hash,
            &result,
        )?;
        tx.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}

pub(crate) fn delete(
    database: &Database,
    input: DeleteTutorProjectInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.project_id, "Tutor project")?;
    validate_revision(input.expected_revision)?;
    let request_hash = organization::request_hash("delete_project", &input)?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if organization::replay::<serde_json::Value>(
            &tx,
            &input.operation_id,
            "delete_project",
            &request_hash,
        )?
        .is_some()
        {
            return Ok(());
        }
        let revision = tx
            .query_row(
                "SELECT revision FROM tutor_projects WHERE id=?1",
                [&input.project_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some(revision) = revision else {
            organization::record(
                &tx,
                &input.operation_id,
                "delete_project",
                "project",
                &input.project_id,
                &request_hash,
                &serde_json::Value::Null,
            )?;
            tx.commit().map_err(CommandError::database)?;
            return Ok(());
        };
        if revision != input.expected_revision {
            return Err(changed_project());
        }
        let now = unix_time_millis()?;
        tx.execute(
            "UPDATE tutor_conversations
             SET project_id=NULL,organization_revision=organization_revision+1,updated_at=?1
             WHERE project_id=?2",
            params![now, input.project_id],
        )
        .map_err(CommandError::database)?;
        tx.execute(
            "DELETE FROM tutor_projects WHERE id=?1",
            [&input.project_id],
        )
        .map_err(CommandError::database)?;
        organization::record(
            &tx,
            &input.operation_id,
            "delete_project",
            "project",
            &input.project_id,
            &request_hash,
            &serde_json::Value::Null,
        )?;
        tx.commit().map_err(CommandError::database)
    })
}

pub(crate) fn load(
    connection: &Connection,
    project_id: &str,
) -> Result<TutorProject, CommandError> {
    validate_entity_id(project_id, "Tutor project")?;
    let raw = connection
        .query_row(
            "SELECT id,title,description,instructions,default_scope_json,revision,
                    archived_at,created_at,updated_at
             FROM tutor_projects WHERE id=?1",
            [project_id],
            raw_project,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(project_missing)?;
    decode_project(raw)
}

pub(crate) fn active_default_scope(
    connection: &Connection,
    project_id: &str,
) -> Result<StudyScope, CommandError> {
    let project = load(connection, project_id)?;
    if project.archived_at.is_some() {
        return Err(invalid(
            "Restore this Tutor project before starting or moving chats into it.",
        ));
    }
    scope_documents(connection, &project.default_scope)?;
    Ok(project.default_scope)
}

pub(crate) fn instructions(
    connection: &Connection,
    project_id: Option<&str>,
) -> Result<Option<String>, CommandError> {
    let Some(project_id) = project_id else {
        return Ok(None);
    };
    let project = load(connection, project_id)?;
    Ok((!project.instructions.trim().is_empty()).then_some(project.instructions))
}

type RawProject = (
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    i64,
    i64,
);

fn raw_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawProject> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn decode_project(row: RawProject) -> Result<TutorProject, CommandError> {
    Ok(TutorProject {
        id: row.0,
        title: row.1,
        description: row.2,
        instructions: row.3,
        default_scope: serde_json::from_str(&row.4).map_err(CommandError::serialization)?,
        revision: row.5,
        archived_at: row.6,
        created_at: row.7,
        updated_at: row.8,
    })
}

fn validate_create(input: &CreateTutorProjectInput) -> Result<(), CommandError> {
    validate_entity_id(&input.operation_id, "Tutor project operation")?;
    validate_text(&input.title, &input.description, &input.instructions)?;
    input.default_scope.validate()
}

fn validate_text(title: &str, description: &str, instructions: &str) -> Result<(), CommandError> {
    bounded(title, 160, "Tutor project title", false)?;
    bounded(description, 4_000, "Tutor project description", true)?;
    bounded(instructions, 8_000, "Tutor project instructions", true)
}

fn validate_revision(revision: i64) -> Result<(), CommandError> {
    if revision < 1 {
        return Err(invalid("The Tutor project revision is invalid."));
    }
    Ok(())
}

fn changed_project() -> CommandError {
    CommandError::new(
        "tutor_project_changed",
        "This Tutor project changed. Reload it before applying another organization change.",
    )
}

fn project_missing() -> CommandError {
    CommandError::new(
        "tutor_project_missing",
        "This Tutor project is no longer available.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        knowledge::contracts::ScopeKind,
        tutor::{contracts::CreateProjectConversationInput, conversations},
    };

    fn general() -> StudyScope {
        StudyScope::single(ScopeKind::General, None)
    }

    fn create_input(operation: char) -> CreateTutorProjectInput {
        CreateTutorProjectInput {
            operation_id: operation.to_string().repeat(32),
            title: "Cardiology".into(),
            description: "Hemodynamics and pharmacology".into(),
            instructions: "Use concise mechanistic explanations.".into(),
            default_scope: general(),
        }
    }

    #[test]
    fn project_create_update_archive_and_delete_are_retry_safe() {
        let database = Database::open_in_memory().unwrap();
        let created = create(&database, create_input('a')).unwrap();
        assert_eq!(create(&database, create_input('a')).unwrap(), created);

        let updated = update(
            &database,
            UpdateTutorProjectInput {
                operation_id: "b".repeat(32),
                project_id: created.id.clone(),
                expected_revision: created.revision,
                title: "Cardiovascular".into(),
                description: created.description.clone(),
                instructions: "Compare mechanisms before memorizing drugs.".into(),
                default_scope: general(),
            },
        )
        .unwrap();
        assert_eq!(updated.revision, created.revision + 1);

        let archived = set_archived(
            &database,
            SetTutorProjectArchivedInput {
                operation_id: "c".repeat(32),
                project_id: created.id.clone(),
                expected_revision: updated.revision,
                archived: true,
            },
        )
        .unwrap();
        assert!(archived.archived_at.is_some());

        delete(
            &database,
            DeleteTutorProjectInput {
                operation_id: "d".repeat(32),
                project_id: created.id.clone(),
                expected_revision: archived.revision,
            },
        )
        .unwrap();
        delete(
            &database,
            DeleteTutorProjectInput {
                operation_id: "d".repeat(32),
                project_id: created.id,
                expected_revision: archived.revision,
            },
        )
        .unwrap();
    }

    #[test]
    fn missing_project_delete_reserves_its_retry_identity() {
        let database = Database::open_in_memory().unwrap();
        let operation_id = "8".repeat(32);
        delete(
            &database,
            DeleteTutorProjectInput {
                operation_id: operation_id.clone(),
                project_id: "9".repeat(32),
                expected_revision: 1,
            },
        )
        .unwrap();

        let mut reused = create_input('a');
        reused.operation_id = operation_id;
        assert!(create(&database, reused).is_err());
    }

    #[test]
    fn project_default_changes_affect_only_future_chats_and_explicit_sources_win() {
        let database = Database::open_in_memory().unwrap();
        let created = create(&database, create_input('4')).unwrap();

        let first = conversations::create_in_project(
            &database,
            CreateProjectConversationInput {
                operation_id: "5".repeat(32),
                project_id: created.id.clone(),
                title: "First chat".into(),
                scope: None,
            },
        )
        .unwrap();
        assert_eq!(first.scope, general());

        let updated = update(
            &database,
            UpdateTutorProjectInput {
                operation_id: "6".repeat(32),
                project_id: created.id.clone(),
                expected_revision: created.revision,
                title: created.title.clone(),
                description: created.description.clone(),
                instructions: created.instructions.clone(),
                default_scope: StudyScope::single(ScopeKind::Library, None),
            },
        )
        .unwrap();

        let second = conversations::create_in_project(
            &database,
            CreateProjectConversationInput {
                operation_id: "7".repeat(32),
                project_id: updated.id.clone(),
                title: "Second chat".into(),
                scope: None,
            },
        )
        .unwrap();
        assert_eq!(
            second.scope,
            StudyScope::single(ScopeKind::Library, None)
        );

        let explicit = conversations::create_in_project(
            &database,
            CreateProjectConversationInput {
                operation_id: "8".repeat(32),
                project_id: updated.id,
                title: "Explicit chat".into(),
                scope: Some(general()),
            },
        )
        .unwrap();
        assert_eq!(explicit.scope, general());

        let reloaded_first = database
            .with_connection(|connection| conversations::load_summary(connection, &first.id))
            .unwrap();
        assert_eq!(reloaded_first.scope, general());
    }

    #[test]
    fn deleting_project_ungroups_chats_without_rewriting_their_sources() {
        let database = Database::open_in_memory().unwrap();
        let project = create(&database, create_input('e')).unwrap();
        let conversation = conversations::create_in_project(
            &database,
            CreateProjectConversationInput {
                operation_id: "f".repeat(32),
                project_id: project.id.clone(),
                title: "Hemodynamics".into(),
                scope: None,
            },
        )
        .unwrap();
        let original_scope = conversation.scope.clone();

        delete(
            &database,
            DeleteTutorProjectInput {
                operation_id: "1".repeat(32),
                project_id: project.id,
                expected_revision: project.revision,
            },
        )
        .unwrap();

        let saved = database
            .with_connection(|connection| conversations::load_summary(connection, &conversation.id))
            .unwrap();
        assert_eq!(saved.project_id, None);
        assert_eq!(saved.scope, original_scope);
        assert_eq!(
            saved.organization_revision,
            conversation.organization_revision + 1
        );
    }
}
