use serde_json::{json, Value};

use super::{policy::{GenerationPolicy, QuestionFamily}, MAX_ASSESSMENT_UNITS, MAX_COMPONENTS};

pub(crate) fn generation_schema(mut legacy: Value, policy: &GenerationPolicy) -> Value {
    let mut alternatives = Vec::new();
    if policy.allowed_types.contains(&QuestionFamily::Recall) {
        let mut recall = legacy["properties"]["cards"]["items"].take();
        if let Some(variants) = recall["oneOf"].as_array_mut() {
            variants.retain(|value| value["properties"]["type"]["const"] != "multiple_choice");
        }
        alternatives.push(json!({"type":"object", "additionalProperties":false,
            "required":["card","knowledgeUnitIds"], "properties":{
                "card":recall, "knowledgeUnitIds":unit_ids(2)}}));
    }
    let mut questions = Vec::new();
    for kind in &policy.allowed_types {
        if let Some(schema) = question_schema(*kind, policy.max_compound_items) { questions.push(schema); }
    }
    if !questions.is_empty() {
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
                                    "knowledgeUnitIds":unit_ids(2)}}}}}}}));
    }
    legacy["properties"]["cards"]["items"] = json!({"oneOf":alternatives});
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
            properties["options"] = array(json!({"type":"object","additionalProperties":false,
                "required":["id","content"],"properties":{
                    "id":local_key(),"content":text(50_000),"rationale":optional_text()}}), maximum);
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
            properties["pairs"] = array(json!({"type":"object","additionalProperties":false,
                "required":["id","left","right"],"properties":{
                    "id":local_key(),"left":text(50_000),"right":text(50_000),"rationale":optional_text()}}), maximum);
            required.push("pairs");
        }
        Grouping => {
            properties["categories"] = array(item(), maximum);
            properties["items"] = array(json!({"type":"object","additionalProperties":false,
                "required":["id","content","categoryId"],"properties":{
                    "id":local_key(),"content":text(50_000),"categoryId":local_key(),"rationale":optional_text()}}), maximum);
            required.extend(["categories", "items"]);
        }
        Ordering => { properties["items"] = array(item(), maximum); required.push("items"); }
        TrueFalse | Recall => {}
    }
    Some(json!({"type":"object","additionalProperties":false,"required":required,"properties":properties}))
}

fn text(maximum: usize) -> Value { json!({"type":"string","minLength":1,"maxLength":maximum}) }
fn optional_text() -> Value { json!({"type":["string","null"],"maxLength":50_000}) }
fn local_key() -> Value { json!({"type":"string","minLength":1,"maxLength":64,"pattern":"^[A-Za-z0-9_-]+$"}) }
fn item() -> Value { json!({"type":"object","additionalProperties":false,"required":["id","content"],
    "properties":{"id":local_key(),"content":text(50_000)}}) }
fn array(items: Value, maximum: usize) -> Value { json!({"type":"array","minItems":2,"maxItems":maximum,"items":items}) }
fn unit_ids(maximum: usize) -> Value { json!({"type":"array","minItems":1,"maxItems":maximum.min(MAX_ASSESSMENT_UNITS),
    "uniqueItems":true,"items":{"type":"string","minLength":32,"maxLength":32}}) }
