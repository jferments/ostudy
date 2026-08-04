-- Narrow the active schema after versioned repairs have canonicalized every
-- released data set. Historical migration files remain immutable evidence.
--
-- Legacy text template patterns are no longer an active storage contract.
-- Guards prevent abandoned image-occlusion enum values and nullable rich
-- template layouts from re-entering the canonical database.

ALTER TABLE card_template_rules DROP COLUMN front_pattern;
ALTER TABLE card_template_rules DROP COLUMN back_pattern;

CREATE TRIGGER notes_canonical_type_before_insert
BEFORE INSERT ON notes
WHEN NEW.note_type NOT IN ('basic', 'cloze')
BEGIN
    SELECT RAISE(ABORT, 'unsupported note type');
END;

CREATE TRIGGER notes_canonical_type_before_update
BEFORE UPDATE OF note_type ON notes
WHEN NEW.note_type NOT IN ('basic', 'cloze')
BEGIN
    SELECT RAISE(ABORT, 'unsupported note type');
END;

CREATE TRIGGER cards_canonical_direction_before_insert
BEFORE INSERT ON cards
WHEN NEW.direction NOT IN ('forward', 'reverse')
BEGIN
    SELECT RAISE(ABORT, 'unsupported card direction');
END;

CREATE TRIGGER cards_canonical_direction_before_update
BEFORE UPDATE OF direction ON cards
WHEN NEW.direction NOT IN ('forward', 'reverse')
BEGIN
    SELECT RAISE(ABORT, 'unsupported card direction');
END;

CREATE TRIGGER template_rules_canonical_docs_before_insert
BEFORE INSERT ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;

CREATE TRIGGER template_rules_canonical_docs_before_update
BEFORE UPDATE OF front_doc, back_doc ON card_template_rules
WHEN NEW.front_doc IS NULL OR NEW.back_doc IS NULL
BEGIN
    SELECT RAISE(ABORT, 'template layouts are required');
END;
