use super::*;
use crate::{cards::validate_content, scheduler::insert_new_schedule};
use rusqlite::params;
use serde_json::json;

fn add_card(
    connection: &mut Connection,
    deck_id: &str,
    label: &str,
    suspended: Option<i64>,
) -> Result<String, CommandError> {
    let content = validate_content(
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":label}]}]}),
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"A"}]}]}),
        label.to_string(),
        "A".to_string(),
    )?;
    let note = generate_id(connection)?;
    let card = generate_id(connection)?;
    let now = unix_time_millis()?;
    connection.execute("INSERT INTO notes(id,deck_id,note_type,content_format,content_schema_version,front_doc,back_doc,front_text,back_text,duplicate_hash,created_at,updated_at,revision) VALUES(?1,?2,'basic','tiptap-json',1,?3,?4,?5,?6,'',?7,?7,1)",params![note,deck_id,content.front_doc_json,content.back_doc_json,content.front_text,content.back_text,now]).map_err(CommandError::database)?;
    connection.execute("INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at,suspended_at) VALUES(?1,?2,?3,'forward',0,?4,?4,?5)",params![card,note,deck_id,now,suspended]).map_err(CommandError::database)?;
    insert_new_schedule(connection, &card, now)?;
    Ok(card)
}

#[test]
fn archive_suspends_every_card_and_unarchive_preserves_manual_suspensions() {
    let database = Database::open_in_memory().expect("database");
    database.with_connection(|connection|{
        let deck=create_deck_in_connection(connection,CreateDeckInput{name:"Archive".into(),description:None})?;
        let automatic=add_card(connection,&deck.id,"Automatic",None)?;
        let manual=add_card(connection,&deck.id,"Manual",Some(5))?;
        let archived=archive_deck_in_connection(connection,&deck.id)?;assert!(archived.archived_at.is_some());assert_eq!(archived.studyable_due_count,0);
        let states:(Option<i64>,Option<i64>,i64)=connection.query_row("SELECT (SELECT suspended_at FROM cards WHERE id=?1),(SELECT suspended_at FROM cards WHERE id=?2),(SELECT COUNT(*) FROM deck_archive_cards WHERE deck_id=?3)",params![automatic,manual,deck.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(CommandError::database)?;
        assert!(states.0.is_some());assert_eq!(states.1,Some(5));assert_eq!(states.2,1);
        unarchive_deck_in_connection(connection,&deck.id)?;
        let restored:(Option<i64>,Option<i64>)=connection.query_row("SELECT (SELECT suspended_at FROM cards WHERE id=?1),(SELECT suspended_at FROM cards WHERE id=?2)",params![automatic,manual],|row|Ok((row.get(0)?,row.get(1)?))).map_err(CommandError::database)?;
        assert_eq!(restored.0,None);assert_eq!(restored.1,Some(5));Ok(())
    }).expect("archive ownership");
}
