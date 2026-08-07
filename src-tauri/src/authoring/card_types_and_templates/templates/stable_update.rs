use super::*;

pub(super) fn update_card_template_with_stable_children_in_connection(
    connection: &mut Connection,
    paths: &AppPaths,
    input: UpdateCardTemplateInput,
) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&input.template_id, "card template")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The template revision is invalid.",
        ));
    }
    let name = validate_template_name(input.name)?;
    let normalized_name = normalize_template_name(&name);
    let description = validate_bounded_visible_text(
        input.description,
        MAX_TEMPLATE_DESCRIPTION_CHARS,
        "Template descriptions",
        true,
    )?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let current: Option<i64> = transaction
        .query_row(
            "SELECT revision FROM card_templates
             WHERE id = ?1 AND deleted_at IS NULL",
            [input.template_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(current_revision) = current else {
        return Err(CommandError::new(
            "template_not_found",
            "That card template no longer exists.",
        ));
    };
    if current_revision != input.revision {
        return Err(CommandError::new(
            "template_changed",
            "This template changed in another window. Reload it and try again.",
        ));
    }
    ensure_template_name_available(
        &transaction,
        &normalized_name,
        Some(&input.template_id),
    )?;
    seed_new_template_child_ids(
        &transaction,
        &input.template_id,
        &input.fields,
        &input.rules,
    )?;
    let fields = validate_template_fields(
        &transaction,
        &input.template_id,
        input.fields,
        true,
    )?;
    let rules = validate_template_rules(
        &transaction,
        &input.template_id,
        input.rules,
        &fields,
        true,
    )?;
    let asset_ids = template_rule_asset_ids(&rules)?;
    ensure_assets_exist(&transaction, &asset_ids)?;
    let now = unix_time_millis()?;
    let changed = transaction
        .execute(
            "UPDATE card_templates
             SET name = ?1, normalized_name = ?2, description = ?3,
                 updated_at = ?4, revision = revision + 1
             WHERE id = ?5 AND revision = ?6 AND deleted_at IS NULL",
            params![
                name,
                normalized_name,
                description,
                now,
                input.template_id,
                input.revision,
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "template_changed",
            "This template changed in another window. Reload it and try again.",
        ));
    }
    transaction
        .execute(
            "DELETE FROM card_template_rules WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM card_template_fields WHERE template_id = ?1",
            [input.template_id.as_str()],
        )
        .map_err(CommandError::database)?;
    insert_template_children(&transaction, &input.template_id, &fields, &rules)?;
    replace_template_assets(&transaction, &input.template_id, &asset_ids)?;
    transaction.commit().map_err(CommandError::database)?;
    load_stored_template(connection, paths, &input.template_id, true).map(|stored| stored.dto)
}

fn seed_new_template_child_ids(
    connection: &Connection,
    template_id: &str,
    fields: &[CardTemplateFieldInput],
    rules: &[CardTemplateRuleInput],
) -> Result<(), CommandError> {
    let mut field_position: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM card_template_fields WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    for field in fields {
        let Some(field_id) = field.id.as_deref() else {
            continue;
        };
        validate_entity_id(field_id, "template field")?;
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM card_template_fields WHERE id = ?1)",
                [field_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if exists == 0 {
            let pending_name = format!("pending-{field_id}");
            connection
                .execute(
                    "INSERT INTO card_template_fields (
                         id, template_id, name, normalized_name, description, position
                     ) VALUES (?1, ?2, ?3, ?3, '', ?4)",
                    params![field_id, template_id, pending_name, field_position],
                )
                .map_err(CommandError::database)?;
            field_position += 1;
        }
    }

    let mut rule_position: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM card_template_rules WHERE template_id = ?1",
            [template_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    for rule in rules {
        let Some(rule_id) = rule.id.as_deref() else {
            continue;
        };
        validate_entity_id(rule_id, "template rule")?;
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM card_template_rules WHERE id = ?1)",
                [rule_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if exists == 0 {
            let pending_name = format!("pending-{rule_id}");
            connection
                .execute(
                    "INSERT INTO card_template_rules (
                         id, template_id, name, front_pattern, back_pattern, position
                     ) VALUES (?1, ?2, ?3, 'pending', 'pending', ?4)",
                    params![rule_id, template_id, pending_name, rule_position],
                )
                .map_err(CommandError::database)?;
            rule_position += 1;
        }
    }
    Ok(())
}
