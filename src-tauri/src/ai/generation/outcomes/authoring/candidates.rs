//! One bounded admission report per physical response. All rule names originate
//! in native predicates; no generated card, evidence alias or error prose enters
//! this record. Admission and selection are not durable artifact publication.
use serde_json::{json, Value};

use super::super::{current, safe_code, write};
use crate::error::CommandError;

#[derive(Default)]
pub(crate) struct Report {
    pub declared: Option<usize>,
    pub admitted: usize,
    pub boundary: &'static str,
    rejections: Vec<Value>,
}

impl Report {
    pub(crate) fn rejected(
        &mut self,
        index: usize,
        rule: &'static str,
        error: &CommandError,
        retryable: bool,
    ) {
        // Mirrors the parser's hard response bound, not its requested card count.
        if self.rejections.len() < 128 {
            self.rejections.push(json!({
                "candidateIndex": index,
                "rule": rule,
                "errorCode": safe_code(&error.code),
                "retryable": retryable,
            }));
        }
    }

    pub(crate) fn finish(&self, result: &Result<Vec<Value>, CommandError>) {
        let Some(current) = current() else {
            return;
        };
        let Ok(state) = current.lock() else {
            return;
        };
        if state.stage != "flashcard_authoring" {
            return;
        }
        let Some(id) = &state.last_request else {
            // Local-only parsing or lost request accounting must not invent a
            // request, or attach admission to an unrelated earlier response.
            return;
        };
        let selected = result.as_ref().ok().map(Vec::len);
        let examined = self.admitted + self.rejections.len();
        write(
            &state.context,
            id,
            "$.nativeAdmission",
            json!({
                "schemaVersion": 1,
                "state": if result.is_ok() { "returned" } else { "failed" },
                "declaredCandidates": self.declared,
                "examinedCandidates": examined,
                "admittedCandidates": self.admitted,
                "selectedCandidates": selected,
                "unselectedAdmittedCandidates": selected.map(|n| self.admitted.saturating_sub(n)),
                "unexaminedCandidates": self.declared.map(|n| n.saturating_sub(examined)),
                "rejectedCandidates": self.rejections.len(),
                "rejections": self.rejections,
                "failureBoundary": result.as_ref().err().map(|_| self.boundary),
                "errorCode": result.as_ref().err().map(|e| safe_code(&e.code)),
                "persistence": "not_observed",
            }),
        );
    }
}
