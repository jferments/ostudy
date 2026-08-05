#!/usr/bin/env python3
"""Replace Search/Filter with studyable availability, local dates, and two-query pages."""
from __future__ import annotations
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return(ROOT/p).read_text(encoding='utf-8')
def write(p,c):(ROOT/p).write_text(c,encoding='utf-8')

# Explicit native transport fields.
p='src-tauri/src/authoring/types.rs';t=read(p)
t=t.replace('#[serde(default = "default_due_filter")]\n    pub due_filter: String,','#[serde(default = "default_availability_filter")]\n    pub availability_filter: String,')
write(p,t)

# Pure presentation projection in the lexical authoring module.
p='src-tauri/src/authoring/stage3_v2/generated_cards.rs';t=read(p)
marker='pub(crate) fn presentation_variant_for_projected('
if 'fn render_projected_text_presentation(' not in t:
    helper=r'''fn render_projected_text_presentation(
    note_type: &str,
    direction: &str,
    front_text: &str,
    back_text: &str,
    cloze_source: &str,
    cloze_mode: &str,
    reps: i64,
) -> Result<(String, String, String, String), CommandError> {
    if note_type == "basic" {
        return match direction {
            "forward" => Ok(("basic".to_string(), "Basic".to_string(), front_text.to_string(), back_text.to_string())),
            "reverse" => Ok(("basic".to_string(), "Reverse".to_string(), back_text.to_string(), front_text.to_string())),
            _ => Err(CommandError::new("invalid_generated_card", "A Basic note contains an unsupported generated-card direction.")),
        };
    }
    if note_type != "cloze" || direction != "forward" {
        return Err(CommandError::new("invalid_generated_card", "The Fill in the blank note contains an unsupported active card identity."));
    }
    validate_cloze_mode(cloze_mode)?;
    let parsed = parse_cloze(cloze_source)?;
    let target = if cloze_mode == "all_together" {
        None
    } else {
        let index = usize::try_from(reps.max(0)).unwrap_or(0) % parsed.groups.len();
        Some(parsed.groups[index])
    };
    let label = if cloze_mode == "all_together" {
        "Fill in the blank · all blanks".to_string()
    } else {
        format!("Fill in the blank · {} alternating {}", parsed.groups.len(), if parsed.groups.len() == 1 { "blank" } else { "blanks" })
    };
    Ok(("cloze".to_string(), label, render_cloze_question(&parsed, target), render_cloze_answer(&parsed, target)))
}

'''
    t=t.replace(marker,helper+marker,1)
write(p,t)

p='src-tauri/src/authoring/search_and_tags.rs';original=read(p)
start=original.index('struct SearchSql {')
end=original.index('\nfn hydrate_organized_card(',start)
new=r'''struct SearchSql {
    joins: String,
    where_sql: String,
    values: Vec<SqlValue>,
    order_sql: &'static str,
}

fn effective_deck_ids(deck_ids: &[String]) -> Result<Vec<String>, CommandError> {
    let mut unique=BTreeSet::new();
    for deck_id in deck_ids { validate_entity_id(deck_id,"deck")?; unique.insert(deck_id.clone()); }
    if unique.len()>MAX_SEARCH_DECKS { return Err(CommandError::new("too_many_deck_filters",format!("Filter by at most {MAX_SEARCH_DECKS} decks."))); }
    Ok(unique.into_iter().collect())
}

fn list_tags_in_connection(connection:&Connection,deck_ids:&[String])->Result<Vec<TagDto>,CommandError>{
    let mut values:Vec<SqlValue>=Vec::new();
    let mut sql=String::from("SELECT t.id,t.name,COUNT(DISTINCT n.id) FROM tags t JOIN note_tags nt ON nt.tag_id=t.id JOIN notes n ON n.id=nt.note_id JOIN decks d ON d.id=n.deck_id WHERE n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL");
    if !deck_ids.is_empty(){let p=deck_ids.iter().map(|id|format!("?{}",push_value(&mut values,id.clone()))).collect::<Vec<_>>().join(",");sql.push_str(&format!(" AND n.deck_id IN ({p})"));}
    sql.push_str(" GROUP BY t.id,t.name HAVING COUNT(DISTINCT n.id)>0 ORDER BY t.name COLLATE NOCASE");
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;
    let rows=statement.query_map(params_from_iter(values.iter()),tag_from_row).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    Ok(rows)
}
fn tag_from_row(row:&rusqlite::Row<'_>)->rusqlite::Result<TagDto>{Ok(TagDto{id:row.get(0)?,name:row.get(1)?,source_note_count:row.get(2)?})}

fn validate_search_input(input:&SearchCardsInput)->Result<(),CommandError>{
    effective_deck_ids(&input.deck_ids)?;
    if input.query.chars().count()>MAX_SEARCH_QUERY_CHARS{return Err(CommandError::new("search_too_long",format!("Search text cannot exceed {MAX_SEARCH_QUERY_CHARS} characters.")));}
    if input.tag_ids.len()>MAX_SEARCH_TAGS{return Err(CommandError::new("too_many_tag_filters",format!("Filter by at most {MAX_SEARCH_TAGS} tags.")));}
    for tag_id in &input.tag_ids{validate_entity_id(tag_id,"tag")?;}
    if !matches!(input.availability_filter.as_str(),"all"|"ready_now"|"scheduled_later"){return Err(CommandError::new("invalid_availability_filter","Choose all cards, cards ready now, or cards scheduled later."));}
    if !matches!(input.status_filter.as_str(),"all"|"active"|"suspended"|"buried"|"needs_more_practice"){return Err(CommandError::new("invalid_status_filter","Choose active, suspended, buried, or Needs more practice cards."));}
    if input.availability_filter!="all"&&matches!(input.status_filter.as_str(),"suspended"|"buried"){return Err(CommandError::new("incompatible_card_filters","Suspended or buried cards cannot also be Ready now or Scheduled later."));}
    if !matches!(input.reviewed_filter.as_str(),"all"|"never"|"today"|"past_7_dates"|"past_30_dates"|"before_30_dates"){return Err(CommandError::new("invalid_reviewed_filter","Choose one of the local-calendar study recency filters."));}
    if !matches!(input.sort.as_str(),"relevance"|"updated_desc"|"question_asc"|"due_asc"|"last_reviewed_desc"){return Err(CommandError::new("invalid_card_sort","Choose one of the available card sort orders."));}
    if !(1..=MAX_SEARCH_LIMIT).contains(&input.limit)||input.offset<0{return Err(CommandError::new("invalid_search_page","The requested card-results page is invalid."));}
    Ok(())
}

fn build_search_sql(input:&SearchCardsInput,context:&crate::study_calendar::StudyTimeContext)->Result<SearchSql,CommandError>{
    let fts_query=build_fts_query(&input.query);let mut joins=String::from(" LEFT JOIN learning_signals ls ON ls.card_id=c.id");
    if fts_query.is_some(){joins.push_str(" JOIN card_search_fts ON card_search_fts.rowid=n.rowid");}
    let mut conditions=vec!["c.deleted_at IS NULL".to_string(),"n.deleted_at IS NULL".to_string(),"d.deleted_at IS NULL".to_string(),"d.archived_at IS NULL".to_string()];let mut values:Vec<SqlValue>=Vec::new();
    let deck_ids=effective_deck_ids(&input.deck_ids)?;if !deck_ids.is_empty(){let p=deck_ids.into_iter().map(|id|format!("?{}",push_value(&mut values,id))).collect::<Vec<_>>().join(",");conditions.push(format!("c.deck_id IN ({p})"));}
    if let Some(query)=fts_query{conditions.push(format!("card_search_fts MATCH ?{}",push_value(&mut values,query)));}
    for tag_id in &input.tag_ids{let i=push_value(&mut values,tag_id.clone());conditions.push(format!("EXISTS(SELECT 1 FROM note_tags filter_nt WHERE filter_nt.note_id=n.id AND filter_nt.tag_id=?{i})"));}
    let now_index=push_value(&mut values,context.now_utc_millis);
    match input.status_filter.as_str(){
        "active"=>{conditions.push("c.suspended_at IS NULL".to_string());conditions.push(format!("(c.buried_until IS NULL OR c.buried_until<=?{now_index})"));},
        "suspended"=>conditions.push("c.suspended_at IS NOT NULL".to_string()),
        "buried"=>{conditions.push("c.suspended_at IS NULL".to_string());conditions.push(format!("c.buried_until>?{now_index}"));},
        "needs_more_practice"=>{conditions.push("c.suspended_at IS NULL".to_string());conditions.push(format!("(c.buried_until IS NULL OR c.buried_until<=?{now_index})"));conditions.push("COALESCE(ls.needs_more_practice,0)=1".to_string());},_=>{}
    }
    match input.availability_filter.as_str(){
        "ready_now"=>{conditions.push("c.suspended_at IS NULL".to_string());conditions.push(format!("(c.buried_until IS NULL OR c.buried_until<=?{now_index})"));conditions.push(format!("s.due_at<=?{now_index}"));},
        "scheduled_later"=>{conditions.push("c.suspended_at IS NULL".to_string());conditions.push(format!("(c.buried_until IS NULL OR c.buried_until<=?{now_index})"));conditions.push(format!("s.due_at>?{now_index}"));},_=>{}
    }
    let today=context.today_start_utc_millis;let seven=context.resolve_date_start(context.date_after_days(-6)?)?;let thirty=context.resolve_date_start(context.date_after_days(-29)?)?;
    match input.reviewed_filter.as_str(){
        "never"=>conditions.push("s.reps=0".to_string()),
        "today"=>{conditions.push("s.reps>0".to_string());conditions.push(format!("s.last_review_at>=?{}",push_value(&mut values,today)));},
        "past_7_dates"=>{conditions.push("s.reps>0".to_string());conditions.push(format!("s.last_review_at>=?{}",push_value(&mut values,seven)));},
        "past_30_dates"=>{conditions.push("s.reps>0".to_string());conditions.push(format!("s.last_review_at>=?{}",push_value(&mut values,thirty)));},
        "before_30_dates"=>{conditions.push("s.reps>0".to_string());conditions.push(format!("s.last_review_at<?{}",push_value(&mut values,thirty)));},_=>{}
    }
    let order_sql=match input.sort.as_str(){"relevance" if fts_query_is_present(&input.query)=>"bm25(card_search_fts) ASC,n.updated_at DESC,c.template_ordinal ASC","question_asc"=>"n.front_text COLLATE NOCASE,n.updated_at DESC,c.template_ordinal ASC","due_asc"=>"s.due_at,n.updated_at DESC,c.template_ordinal ASC","last_reviewed_desc"=>"CASE WHEN s.reps>0 THEN 0 ELSE 1 END,s.last_review_at DESC,n.updated_at DESC,c.template_ordinal ASC",_=>"n.updated_at DESC,n.created_at DESC,c.template_ordinal,c.direction"};
    Ok(SearchSql{joins,where_sql:conditions.join(" AND "),values,order_sql})
}

fn search_cards_in_connection(connection:&Connection,input:SearchCardsInput)->Result<CardSearchPageDto,CommandError>{
    let context=crate::study_calendar::StudyTimeContext::capture()?;let mut search=build_search_sql(&input,&context)?;
    let count_sql=format!("WITH {signals} SELECT COUNT(*) FROM notes n JOIN cards c ON c.note_id=n.id JOIN decks d ON d.id=c.deck_id JOIN card_schedules s ON s.card_id=c.id {joins} WHERE {where_sql}",signals=crate::studyability::learning_signal_cte(),joins=search.joins,where_sql=search.where_sql);
    let total_count=connection.query_row(&count_sql,params_from_iter(search.values.iter()),|row|row.get(0)).map_err(CommandError::database)?;
    let limit_index=push_value(&mut search.values,input.limit);let offset_index=push_value(&mut search.values,input.offset);
    let query_sql=format!("WITH {signals} SELECT c.id,n.id,c.deck_id,d.name,COALESCE((SELECT group_concat(name,char(31)) FROM (SELECT t.name name FROM note_tags nt JOIN tags t ON t.id=nt.tag_id WHERE nt.note_id=n.id ORDER BY t.name COLLATE NOCASE)),''),s.due_at,CASE WHEN s.reps>0 THEN s.last_review_at ELSE NULL END,s.reps,c.suspended_at,c.buried_until,n.created_at,n.updated_at,n.note_type,c.direction,n.front_text,n.back_text,n.cloze_source,n.cloze_mode,COALESCE(ls.needs_more_practice,0) FROM notes n JOIN cards c ON c.note_id=n.id JOIN decks d ON d.id=c.deck_id JOIN card_schedules s ON s.card_id=c.id {joins} WHERE {where_sql} ORDER BY {order_sql} LIMIT ?{limit_index} OFFSET ?{offset_index}",signals=crate::studyability::learning_signal_cte(),joins=search.joins,where_sql=search.where_sql,order_sql=search.order_sql);
    let mut statement=connection.prepare(&query_sql).map_err(CommandError::database)?;
    let raw=statement.query_map(params_from_iter(search.values.iter()),|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,i64>(5)?,row.get::<_,Option<i64>>(6)?,row.get::<_,i64>(7)?,row.get::<_,Option<i64>>(8)?,row.get::<_,Option<i64>>(9)?,row.get::<_,i64>(10)?,row.get::<_,i64>(11)?,row.get::<_,String>(12)?,row.get::<_,String>(13)?,row.get::<_,String>(14)?,row.get::<_,String>(15)?,row.get::<_,String>(16)?,row.get::<_,String>(17)?,row.get::<_,i64>(18)?))).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    let cards=raw.into_iter().map(|(card_id,note_id,deck_id,deck_name,tags,due,last,reps,suspended,buried,created,updated,note_type,direction,front,back,source,mode,weak)|{let rendered=render_projected_text_presentation(&note_type,&direction,&front,&back,&source,&mode,reps)?;Ok(CardBrowserRowDto{card_id,note_id,deck_id,deck_name,card_type:rendered.0,card_label:rendered.1,front_text:rendered.2,back_text:rendered.3,tags:split_tags(&tags),due_at:due,last_reviewed_at:last,review_count:reps,suspended_at:suspended,buried_until:buried,needs_more_practice:weak!=0,created_at:created,updated_at:updated})}).collect::<Result<Vec<_>,CommandError>>()?;
    Ok(CardSearchPageDto{cards,total_count})
}

fn list_matching_cards_in_connection(connection:&Connection,input:SearchCardsInput)->Result<Vec<SelectionCardDto>,CommandError>{
    let context=crate::study_calendar::StudyTimeContext::capture()?;let mut search=build_search_sql(&input,&context)?;let maximum=i64::try_from(MAX_SEARCH_SELECTION+1).unwrap_or(i64::MAX);let limit_index=push_value(&mut search.values,maximum);
    let sql=format!("WITH {signals} SELECT c.id,n.id,c.deck_id FROM notes n JOIN cards c ON c.note_id=n.id JOIN decks d ON d.id=c.deck_id JOIN card_schedules s ON s.card_id=c.id {joins} WHERE {where_sql} ORDER BY {order_sql} LIMIT ?{limit_index}",signals=crate::studyability::learning_signal_cte(),joins=search.joins,where_sql=search.where_sql,order_sql=search.order_sql);
    let mut statement=connection.prepare(&sql).map_err(CommandError::database)?;let cards=statement.query_map(params_from_iter(search.values.iter()),|row|Ok(SelectionCardDto{card_id:row.get(0)?,note_id:row.get(1)?,deck_id:row.get(2)?})).map_err(CommandError::database)?.collect::<Result<Vec<_>,_>>().map_err(CommandError::database)?;
    if cards.len()>MAX_SEARCH_SELECTION{return Err(CommandError::new("too_many_matching_cards",format!("More than {MAX_SEARCH_SELECTION} cards match. Narrow the filters before selecting or practicing all results.")));}Ok(cards)
}

fn fts_query_is_present(query:&str)->bool{build_fts_query(query).is_some()}
fn push_value(values:&mut Vec<SqlValue>,value:impl Into<SqlValue>)->usize{values.push(value.into());values.len()}
fn build_fts_query(value:&str)->Option<String>{let mut tokens=Vec::new();let mut current=String::new();for ch in value.chars(){if ch.is_alphanumeric()||matches!(ch,'_'|'-'){current.push(ch);if current.chars().count()>=MAX_TAG_CHARS{tokens.push(std::mem::take(&mut current));}}else if !current.is_empty(){tokens.push(std::mem::take(&mut current));}if tokens.len()>=20{break;}}if !current.is_empty()&&tokens.len()<20{tokens.push(current);}let quoted=tokens.into_iter().filter(|t|!t.is_empty()).map(|t|format!("\"{}\"*",t.replace('"',"\"\""))).collect::<Vec<_>>();(!quoted.is_empty()).then(||quoted.join(" AND "))}
'''
write(p,original[:start]+new+original[end:])

# Frontend transport and controls use Availability/local-date labels.
p='src/ipc.ts';t=read(p)
t=t.replace('export type DueFilter = "all" | "due" | "not_due";','export type AvailabilityFilter = "all" | "ready_now" | "scheduled_later";')
t=t.replace('''export type ReviewedFilter =
  | "all"
  | "never"
  | "last_1_day"
  | "last_7_days"
  | "last_30_days"
  | "older_30_days";''','''export type ReviewedFilter =
  | "all"
  | "never"
  | "today"
  | "past_7_dates"
  | "past_30_dates"
  | "before_30_dates";''')
t=t.replace('dueFilter: DueFilter;','availabilityFilter: AvailabilityFilter;')
write(p,t)
p='src/screens/CardsScreen.tsx';t=read(p)
t=t.replace('  DueFilter,','  AvailabilityFilter,').replace('const [dueFilter, setDueFilter] = useState<DueFilter>("all");','const [availabilityFilter, setAvailabilityFilter] = useState<AvailabilityFilter>("all");')
t=t.replace('dueFilter,','availabilityFilter,').replace('dueFilter]','availabilityFilter]')
t=t.replace('setDueFilter','setAvailabilityFilter').replace('value={dueFilter}','value={availabilityFilter}')
t=t.replace('<span>Due</span>','<span>Availability</span>')
t=t.replace('<option value="due">Due now</option>\n              <option value="not_due">Not due</option>','<option value="ready_now">Ready now</option>\n              <option value="scheduled_later">Scheduled later</option>')
t=t.replace('<option value="last_1_day">Last day</option>\n              <option value="last_7_days">Last 7 days</option>\n              <option value="last_30_days">Last 30 days</option>\n              <option value="older_30_days">Older than 30 days</option>','<option value="today">Today</option>\n              <option value="past_7_dates">Past 7 local dates</option>\n              <option value="past_30_dates">Past 30 local dates</option>\n              <option value="before_30_dates">Before the past 30 local dates</option>')
write(p,t)

# Tests and all callers intentionally migrate; no stale dueFilter key remains.
for path in list((ROOT/'src').rglob('*.ts'))+list((ROOT/'src').rglob('*.tsx')):
    text=path.read_text(encoding='utf-8')
    text=text.replace('dueFilter: "due"','availabilityFilter: "ready_now"').replace('dueFilter: "not_due"','availabilityFilter: "scheduled_later"').replace('dueFilter: "all"','availabilityFilter: "all"')
    text=text.replace('reviewedFilter: "last_1_day"','reviewedFilter: "today"').replace('reviewedFilter: "last_7_days"','reviewedFilter: "past_7_dates"').replace('reviewedFilter: "last_30_days"','reviewedFilter: "past_30_dates"').replace('reviewedFilter: "older_30_days"','reviewedFilter: "before_30_dates"')
    path.write_text(text,encoding='utf-8')

# Backend default function rename.
for p in ['src-tauri/src/authoring/search_and_tags.rs','src-tauri/src/authoring/types.rs','src-tauri/src/authoring/tests.rs']:
    t=read(p).replace('default_due_filter','default_availability_filter').replace('due_filter:','availability_filter:').replace('.due_filter','.availability_filter')
    write(p,t)

print('Applied bounded Search/Filter availability, local recency, and pure presentation projection.')
