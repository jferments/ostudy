#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_tag_payload_relationships() {
        let notes = vec![SourceNoteSignature {
            source_id: "a".repeat(32),
            front_text: "Q".to_string(),
            back_text: "A".to_string(),
            created_at: 1,
            updated_at: 1,
            revision: 1,
        }];
        let tags = vec![ExportTag {
            source_id: "b".repeat(32),
            name: "Pharmacology".to_string(),
            normalized_name: "pharmacology".to_string(),
        }];
        assert!(validate_payload(
            &tags,
            &[(notes[0].source_id.clone(), tags[0].source_id.clone())],
            &notes,
        )
        .is_ok());
        assert!(validate_payload(
            &tags,
            &[("c".repeat(32), tags[0].source_id.clone())],
            &notes,
        )
        .is_err());
    }
    #[test]
    fn optional_tag_tables_round_trip() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ostudy-tag-package-{suffix}.studydeck"));
        let note_id = "a".repeat(32);
        let tag_id = "b".repeat(32);
        {
            let package = Connection::open(&path).expect("package should open");
            package
                .execute_batch(
                    "CREATE TABLE notes (
                         source_id TEXT PRIMARY KEY, front_text TEXT NOT NULL,
                         back_text TEXT NOT NULL, created_at INTEGER NOT NULL,
                         updated_at INTEGER NOT NULL, revision INTEGER NOT NULL
                     ) STRICT;",
                )
                .expect("notes table should be created");
            package
                .execute(
                    "INSERT INTO notes VALUES (?1, 'Question', 'Answer', 1, 1, 1)",
                    [note_id.as_str()],
                )
                .expect("note should be inserted");
        }
        let payload = TagPayload {
            tags: vec![ExportTag {
                source_id: tag_id.clone(),
                name: "Pharmacology".to_string(),
                normalized_name: "pharmacology".to_string(),
            }],
            links: vec![(note_id.clone(), tag_id)],
            notes: Vec::new(),
        };
        append_payload(&path, &payload).expect("tag payload should append");
        let loaded = read_payload(&path).expect("tag payload should read");
        assert_eq!(loaded.tags.len(), 1);
        assert_eq!(loaded.links.len(), 1);
        assert_eq!(loaded.notes[0].source_id, note_id);
        let _ = std::fs::remove_file(path);
    }

}
