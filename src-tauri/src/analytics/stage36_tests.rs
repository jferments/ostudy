use super::*;
use crate::{
    cards::validate_content,
    decks::{create_deck_in_connection, CreateDeckInput},
    historical_attribution::{insert_session_snapshots, prepare_attributions},
    practice::{submit_review_in_connection, SubmitReviewInput},
    scheduler::{insert_new_schedule, load_schedule},
    util::{generate_id, unix_time_millis},
};
use rusqlite::params;
use serde_json::json;

#[test]
fn archived_history_uses_current_names_and_final_deletion_purges_every_analytics_source() {
    let database = Database::open_in_memory().expect("database");
    database.with_connection(|connection|{
        let deck=create_deck_in_connection(connection,CreateDeckInput{name:"Old deck".into(),description:None})?;let now=unix_time_millis()?;
        let content=validate_content(json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}),json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}),"Question".into(),"Answer".into())?;
        let note=generate_id(connection)?;let card=generate_id(connection)?;
        connection.execute("INSERT INTO notes(id,deck_id,note_type,content_format,content_schema_version,front_doc,back_doc,front_text,back_text,duplicate_hash,created_at,updated_at,revision) VALUES(?1,?2,'basic','tiptap-json',1,?3,?4,?5,?6,'',?7,?7,1)",params![note,deck.id,content.front_doc_json,content.back_doc_json,content.front_text,content.back_text,now]).map_err(CommandError::database)?;
        connection.execute("INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at) VALUES(?1,?2,?3,'forward',0,?4,?4)",params![card,note,deck.id,now]).map_err(CommandError::database)?;insert_new_schedule(connection,&card,now)?;
        let tag=generate_id(connection)?;connection.execute("INSERT INTO tags(id,name,normalized_name,created_at,updated_at) VALUES(?1,'Old tag','old tag',?2,?2)",params![tag,now]).map_err(CommandError::database)?;connection.execute("INSERT INTO note_tags(note_id,tag_id) VALUES(?1,?2)",params![note,tag]).map_err(CommandError::database)?;
        let session=generate_id(connection)?;connection.execute("INSERT INTO practice_sessions(id,selected_deck_ids_json,shuffle,card_count,mode,started_at) VALUES(?1,?2,0,1,'scheduled',?3)",params![session,format!("[\"{}\"]",deck.id),now]).map_err(CommandError::database)?;connection.execute("INSERT INTO practice_session_cards(session_id,card_id,position,viewed_at,revealed_at,presentation_variant) VALUES(?1,?2,0,?3,?3,'')",params![session,card,now]).map_err(CommandError::database)?;
        let attributions=prepare_attributions(connection,&[(card.clone(),String::new())])?;insert_session_snapshots(connection,&session,attributions.into_values(),now)?;
        let revision=load_schedule(connection,&card)?.revision;submit_review_in_connection(connection,SubmitReviewInput{session_id:session.clone(),card_id:card.clone(),submission_id:generate_id(connection)?,outcome:"somewhat_confident".into(),response_time_ms:1500,schedule_revision:revision})?;
        connection.execute("UPDATE decks SET name='Renamed deck',archived_at=?1 WHERE id=?2",params![now,deck.id]).map_err(CommandError::database)?;connection.execute("UPDATE cards SET suspended_at=?1 WHERE deck_id=?2",params![now,deck.id]).map_err(CommandError::database)?;connection.execute("UPDATE tags SET name='Renamed tag',normalized_name='renamed tag' WHERE id=?1",[tag]).map_err(CommandError::database)?;
        let scope=validate_scope(AnalyticsInput{range:"all".into(),deck_ids:vec![],tag_ids:vec![],highlighted_session_id:None})?;let archived=analytics_in_connection(connection,&scope)?;
        assert_eq!(archived.summary.generated_card_count,0);assert_eq!(archived.summary.review_count,1);assert_eq!(archived.deck_stats[0].deck_name,"Renamed deck");assert!(archived.deck_stats[0].archived);assert_eq!(archived.tag_stats[0].tag_name,"Renamed tag");
        let deletion=crate::deletion::delete_deck(connection,&deck.id)?;let deleted=analytics_in_connection(connection,&scope)?;assert_eq!(deleted.summary.review_count,0);assert!(deleted.deck_stats.is_empty());
        connection.execute("UPDATE deletion_operations SET expires_at=deleted_at WHERE id=?1",[deletion.undo_token]).map_err(CommandError::database)?;crate::deletion::finalize_expired_in_connection(connection,now.saturating_add(20_000))?;
        let counts:(i64,i64,i64)=connection.query_row("SELECT (SELECT COUNT(*) FROM review_events WHERE card_id=?1),(SELECT COUNT(*) FROM practice_session_cards WHERE card_id=?1),(SELECT COUNT(*) FROM review_event_attributions WHERE deck_id=?2)",params![card,deck.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(CommandError::database)?;assert_eq!(counts,(0,0,0));Ok(())
    }).expect("historical semantics");
}
