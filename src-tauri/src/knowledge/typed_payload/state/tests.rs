use super::*;

fn quantity() -> Value {
    let phrase = |text: &str| json!({"text": text, "occurrence": 0});
    json!({"kind":"quantity","subject":phrase("v"),"unit":phrase("m/s"),
        "bounds":[{"label":phrase("≤"),"value":phrase("−2.050e1")}]})
}

#[test]
fn document_annotation_validated_wire_and_stored_bytes_are_unchanged() {
    let statement = "v ≤ −2.050e1 m/s";
    let old = ValidatedPayload::admit(quantity(), statement, KnowledgeKind::Fact).unwrap();
    let new = TypedPayload::admit_or_defer(quantity(), statement, KnowledgeKind::Fact).unwrap();
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        serde_json::to_value(&new).unwrap()
    );
    let restored: TypedPayload = serde_json::from_value(serde_json::to_value(&old).unwrap()).unwrap();
    restored.validate(statement, KnowledgeKind::Fact).unwrap();
    assert!(restored.deferred_rule().is_none());
    assert_eq!(
        old.prompt(statement).unwrap(),
        restored.prompt(statement).unwrap()
    );
    assert_eq!(ValidatedPayload::schema(), TypedPayload::schema());
}

#[test]
fn document_annotation_debt_is_bound_to_statement_kind_draft_and_failure() {
    let statement = "v ≤ −2.050e1 m/s";
    let mut draft = quantity();
    draft["unit"]["text"] = json!("PRIVATE_MISSING_PHRASE");
    let deferred =
        TypedPayload::admit_or_defer(draft.clone(), statement, KnowledgeKind::Fact).unwrap();
    assert_eq!(
        deferred.deferred_rule(),
        Some("knowledge_annotation_selection_absent")
    );
    let stored = serde_json::to_value(&deferred).unwrap();
    assert_eq!(stored["draft"], draft);
    for (path, value) in [
        ("/deferredVersion", json!(2)),
        ("/statementHash", json!(hash("another statement"))),
        ("/kind", json!("process")),
        ("/rule", json!("knowledge_annotation_dependency_cycle")),
        ("/draftHash", json!("0".repeat(64))),
        ("/draftBytes", json!(1)),
        ("/draft", Value::Null),
    ] {
        let mut damaged = stored.clone();
        *damaged.pointer_mut(path).unwrap() = value;
        let decoded: TypedPayload = serde_json::from_value(damaged).unwrap();
        assert!(
            decoded.validate(statement, KnowledgeKind::Fact).is_err(),
            "{path}"
        );
    }
    deferred.validate(statement, KnowledgeKind::Fact).unwrap();
    assert!(deferred
        .validate("v < −2.050e1 m/s", KnowledgeKind::Fact)
        .is_err());
    let prompt = deferred.prompt(statement).unwrap();
    assert_eq!(prompt["annotationState"], "deferred");
    assert_eq!(prompt["gradedBindingsAllowed"], false);
    assert!(!prompt.to_string().contains("PRIVATE"));
    assert!(!deferred.diagnostic().unwrap().to_string().contains("PRIVATE"));
    assert!(TypedPayload::admit(stored, statement, KnowledgeKind::Fact).is_err());
}

#[test]
fn document_annotation_corrupt_validated_data_is_not_reinterpreted_as_deferred() {
    let statement = "v ≤ −2.050e1 m/s";
    let payload = TypedPayload::admit(quantity(), statement, KnowledgeKind::Fact).unwrap();
    let mut damaged = serde_json::to_value(payload).unwrap();
    damaged["version"] = json!(2);
    let restored: TypedPayload = serde_json::from_value(damaged.clone()).unwrap();
    assert!(restored.validate(statement, KnowledgeKind::Fact).is_err());
    assert!(restored.deferred_rule().is_none());
    damaged["deferredVersion"] = json!(1);
    assert!(serde_json::from_value::<TypedPayload>(damaged).is_err());
}

#[test]
fn document_annotation_oversized_draft_preserves_explicit_fingerprint_not_fake_structure() {
    let draft = json!({"unexpected": "x".repeat(MAX_RETAINED_DRAFT_BYTES)});
    let statement = "A triangle has three sides.";
    let payload =
        TypedPayload::admit_or_defer(draft.clone(), statement, KnowledgeKind::Fact).unwrap();
    payload.validate(statement, KnowledgeKind::Fact).unwrap();
    let stored = serde_json::to_value(&payload).unwrap();
    assert!(stored["draft"].is_null());
    assert_eq!(stored["draftHash"], hash(&draft.to_string()));
    assert_eq!(stored["draftBytes"], draft.to_string().len());
    assert!(stored.to_string().len() < 500);
    assert_eq!(payload.diagnostic().unwrap()["draftRetained"], false);
    assert!(payload.deferred_rule().is_some());
}
