use super::*;
use crate::knowledge::typed_payload::TypedPayload;
use serde_json::json;

fn selection(text: &str, occurrence: usize) -> Value {
    json!({"text": text, "occurrence": occurrence})
}

fn quantity(subject: &str, operator: &str, value: &str) -> Value {
    json!({
        "kind": "quantity",
        "subject": selection(subject, 0),
        "bounds": [{"label": selection(operator, 0), "value": selection(value, 0)}],
        "unit": null
    })
}

fn deferred(value: Value, statement: &str, kind: KnowledgeKind) -> TypedPayload {
    // Simulate a valid historical receipt, before first-pass normalization.
    let error = ValidatedPayload::admit(value.clone(), statement, kind).unwrap_err();
    let encoded = value.to_string();
    let result = TypedPayload::Deferred(Box::new(super::super::DeferredAnnotation {
        deferred_version: 1,
        kind,
        statement_hash: crate::knowledge::contracts::hash(statement),
        rule: super::super::rejection_rule(&error).to_string(),
        draft_hash: crate::knowledge::contracts::hash(&encoded),
        draft_bytes: encoded.len(),
        draft: (encoded.len() <= super::super::MAX_RETAINED_DRAFT_BYTES).then_some(value),
    }));
    result.validate(statement, kind).unwrap();
    result
}

#[test]
fn document_local_annotation_repairs_unique_occurrence_and_is_idempotent() {
    let statement = "Sides = 3";
    let mut value = quantity("Sides", "=", "3");
    value["subject"]["occurrence"] = json!(1);
    let original = deferred(value, statement, KnowledgeKind::Fact);
    let bytes = serde_json::to_value(&original).unwrap();
    let repaired = original
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .unwrap();
    repaired.validate(statement, KnowledgeKind::Fact).unwrap();
    assert!(repaired.deferred_rule().is_none());
    assert!(repaired
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .is_none());
    assert_eq!(serde_json::to_value(original).unwrap(), bytes);
    assert_eq!(repaired.prompt(statement).unwrap()["bounds"][0]["value"], "3");
}

#[test]
fn document_local_annotation_repairs_missing_occurrence_only_when_unique() {
    for occurrence in [None, Some(Value::Null), Some(json!(65))] {
        let statement = "Sides = 3";
        let mut value = quantity("Sides", "=", "3");
        match occurrence {
            Some(value_at_key) => value["subject"]["occurrence"] = value_at_key,
            None => {
                value["subject"]
                    .as_object_mut()
                    .unwrap()
                    .remove("occurrence")
                    .unwrap();
            }
        }
        assert!(deferred(value, statement, KnowledgeKind::Fact)
            .repair_locally(statement, KnowledgeKind::Fact)
            .unwrap()
            .is_some());
    }
}

#[test]
fn document_local_annotation_keeps_repeated_phrase_selection_and_literal_values() {
    let statement = "x = 1; x = 2";
    let mut value = quantity("x", "=", "2");
    value["subject"]["occurrence"] = json!(1);
    value["bounds"][0]["label"]["occurrence"] = json!(1);
    value["bounds"][0]["value"]["occurrence"] = json!(1);
    let repaired = deferred(value, statement, KnowledgeKind::Fact)
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .unwrap();
    let encoded = serde_json::to_value(&repaired).unwrap();
    assert_eq!(encoded["shape"]["subject"]["start"], 7);
    assert_eq!(encoded["shape"]["bounds"][0]["label"]["start"], 9);
    assert_eq!(repaired.prompt(statement).unwrap()["bounds"][0]["value"], "2");
}

#[test]
fn document_local_annotation_does_not_guess_between_repeated_phrases() {
    let statement = "x = 1; x = 2";
    for occurrence in [None, Some(json!(2))] {
        let mut value = quantity("x", "=", "2");
        match occurrence {
            Some(value_at_key) => value["subject"]["occurrence"] = value_at_key,
            None => {
                value["subject"]
                    .as_object_mut()
                    .unwrap()
                    .remove("occurrence")
                    .unwrap();
            }
        }
        let original = deferred(value, statement, KnowledgeKind::Fact);
        let bytes = serde_json::to_value(&original).unwrap();
        assert!(original
            .repair_locally(statement, KnowledgeKind::Fact)
            .unwrap()
            .is_none());
        assert_eq!(serde_json::to_value(original).unwrap(), bytes);
    }
}

#[test]
fn document_local_annotation_preserves_unicode_sign_operator_and_unit() {
    let statement = "温度 ≤ −5 °C.";
    let mut value = quantity("温度", "≤", "−5");
    value["subject"]["occurrence"] = json!(1);
    value["unit"] = selection("°C", 0);
    let repaired = deferred(value, statement, KnowledgeKind::Fact)
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .unwrap();
    let prompt = repaired.prompt(statement).unwrap();
    assert_eq!(prompt["subject"], "温度");
    assert_eq!(prompt["bounds"][0]["label"], "≤");
    assert_eq!(prompt["bounds"][0]["value"], "−5");
    assert_eq!(prompt["unit"], "°C");
}

#[test]
fn document_local_annotation_cannot_repair_semantics_after_fixing_occurrence() {
    for (statement, subject, operator, value, kind) in [
        ("x = -5", "x", "=", "5", KnowledgeKind::Fact),
        ("x <= 5", "x", "<", "5", KnowledgeKind::Fact),
        ("x = 5", "absent", "=", "5", KnowledgeKind::Fact),
        ("x = 5", "x", "=", "5", KnowledgeKind::Process),
    ] {
        let mut value = quantity(subject, operator, value);
        value["subject"]["occurrence"] = json!(1);
        let original = deferred(value, statement, kind);
        assert!(original.repair_locally(statement, kind).unwrap().is_none());
        assert!(original.deferred_rule().is_some());
    }
}

#[test]
fn document_local_annotation_preserves_process_edges_and_rejects_cycles() {
    let statement = "Heat, then cool.";
    let value = json!({
        "kind": "process",
        "steps": [{"text": "Heat"}, selection("cool", 0)],
        "dependencies": [{"before": 0, "after": 1}]
    });
    let original = deferred(value.clone(), statement, KnowledgeKind::Process);
    let repaired = original
        .repair_locally(statement, KnowledgeKind::Process)
        .unwrap()
        .unwrap();
    assert_eq!(
        repaired.prompt(statement).unwrap()["dependencies"],
        value["dependencies"]
    );
    let mut cyclic = value;
    cyclic["dependencies"] = json!([{"before": 0, "after": 1}, {"before": 1, "after": 0}]);
    assert!(deferred(cyclic, statement, KnowledgeKind::Process)
        .repair_locally(statement, KnowledgeKind::Process)
        .unwrap()
        .is_none());
}

#[test]
fn document_local_annotation_keeps_comparison_and_classification_structure() {
    let comparison = json!({
        "kind": "comparison", "criterion": {"text": "speed"},
        "items": [
            {"label": selection("A", 0), "value": selection("fast", 0)},
            {"label": selection("B", 0), "value": selection("slow", 0)}
        ]
    });
    let classification = json!({
        "kind": "classification", "criterion": {"text": "type"},
        "categories": [
            {"label": selection("A", 0), "members": [selection("red", 0)]},
            {"label": selection("B", 0), "members": [selection("blue", 0)]}
        ],
        "exclusion": selection("except overlap", 0)
    });
    for (draft, statement, kind) in [
        (comparison, "speed: A fast; B slow.", KnowledgeKind::Comparison),
        (
            classification,
            "type: A red; B blue, except overlap.",
            KnowledgeKind::Definition,
        ),
    ] {
        let repaired = deferred(draft, statement, kind)
            .repair_locally(statement, kind)
            .unwrap()
            .unwrap();
        repaired.validate(statement, kind).unwrap();
    }
}

#[test]
fn document_local_annotation_rejects_corrupt_binding_before_repair() {
    let statement = "Sides = 3";
    let mut value = quantity("Sides", "=", "3");
    value["subject"]["occurrence"] = json!(1);
    let original = deferred(value, statement, KnowledgeKind::Fact);
    assert!(original
        .repair_locally("Sides = 4", KnowledgeKind::Fact)
        .is_err());
    assert!(original
        .repair_locally(statement, KnowledgeKind::Process)
        .is_err());
    let mut serialized = serde_json::to_value(original).unwrap();
    serialized["draft"]["subject"]["text"] = json!("changed");
    let corrupt: TypedPayload = serde_json::from_value(serialized).unwrap();
    assert!(corrupt
        .repair_locally(statement, KnowledgeKind::Fact)
        .is_err());
}

#[test]
fn document_local_annotation_never_reconstructs_an_unretained_draft() {
    let statement = "Sides = 3";
    let large = json!({"unsupported": "x".repeat(12_001)});
    let original = deferred(large, statement, KnowledgeKind::Fact);
    assert_eq!(original.diagnostic().unwrap()["draftRetained"], false);
    assert!(original
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .is_none());
}

#[test]
fn document_local_annotation_does_not_drop_unknown_fields_to_make_shapes_valid() {
    let statement = "Sides = 3";
    let mut value = quantity("Sides", "=", "3");
    value["subject"]["occurrence"] = json!(1);
    value["subject"]["invented"] = json!("do not discard this silently");
    assert!(deferred(value, statement, KnowledgeKind::Fact)
        .repair_locally(statement, KnowledgeKind::Fact)
        .unwrap()
        .is_none());
}

#[test]
fn document_first_pass_annotation_normalizes_without_changing_strict_admission() {
    let statement = "Sides = 3";
    let mut value = quantity("Sides", "=", "3");
    value["subject"]["occurrence"] = json!(1);
    assert!(TypedPayload::admit(value.clone(), statement, KnowledgeKind::Fact).is_err());
    let admitted = TypedPayload::admit_or_defer(value, statement, KnowledgeKind::Fact).unwrap();
    assert!(admitted.deferred_rule().is_none());
    admitted.validate(statement, KnowledgeKind::Fact).unwrap();
    assert_eq!(admitted.prompt(statement).unwrap()["bounds"][0]["value"], "3");
}
