#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub field_count: i64,
    pub generated_card_rule_count: i64,
    pub image_count: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[tauri::command]
pub fn list_card_template_summaries(
    database: State<'_, Database>,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    database.with_connection(list_card_template_summaries_in_connection)
}

fn list_card_template_summaries_in_connection(
    connection: &mut Connection,
) -> Result<Vec<CardTemplateSummaryDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT
                 ct.id,
                 ct.name,
                 ct.description,
                 COUNT(DISTINCT ctf.id),
                 COUNT(DISTINCT ctr.id),
                 COUNT(DISTINCT CASE
                     WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id
                 END),
                 ct.updated_at,
                 ct.revision
             FROM card_templates ct
             LEFT JOIN card_template_fields ctf ON ctf.template_id = ct.id
             LEFT JOIN card_template_rules ctr ON ctr.template_id = ct.id
             LEFT JOIN card_template_assets cta ON cta.template_id = ct.id
             LEFT JOIN assets a ON a.id = cta.asset_id
             WHERE ct.deleted_at IS NULL
             GROUP BY ct.id, ct.name, ct.description, ct.updated_at, ct.revision
             ORDER BY ct.name COLLATE NOCASE ASC, ct.created_at ASC, ct.id ASC",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([], |row| {
            Ok(CardTemplateSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                field_count: row.get(3)?,
                generated_card_rule_count: row.get(4)?,
                image_count: row.get(5)?,
                updated_at: row.get(6)?,
                revision: row.get(7)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

#[cfg(test)]
mod template_summary_tests {
    use super::*;

    #[test]
    fn empty_template_library_returns_an_empty_summary_list() {
        let database = Database::open_in_memory().expect("database should open");
        let summaries = database
            .with_connection(list_card_template_summaries_in_connection)
            .expect("summaries should load");
        assert!(summaries.is_empty());
    }
}
