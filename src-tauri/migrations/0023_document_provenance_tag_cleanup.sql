-- Remove legacy provenance tags that older document-generation builds attached
-- automatically to document-generated cards. Subject-matter tags remain intact.
-- Historical tag snapshots are cleaned only when their captured deck is known
-- to be a managed-document generated deck.

DELETE FROM review_event_tag_attributions
WHERE tag_id IN (
    SELECT id
    FROM tags
    WHERE lower(trim(name)) IN ('ai generated', 'pdf')
)
AND review_event_id IN (
    SELECT attribution.review_event_id
    FROM review_event_attributions attribution
    JOIN document_decks provenance
      ON provenance.deck_id = attribution.owner_deck_id_at_review
);

DELETE FROM practice_session_card_tag_snapshots
WHERE tag_id IN (
    SELECT id
    FROM tags
    WHERE lower(trim(name)) IN ('ai generated', 'pdf')
)
AND EXISTS (
    SELECT 1
    FROM practice_session_card_snapshots snapshot
    JOIN document_decks provenance
      ON provenance.deck_id = snapshot.owner_deck_id_at_capture
    WHERE snapshot.session_id = practice_session_card_tag_snapshots.session_id
      AND snapshot.card_id = practice_session_card_tag_snapshots.card_id
);

DELETE FROM note_tags
WHERE tag_id IN (
    SELECT id
    FROM tags
    WHERE lower(trim(name)) IN ('ai generated', 'pdf')
)
AND note_id IN (
    SELECT note.id
    FROM notes note
    JOIN document_decks provenance
      ON provenance.deck_id = note.deck_id
);

DELETE FROM tags
WHERE lower(trim(name)) IN ('ai generated', 'pdf')
  AND NOT EXISTS (
      SELECT 1 FROM note_tags current_tag
      WHERE current_tag.tag_id = tags.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM review_event_tag_attributions review_tag
      WHERE review_tag.tag_id = tags.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM practice_session_card_tag_snapshots session_tag
      WHERE session_tag.tag_id = tags.id
  );
