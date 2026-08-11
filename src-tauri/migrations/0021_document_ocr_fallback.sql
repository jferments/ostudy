-- Record extraction degradation and OCR fallback provenance for managed documents.
--
-- OCR is page-selective and optional. Existing documents remain native until a
-- retry under the quality-aware extraction backend determines otherwise.

ALTER TABLE documents
    ADD COLUMN extraction_mode TEXT NOT NULL DEFAULT 'native'
    CHECK(extraction_mode IN ('native', 'page_text_fallback', 'ocr_fallback'));

ALTER TABLE documents
    ADD COLUMN ocr_page_count INTEGER NOT NULL DEFAULT 0
    CHECK(ocr_page_count >= 0);

ALTER TABLE documents
    ADD COLUMN ocr_completed_page_count INTEGER NOT NULL DEFAULT 0
    CHECK(ocr_completed_page_count >= 0);

ALTER TABLE documents
    ADD COLUMN ocr_mean_confidence REAL
    CHECK(ocr_mean_confidence IS NULL
          OR (ocr_mean_confidence >= 0.0 AND ocr_mean_confidence <= 1.0));

ALTER TABLE documents
    ADD COLUMN ocr_engine TEXT NOT NULL DEFAULT ''
    CHECK(length(ocr_engine) <= 120);
