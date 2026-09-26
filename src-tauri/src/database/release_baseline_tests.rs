use super::*;
use rusqlite::types::ValueRef;

const RELEASE_BASELINE_PATH: &str = "schema/0001_release_baseline.sql";

#[test]
fn release_baseline_is_committed_and_matches_schema30() {
    let mut connection = Connection::open_in_memory().expect("generator database should open");
    configure_connection(&connection).expect("generator connection should configure");
    for migration in MIGRATIONS {
        apply_transactional_migration(&mut connection, migration)
            .expect("legacy public migration should apply");
    }
    crate::data_repairs::apply_required_repairs(
        &mut connection,
        crate::data_repairs::RepairPhase::AfterMigration(LATEST_SCHEMA_VERSION),
    )
    .expect("current public repairs should apply");

    let rendered = render_release_baseline(&connection).expect("release baseline should render");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(RELEASE_BASELINE_PATH);
    match std::fs::read_to_string(&path) {
        Ok(committed) => assert_eq!(
            committed, rendered,
            "The release baseline is stale. Regenerate it from the exact qualified schema-30 source."
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            use base64::Engine as _;
            use sha2::{Digest, Sha256};
            use std::io::{Cursor, Write};
            use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .start_file(
                    "0001_release_baseline.sql",
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .expect("release baseline ZIP member should start");
            writer
                .write_all(rendered.as_bytes())
                .expect("release baseline ZIP payload should write");
            let compressed = writer
                .finish()
                .expect("release baseline ZIP should finish")
                .into_inner();
            let encoded = base64::engine::general_purpose::STANDARD.encode(compressed);
            let chunks = encoded.as_bytes().chunks(6000).collect::<Vec<_>>();
            eprintln!(
                "OSTUDY_RELEASE_BASELINE_SHA256={:x}",
                Sha256::digest(rendered.as_bytes())
            );
            eprintln!("OSTUDY_RELEASE_BASELINE_CHUNKS={}", chunks.len());
            for (index, chunk) in chunks.into_iter().enumerate() {
                let chunk = std::str::from_utf8(chunk).expect("base64 output is ASCII");
                eprintln!("OSTUDY_RELEASE_BASELINE_CHUNK_{index:04}={chunk}");
            }
            panic!(
                "The release baseline is not committed. Capture the generated deflate ZIP chunks."
            );
        }
        Err(error) => panic!("Could not read the release baseline fixture: {error}"),
    }
}

fn render_release_baseline(connection: &Connection) -> Result<String, String> {
    let mut output = String::from(
        "-- oStudy 0.1 release schema baseline.\n\
         -- Generated from the qualified ostudy-public-v1 schema-30 result.\n\
         -- Historical public-v1 migrations remain compatibility input only;\n\
         -- fresh 0.1 libraries execute this baseline directly.\n\n\
         PRAGMA foreign_keys = ON;\n\n",
    );

    let mut statement = connection
        .prepare(
            "SELECT type, name, RTRIM(sql, ';') FROM sqlite_schema
             WHERE sql IS NOT NULL
               AND name NOT LIKE 'sqlite_%'
               AND name NOT GLOB 'card_search_fts_*'
             ORDER BY CASE type
                        WHEN 'table' THEN 0
                        WHEN 'view' THEN 1
                        WHEN 'index' THEN 2
                        WHEN 'trigger' THEN 3
                        ELSE 4
                      END,
                      name",
        )
        .map_err(|error| format!("Could not inspect the current schema: {error}"))?;
    let objects = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Could not read the current schema: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the current schema: {error}"))?;

    for (_, _, sql) in &objects {
        output.push_str(sql.trim());
        output.push_str(";\n\n");
    }

    let table_names = objects
        .iter()
        .filter(|(object_type, name, _)| {
            object_type == "table"
                && name != "schema_metadata"
                && name != "schema_migrations"
                && name != "data_repairs"
                && name != "card_search_fts"
        })
        .map(|(_, name, _)| name.as_str())
        .collect::<Vec<_>>();
    for table in table_names {
        append_seed_rows(connection, table, &mut output)?;
    }

    Ok(output)
}

fn append_seed_rows(
    connection: &Connection,
    table: &str,
    output: &mut String,
) -> Result<(), String> {
    let identifier = quote_identifier(table);
    let statement = connection
        .prepare(&format!("SELECT * FROM {identifier}"))
        .map_err(|error| format!("Could not inspect release seed table {table}: {error}"))?;
    let column_count = statement.column_count();
    if column_count == 0 {
        return Ok(());
    }
    let columns = statement
        .column_names()
        .into_iter()
        .map(quote_identifier)
        .collect::<Vec<_>>();
    let order = (1..=column_count)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut ordered = connection
        .prepare(&format!("SELECT * FROM {identifier} ORDER BY {order}"))
        .map_err(|error| format!("Could not order release seed table {table}: {error}"))?;
    let mut rows = ordered
        .query([])
        .map_err(|error| format!("Could not read release seed table {table}: {error}"))?;
    let mut wrote_header = false;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not advance release seed table {table}: {error}"))?
    {
        if !wrote_header {
            output.push_str("-- Seed rows retained by a fresh schema-30 library.\n");
            wrote_header = true;
        }
        output.push_str("INSERT INTO ");
        output.push_str(&identifier);
        output.push_str(" (");
        output.push_str(&columns.join(", "));
        output.push_str(") VALUES (");
        for index in 0..column_count {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str(&sql_literal(row.get_ref(index).map_err(|error| {
                format!("Could not read release seed value: {error}")
            })?)?);
        }
        output.push_str(");\n");
    }
    if wrote_header {
        output.push('\n');
    }
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sql_literal(value: ValueRef<'_>) -> Result<String, String> {
    match value {
        ValueRef::Null => Ok("NULL".to_string()),
        ValueRef::Integer(value) => Ok(value.to_string()),
        ValueRef::Real(value) if value.is_finite() => Ok(format!("{value:.17}")),
        ValueRef::Real(_) => {
            Err("Release seed rows cannot contain non-finite numbers.".to_string())
        }
        ValueRef::Text(value) => {
            let value = std::str::from_utf8(value)
                .map_err(|error| format!("Release seed text is not UTF-8: {error}"))?;
            Ok(format!("'{}'", value.replace('\'', "''")))
        }
        ValueRef::Blob(value) => {
            let hex = value
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            Ok(format!("X'{hex}'"))
        }
    }
}
