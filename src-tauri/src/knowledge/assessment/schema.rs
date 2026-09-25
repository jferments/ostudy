use serde_json::{json, Value};

use crate::knowledge::authoring::KnowledgeCatalog;

use super::{
    policy::{GenerationPolicy, QuestionFamily},
    MAX_ASSESSMENT_UNITS, MAX_COMPONENTS,
};

pub(crate) fn has_supported_generation_family(
    policy: &GenerationPolicy,
    knowledge: &KnowledgeCatalog,
) -> bool {
    if policy.allowed_types.contains(&QuestionFamily::Recall) {
        return true;
    }
    policy
        .allowed_types
        .iter()
        .any(|family| *family != QuestionFamily::Recall)
        && knowledge.units.iter().any(super::annotations::available)
}

pub(crate) fn generation_schema(
    mut legacy: Value,
    policy: &GenerationPolicy,
    knowledge: &KnowledgeCatalog,
) -> Value {
    let mut alternatives = Vec::new();
    if policy.allowed_types.contains(&QuestionFamily::Recall) {
        let mut recall = legacy["properties"]["cards"]["items"].take();
        if let Some(variants) = recall["oneOf"].as_array_mut() {
            variants.retain(|value| value["properties"]["type"]["const"] != "multiple_choice");
        }
        alternatives.push(json!({"type":"object", "additionalProperties":false,
            "required":["card","knowledgeUnitIds"], "properties":{
                "card":recall, "knowledgeUnitIds":unit_ids(knowledge, 2)}}));
    }
    let mut questions = Vec::new();
    for kind in &policy.allowed_types {
        if let Some(schema) = question_schema(*kind, policy.max_compound_items) {
            questions.push(schema);
        }
    }
    let graded_aliases = knowledge
        .units
        .iter()
        .enumerate()
        .filter(|(_, unit)| super::annotations::available(unit))
        .map(|(index, _)| KnowledgeCatalog::unit_alias(index))
        .collect::<Vec<_>>();
    let mut graded_ids = unit_ids(knowledge, 2);
    if graded_aliases.len() != knowledge.units.len() {
        graded_ids["items"]["enum"] = json!(graded_aliases);
    }
    if !questions.is_empty() && !graded_aliases.is_empty() {
        alternatives.push(json!({"type":"object", "additionalProperties":false,
            "required":["card","assessment"], "properties":{
                "card":{"type":"object","additionalProperties":false,
                    "required":["type","question"],"properties":{
                        "type":{"const":"interactive"},"question":{"oneOf":questions},
                        "tags":{"type":"array","maxItems":32,"items":text(64)}}},
                "assessment":{"type":"object","additionalProperties":false,
                    "required":["criterion","ambiguous","components"],"properties":{
                        "criterion":text(500),"ambiguous":{"type":"boolean"},
                        "components":{"type":"array","minItems":1,"maxItems":MAX_COMPONENTS,
                            "items":{"type":"object","additionalProperties":false,
                                "required":["key","stance","knowledgeUnitIds"],"properties":{
                                    "key":text(160),"stance":{"enum":["supports","contradicts"]},
                                    "knowledgeUnitIds":graded_ids}}}}}}}));
    }
    if alternatives.is_empty() {
        // No permitted, supported card family: represent the already-supported
        // empty result rather than an invalid empty oneOf or invented grading.
        legacy["properties"]["cards"] = json!({"type":"array","maxItems":0,
            "items":{"type":"object"}});
    } else {
        legacy["properties"]["cards"]["items"] = json!({"oneOf":alternatives});
    }
    legacy
}

fn question_schema(family: QuestionFamily, maximum: usize) -> Option<Value> {
    use QuestionFamily::*;
    let kind = match family {
        Recall => return None,
        SingleChoice => "single_choice",
        SelectAll => "select_all",
        TrueFalse => "true_false",
        Matching => "matching",
        Grouping => "grouping",
        Ordering => "ordering",
    };
    let mut properties = json!({"kind":{"const":kind}, "explanation":optional_text()});
    let mut required = vec!["kind"];
    if family == TrueFalse {
        properties["statement"] = text(50_000);
        properties["correct"] = json!({"type":"boolean"});
        properties["correctedStatement"] = optional_text();
        required.extend(["statement", "correct", "explanation"]);
    } else {
        properties["prompt"] = text(50_000);
        required.push("prompt");
    }
    match family {
        SingleChoice | SelectAll => {
            properties["options"] = array(
                json!({"type":"object","additionalProperties":false,
                "required":["id","content"],"properties":{
                    "id":local_key(),"content":text(50_000),"rationale":optional_text()}}),
                maximum,
            );
            required.push("options");
            if family == SingleChoice {
                properties["correctOptionId"] = local_key();
                required.push("correctOptionId");
            } else {
                properties["correctOptionIds"] = json!({"type":"array","maxItems":maximum,
                    "uniqueItems":true,"items":local_key()});
                required.push("correctOptionIds");
            }
        }
        Matching => {
            properties["pairs"] = array(
                json!({"type":"object","additionalProperties":false,
                "required":["id","left","right"],"properties":{
                    "id":local_key(),"left":text(50_000),"right":text(50_000),"rationale":optional_text()}}),
                maximum,
            );
            required.push("pairs");
        }
        Grouping => {
            properties["categories"] = array(item(), maximum);
            properties["items"] = array(
                json!({"type":"object","additionalProperties":false,
                "required":["id","content","categoryId"],"properties":{
                    "id":local_key(),"content":text(50_000),"categoryId":local_key(),"rationale":optional_text()}}),
                maximum,
            );
            required.extend(["categories", "items"]);
        }
        Ordering => {
            properties["items"] = array(item(), maximum);
            required.push("items");
        }
        TrueFalse | Recall => {}
    }
    Some(
        json!({"type":"object","additionalProperties":false,"required":required,"properties":properties}),
    )
}

fn text(maximum: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":maximum})
}
fn optional_text() -> Value {
    json!({"type":["string","null"],"maxLength":50_000})
}
fn local_key() -> Value {
    json!({"type":"string","minLength":1,"maxLength":64,"pattern":"^[A-Za-z0-9_-]+$"})
}
fn item() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["id","content"],
    "properties":{"id":local_key(),"content":text(50_000)}})
}
fn array(items: Value, maximum: usize) -> Value {
    json!({"type":"array","minItems":2,"maxItems":maximum,"items":items})
}
fn unit_ids(knowledge: &KnowledgeCatalog, maximum: usize) -> Value {
    let mut schema = knowledge.support_schema(maximum.min(MAX_ASSESSMENT_UNITS));
    schema["uniqueItems"] = json!(true);
    schema
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::typed_payload::TypedPayload;
    use serde_json::json;

    #[test]
    fn document_generation_policy_capability_is_decided_before_provider_dispatch() {
        let mut catalog = crate::knowledge::authoring::tests::catalog_fixture();

        let mut interactive_only = GenerationPolicy::default();
        interactive_only.allowed_types = vec![QuestionFamily::SingleChoice];
        interactive_only.validate().unwrap();
        assert!(has_supported_generation_family(&interactive_only, &catalog));

        let statement = catalog.units[0].content.statement.clone();
        let kind = catalog.units[0].content.kind;
        let deferred = TypedPayload::admit_or_defer(
            json!({"unsupported": true}),
            &statement,
            kind,
        )
        .unwrap();
        assert!(deferred.deferred_rule().is_some());
        catalog.units[0].content.details = Some(deferred);

        assert!(!has_supported_generation_family(
            &interactive_only,
            &catalog
        ));

        let mut recall_only = GenerationPolicy::default();
        recall_only.allowed_types = vec![QuestionFamily::Recall];
        recall_only.validate().unwrap();
        assert!(has_supported_generation_family(&recall_only, &catalog));
    }
}
