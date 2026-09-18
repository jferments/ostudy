use super::super::{contracts::SourceRecord, evidence::EvidenceCatalog};
use super::decode::{AnalysisReport, MAX_WINDOW_UNITS};
use serde_json::{json, Value};

fn proposal(catalog: &EvidenceCatalog) -> Value {
    json!({"type":"object","additionalProperties":false,
    "required":["title","kind","statement","qualifications","topic","importance","evidenceIds"],
    "properties":{
        "title":{"type":"string","minLength":1,"maxLength":200},
        "kind":{"type":"string","enum":["definition","relationship","comparison","process","fact"]},
        "statement":{"type":"string","minLength":1,"maxLength":3000},
        "qualifications":{"type":"array","maxItems":8,"items":{"type":"string","minLength":1,"maxLength":1000}},
        "topic":{"type":"string","maxLength":500},
        "details":super::super::typed_payload::TypedPayload::schema(),
        "importance":{"type":"string","enum":["core","supporting","detail"]},
        "evidenceIds":{"type":"array","minItems":1,"maxItems":8,"uniqueItems":true,"items":{"type":"string","enum":catalog.aliases()}}
    }})
}

fn dispositions(sources: &[SourceRecord]) -> Value {
    json!({"type":"array","maxItems":sources.len(),"uniqueItems":true,
        "items":{"type":"string","enum":sources.iter().map(|source| &source.id).collect::<Vec<_>>()}})
}

pub(super) fn initial(catalog: &EvidenceCatalog, sources: &[SourceRecord]) -> Value {
    json!({"type":"object","additionalProperties":false,"required":["units","noLearningSourceIds"],
        "properties":{"units":{"type":"array","maxItems":MAX_WINDOW_UNITS,"items":proposal(catalog)},
            "noLearningSourceIds":dispositions(sources)}})
}

pub(super) fn repair(
    catalog: &EvidenceCatalog,
    sources: &[SourceRecord],
    report: &AnalysisReport,
) -> Value {
    // A missing slot is still pending, never silently dropped. The native
    // decoder verifies uniqueness even when a provider ignores schema keywords.
    json!({"type":"object","additionalProperties":false,"required":["repairs","additionalUnits","noLearningSourceIds"],
        "properties":{
            "repairs":{"type":"array","maxItems":report.rejected.len(),"items":{
                "type":"object","additionalProperties":false,"required":["slot","unit"],
                "properties":{"slot":{"type":"integer","minimum":0},
                    "unit":{"anyOf":[proposal(catalog),{"type":"null"}]}}}},
            "additionalUnits":{"type":"array","maxItems":if report.missing_sources.is_empty(){0}else{MAX_WINDOW_UNITS},"items":proposal(catalog)},
            "noLearningSourceIds":dispositions(sources)}})
}
