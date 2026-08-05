#!/usr/bin/env python3
"""Add final race guards, deletion purging, regressions, docs, and verifier."""
from __future__ import annotations
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):
    target=ROOT/p;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(c,encoding='utf-8')

# Analytics stale response guard.
p='src/screens/AnalyticsScreen.tsx';t=read(p)
if 'analyticsRequestRef' not in t:
    t=t.replace('const handledLaunchRef = useRef<number | null>(null);','const handledLaunchRef = useRef<number | null>(null);\n  const analyticsRequestRef = useRef(0);')
    old='''  const load = useCallback(async (): Promise<void> => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      setAnalytics(
        await getAnalytics({
          range,
          deckIds,
          tagIds,
          highlightedSessionId,
        }),
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setIsLoading(false);
    }
  }, [deckIds, highlightedSessionId, range, showToast, tagIds]);'''
    new='''  const load = useCallback(async (): Promise<void> => {
    const requestId = ++analyticsRequestRef.current;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const result = await getAnalytics({ range, deckIds, tagIds, highlightedSessionId });
      if (analyticsRequestRef.current === requestId) setAnalytics(result);
    } catch (error: unknown) {
      if (analyticsRequestRef.current !== requestId) return;
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      if (analyticsRequestRef.current === requestId) setIsLoading(false);
    }
  }, [deckIds, highlightedSessionId, range, showToast, tagIds]);'''
    if old not in t:raise SystemExit('Analytics load anchor changed')
    t=t.replace(old,new,1)
write(p,t)

# Practice workload stale response guard.
p='src/screens/PracticeScreen.tsx';t=read(p)
if 'workloadRequestRef' not in t:
    t=t.replace('const pendingReviewRef = useRef<PendingReviewOperation | null>(null);','const pendingReviewRef = useRef<PendingReviewOperation | null>(null);\n  const workloadRequestRef = useRef(0);')
    old='''  const loadWorkload = useCallback(async (deckIds: string[]): Promise<void> => {
    if (deckIds.length === 0) {
      setWorkload(null);
      return;
    }
    setIsLoadingWorkload(true);
    try {
      setWorkload(await getPracticeWorkload(deckIds, selectedTagIds));
    } catch (error: unknown) {
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWorkload(false);
    }
  }, [selectedTagIds]);'''
    new='''  const loadWorkload = useCallback(async (deckIds: string[]): Promise<void> => {
    const requestId = ++workloadRequestRef.current;
    if (deckIds.length === 0 && selectedTagIds.length === 0) {
      setWorkload(null);
      setIsLoadingWorkload(false);
      return;
    }
    setIsLoadingWorkload(true);
    try {
      const result = await getPracticeWorkload(deckIds, selectedTagIds);
      if (workloadRequestRef.current === requestId) setWorkload(result);
    } catch (error: unknown) {
      if (workloadRequestRef.current !== requestId) return;
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
    } finally {
      if (workloadRequestRef.current === requestId) setIsLoadingWorkload(false);
    }
  }, [selectedTagIds]);'''
    if old not in t:raise SystemExit('Practice workload anchor changed')
    t=t.replace(old,new,1)
write(p,t)

# Deck detail search executes in SQLite and stale pages cannot overwrite the latest search.
p='src/screens/DecksScreen.tsx';t=read(p)
t=t.replace('import { useEffect, useMemo, useRef, useState } from "react";','import { useEffect, useRef, useState } from "react";')
if 'deckSearchRequestRef' not in t:
    t=t.replace('const loadingDeckRef = useRef(false);','const loadingDeckRef = useRef(false);\n  const deckSearchRequestRef = useRef(0);\n  const [isSearchingCards, setIsSearchingCards] = useState(false);')
start=t.index('  const visibleCards = useMemo(')
end=t.index('\n\n  async function openDeck',start)
t=t[:start]+'  const visibleCards = cards;'+t[end:]
    # Search after selected deck/view is established.
anchor='  async function openDeck(deckId: string): Promise<void> {'
effect='''  useEffect(() => {
    if (view !== "detail" || !selectedDeckId) return;
    const requestId = ++deckSearchRequestRef.current;
    const timer = window.setTimeout(() => {
      setIsSearchingCards(true);
      void listDeckCards(selectedDeckId, search, 50, 0)
        .then((page) => {
          if (deckSearchRequestRef.current !== requestId) return;
          setCards(page.cards);
          setCardTotal(page.totalCount);
        })
        .catch((error: unknown) => {
          if (deckSearchRequestRef.current === requestId) setErrorMessage(getErrorMessage(error));
        })
        .finally(() => {
          if (deckSearchRequestRef.current === requestId) setIsSearchingCards(false);
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [search, selectedDeckId, view]);

'''
t=t.replace(anchor,effect+anchor,1)
t=t.replace('Promise.all([getDeck(deckId), listDeckCards(deckId, "", 50, 0)])','Promise.all([getDeck(deckId), listDeckCards(deckId, search, 50, 0)])',1)
# First open remains empty search; refresh should preserve current search.
idx=t.find('Promise.all([getDeck(deckId), listDeckCards(deckId, "", 50, 0)])')
if idx>=0:t=t[:idx]+t[idx:].replace('Promise.all([getDeck(deckId), listDeckCards(deckId, "", 50, 0)])','Promise.all([getDeck(deckId), listDeckCards(deckId, search, 50, 0)])',1)
t=t.replace('{isLoadingDeck ? <div className="inline-progress">Opening deck…</div> : null}','{isLoadingDeck ? <div className="inline-progress">Opening deck…</div> : null}\n          {isSearchingCards ? <div className="inline-progress">Searching this deck…</div> : null}')
write(p,t)

# Deletion finalization purges every deleted-deck review/session Analytics source row.
p='src-tauri/src/deletion.rs';t=read(p).replace('fn finalize_expired_in_connection(','pub(crate) fn finalize_expired_in_connection(',1);write(p,t)
p='src-tauri/src/historical_attribution.rs';t=read(p)
start=t.index('pub fn purge_deck_history(');end=t.index('\nfn load_asset_ids_for_notes(',start)
new=r'''pub fn purge_deck_history(connection: &Connection, deck_id: &str) -> Result<(), CommandError> {
    let session_ids = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT psc.session_id
             FROM practice_session_cards psc
             JOIN cards c ON c.id=psc.card_id
             WHERE c.deck_id=?1"
        ).map_err(CommandError::database)?;
        statement.query_map([deck_id],|row|row.get::<_,String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?
    };
    connection.execute(
        "DELETE FROM review_events
         WHERE card_id IN (SELECT id FROM cards WHERE deck_id=?1)
            OR id IN (SELECT review_event_id FROM review_event_attributions WHERE deck_id=?1)",
        [deck_id],
    ).map_err(CommandError::database)?;
    connection.execute(
        "DELETE FROM practice_session_card_snapshots WHERE deck_id=?1",
        [deck_id],
    ).map_err(CommandError::database)?;
    connection.execute(
        "DELETE FROM practice_session_cards
         WHERE card_id IN (SELECT id FROM cards WHERE deck_id=?1)",
        [deck_id],
    ).map_err(CommandError::database)?;
    for session_id in session_ids {
        connection.execute(
            "UPDATE practice_sessions SET
                 card_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1),
                 viewed_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND viewed_at IS NOT NULL),
                 revealed_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND revealed_at IS NOT NULL),
                 rated_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND rated_at IS NOT NULL),
                 correct_count=(SELECT COUNT(*) FROM practice_session_cards WHERE session_id=?1 AND rating>1)
             WHERE id=?1",
            [session_id],
        ).map_err(CommandError::database)?;
    }
    connection.execute(
        "DELETE FROM practice_sessions
         WHERE NOT EXISTS (SELECT 1 FROM practice_session_cards psc WHERE psc.session_id=practice_sessions.id)
           AND NOT EXISTS (SELECT 1 FROM review_events r WHERE r.session_id=practice_sessions.id)",
        [],
    ).map_err(CommandError::database)?;
    Ok(())
}
'''
t=t[:start]+new+t[end:]
write(p,t)

# Register module-local regression suites.
for p,module in [('src-tauri/src/decks.rs','stage36_tests'),('src-tauri/src/analytics.rs','stage36_tests')]:
    t=read(p)
    declaration=f'\n#[cfg(test)]\nmod {module};\n'
    if declaration.strip() not in t:t+=declaration
    write(p,t)

write('src-tauri/src/decks/stage36_tests.rs',r'''use super::*;
use crate::{cards::validate_content,scheduler::insert_new_schedule};
use rusqlite::params;
use serde_json::json;

fn add_card(connection:&mut Connection,deck_id:&str,label:&str,suspended:Option<i64>)->Result<String,CommandError>{
    let content=validate_content(json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":label}]}]}),json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"A"}]}]}),label.to_string(),"A".to_string())?;
    let note=generate_id(connection)?;let card=generate_id(connection)?;let now=unix_time_millis()?;
    connection.execute("INSERT INTO notes(id,deck_id,note_type,content_format,content_schema_version,front_doc,back_doc,front_text,back_text,duplicate_hash,created_at,updated_at,revision) VALUES(?1,?2,'basic','tiptap-json',1,?3,?4,?5,?6,'',?7,?7,1)",params![note,deck_id,content.front_doc_json,content.back_doc_json,content.front_text,content.back_text,now]).map_err(CommandError::database)?;
    connection.execute("INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at,suspended_at) VALUES(?1,?2,?3,'forward',0,?4,?4,?5)",params![card,note,deck_id,now,suspended]).map_err(CommandError::database)?;
    insert_new_schedule(connection,&card,now)?;Ok(card)
}

#[test]
fn archive_suspends_every_card_and_unarchive_preserves_manual_suspensions(){
    let database=Database::open_in_memory().expect("database");
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
''')

write('src-tauri/src/analytics/stage36_tests.rs',r'''use super::*;
use crate::{
    cards::validate_content,
    decks::{create_deck_in_connection,CreateDeckInput},
    historical_attribution::{insert_session_snapshots,prepare_attributions},
    practice::{submit_review_in_connection,SubmitReviewInput},
    scheduler::{insert_new_schedule,load_schedule},
    util::{generate_id,unix_time_millis},
};
use rusqlite::params;
use serde_json::json;

#[test]
fn archived_history_uses_current_names_and_final_deletion_purges_every_analytics_source(){
    let database=Database::open_in_memory().expect("database");
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
''')

write('docs/development/scheduling-analytics-query-testing.md',r'''# Scheduling, Analytics, and projection testing

Stages 3.6–3.8 use one local-calendar context and separate current inventory from historical review evidence.

## Invariants

- **Studyable due** means the card, note, and deck are current; the deck is not archived; the card is not suspended; burial has expired; and `due_at <= now`.
- FSRS due values remain exact instants. Calendar dates are used only for burial, date-only rescheduling, daily limits, recency, and Analytics buckets.
- Archived decks have zero current studyable inventory but retain historical Analytics.
- Deck and tag IDs remain historical dimensions; retained rows provide their current names.
- Soft-deleted decks are immediately excluded from every Analytics query. When Undo expires, their review events, attribution rows, and practice-session card rows are removed.
- List commands return bounded summaries. Full documents/assets are loaded only for an opened card/template or a bounded practice window.

## Fast gate

```bash
bash ./scripts/check-fast.sh
```

This checks the source architecture, React behavior, strict TypeScript, Rust formatting, and all-target compilation.

## Full gate

```bash
./scripts/check.sh
```

The full gate adds Clippy with warnings denied, every Rust/migration/backup test, the standalone Tauri build, dependency verification, and native restart persistence.

## Manual acceptance

Use an isolated XDG profile. Verify:

1. A buried or suspended overdue card is not counted as Ready now in Decks, Search/Filter, Practice, or Analytics.
2. Bury until tomorrow becomes studyable at the next local date boundary.
3. Reschedule presets and custom dates resolve in Rust; selecting today means due now.
4. Archive a deck. Every previously active card becomes suspended, it leaves current workload, and its historical Analytics remains.
5. Unarchive it. Cards suspended by archive resume; cards manually suspended beforehand stay suspended.
6. Rename the archived deck and a historical tag. Analytics displays the new names without moving review counts.
7. Delete a reviewed deck. It disappears from Analytics immediately. Let Undo expire, restart, and confirm its review/session history is gone.
8. Search a large deck and confirm only one bounded page is transferred. Open a card to load its full rich document.
9. Start a session with more than 50 cards and proceed past the first window.
10. Back up and restore the library; attribution and archive state remain. Exporting `.studydeck` still excludes learner history.
''')

write('scripts/verify-stage36-38.mjs',r'''import {readFile,readdir} from "node:fs/promises";import{join}from"node:path";import{fileURLToPath}from"node:url";const root=fileURLToPath(new URL("..",import.meta.url));const read=(...p)=>readFile(join(root,...p),"utf8");const [migration,database,lib,calendar,studyability,workload,search,practice,analytics,history,decks,deletion,ipc,cards,templates]=await Promise.all([read("src-tauri","migrations","0014_study_calendar_history_projections.sql"),read("src-tauri","src","database.rs"),read("src-tauri","src","lib.rs"),read("src-tauri","src","study_calendar.rs"),read("src-tauri","src","studyability.rs"),read("src-tauri","src","workload.rs"),read("src-tauri","src","authoring","search_and_tags.rs"),read("src-tauri","src","practice_selection.rs"),read("src-tauri","src","analytics.rs"),read("src-tauri","src","historical_attribution.rs"),read("src-tauri","src","decks.rs"),read("src-tauri","src","deletion.rs"),read("src","ipc.ts"),read("src-tauri","src","card_projections.rs"),read("src-tauri","src","template_projections.rs")]);const failures=[];for(const f of ["practice_session_card_snapshots","review_event_attributions","review_event_tag_attributions","deck_archive_cards"])if(!migration.includes(f))failures.push(`migration 14 missing ${f}`);if(!database.includes("LATEST_SCHEMA_VERSION: i64 = 14"))failures.push("schema 14 is not latest");for(const m of ["study_calendar","studyability","historical_attribution","card_projections","template_projections"])if(!lib.includes(`mod ${m};`))failures.push(`native module missing ${m}`);for(const c of ["list_archived_decks","archive_deck","unarchive_deck","list_deck_cards","list_card_template_summaries","get_practice_card_window"])if(!lib.includes(c)||!ipc.includes(`\"${c}\"`))failures.push(`bounded command missing ${c}`);for(const f of ["tomorrow_start_utc_millis","resolve_date_or_now","range_start_date"])if(!calendar.includes(f))failures.push(`calendar contract missing ${f}`);if(!studyability.includes("learning_signal_cte")||!studyability.includes("studyable_due"))failures.push("shared studyability projection missing");for(const source of [workload,search,analytics])if(source.includes("86_400_000"))failures.push("native calendar semantics still use fixed 24-hour arithmetic");if(search.includes("render_stored_card_presentation(connection")||search.includes("needs_more_practice(connection"))failures.push("Search page still performs per-result queries");for(const f of ["ROW_NUMBER() OVER","LIMIT","planned_scheduled_cards_scoped"])if(!workload.includes(f))failures.push(`bounded workload missing ${f}`);for(const f of ["PRACTICE_WINDOW_SIZE","get_practice_card_window","prepare_attributions"])if(!practice.includes(f))failures.push(`bounded practice missing ${f}`);if(!analytics.includes("JOIN decks d ON d.id=a.deck_id AND d.deleted_at IS NULL"))failures.push("historical Analytics does not exclude deleted decks");if(!analytics.includes("COALESCE(t.name")||!analytics.includes("d.name"))failures.push("historical dimensions do not resolve current names");for(const f of ["purge_deck_history","DELETE FROM review_events","DELETE FROM practice_session_cards"])if(!history.includes(f))failures.push(`deleted-deck purge missing ${f}`);if(!deletion.includes("purge_deck_history"))failures.push("deletion finalization does not purge Analytics");for(const f of ["deck_archive_cards","suspended_at = NULL","archived_at = NULL"])if(!decks.includes(f))failures.push(`archive ownership missing ${f}`);if(!cards.includes("MAX_PAGE")||!templates.includes("CardTemplateSummaryDto"))failures.push("bounded list projections missing");const helpers=(await readdir(join(root,"scripts"))).filter(n=>n.startsWith("apply-stage36-38-"));if(helpers.length)failures.push(`temporary Stage 3.6-3.8 helpers remain: ${helpers.join(", ")}`);if(failures.length){console.error("Stage 3.6-3.8 verification failed:");for(const f of failures)console.error(`  - ${f}`);process.exit(1)}console.log("OK   Stage 3.6-3.8 use studyable local-calendar semantics, retained current-name history, deleted-deck purging, and bounded projections.");
''')

p='scripts/check-fast.sh';t=read(p)
if 'verify-stage36-38.mjs' not in t:t=t.replace('node "$SCRIPT_DIR/verify-stage34-35.mjs"','node "$SCRIPT_DIR/verify-stage34-35.mjs"\nnode "$SCRIPT_DIR/verify-stage36-38.mjs"')
write(p,t)

print('Applied final race guards, server-side deck search, deletion purge, regressions, docs, and verifier.')
