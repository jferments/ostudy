//! Shared application service for compiling document learning plans.
//!
//! Both flashcard and study-guide workflows arrive here only after a validated
//! outline exists. This service projects the outline onto canonical semantic
//! evidence and publishes/reuses one durable `DocumentLearningPlan`; artifact
//! authors never independently reinterpret the document structure.

use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::DocumentOutline,
    database::Database,
    document_domain::DocumentSourceDescriptor,
    document_learning::{
        ensure_document_learning_plan, DocumentLearningPlan, LearningEvidenceChunk,
    },
    document_learning_plan::learning_plan_input_from_outline,
    document_semantics::{SemanticChunk},
    error::CommandError,
};

pub(crate) fn ensure_learning_plan_for_outline(
    database: &Database,
    document_id: &str,
    source: &DocumentSourceDescriptor,
    outline: &DocumentOutline,
    chunks: &[SemanticChunk],
) -> Result<DocumentLearningPlan, CommandError> {
    source.validate()?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The document did not contain canonical semantic evidence for a learning plan.",
        ));
    }
    let input = learning_plan_input_from_outline(outline, source.unit_kind, source.unit_count)?;
    let evidence = chunks
        .iter()
        .map(learning_evidence_chunk)
        .collect::<Result<Vec<_>, _>>()?;
    ensure_document_learning_plan(database, document_id, input, &evidence)
}

pub(crate) fn learning_evidence_chunk(
    chunk: &SemanticChunk,
) -> Result<LearningEvidenceChunk, CommandError> {
    let chunk_index = usize::try_from(chunk.chunk_index).map_err(|_| {
        CommandError::new(
            "document_learning_plan_evidence_invalid",
            "A canonical semantic chunk had an invalid learning-evidence index.",
        )
    })?;
    let source_locator = chunk.source_locator.clone();
    let content_hash = hash_text(&format!(
        "learning-evidence-v1|locator={}|text={}",
        source_locator.stable_identity(),
        chunk.full_text,
    ));
    Ok(LearningEvidenceChunk {
        chunk_index,
        source_locator,
        content_hash,
    })
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    #[test]
    fn evidence_identity_is_source_grounded_not_chunk_index_grounded() {
        let chunk = SemanticChunk {
            chunk_index: 42,
            text: "Preload".to_string(),
            full_text: "Preload affects ventricular filling.".to_string(),
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![3], None).unwrap(),
            element_types: vec!["paragraph".to_string()],
            heading_context: None,
            token_estimate: 10,
        };
        let evidence = learning_evidence_chunk(&chunk).unwrap();
        assert_eq!(evidence.chunk_index, 42);
        assert_eq!(evidence.content_hash.len(), 64);
        assert_eq!(evidence.source_locator.first(), 3);
    }
}
