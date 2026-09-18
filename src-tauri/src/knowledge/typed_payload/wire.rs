use super::*;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    text: String,
    occurrence: usize,
}

impl Selection {
    pub(super) fn resolve(self, statement: &str) -> Result<StatementSpan, CommandError> {
        if self.text.trim().is_empty()
            || self.text.len() > statement.len()
            || self.occurrence > 64
        {
            return Err(invalid("Invalid qualified phrase selection."));
        }
        let (start, text) = statement
            .match_indices(self.text.as_str())
            .nth(self.occurrence)
            .ok_or_else(|| invalid("The selected qualified phrase is absent from the statement."))?;
        Ok(StatementSpan {
            start,
            end: start + text.len(),
        })
    }
}

pub(super) fn schema() -> Value {
    let select = json!({"type":"object","additionalProperties":false,
        "required":["text","occurrence"],"properties":{
            "text":{"type":"string","minLength":1,"maxLength":3000},
            "occurrence":{"type":"integer","minimum":0,"maximum":64}}});
    let pair = json!({"type":"object","additionalProperties":false,
        "required":["label","value"],"properties":{"label":select,"value":select}});
    let category = json!({"type":"object","additionalProperties":false,
        "required":["label","members"],"properties":{"label":select,
            "members":{"type":"array","minItems":1,"maxItems":32,"items":select}}});
    json!({"description":"Optional qualified structure. Select literal phrases from your returned statement using text and zero-based occurrence, never source offsets. Use comparison only with comparison units, process only with process units, quantity only with fact units, and classification with definition/relationship units. Preserve units, operators, conditions, branches and overlapping memberships. Process dependencies are only explicitly required preceding steps, not list order. Use null when the evidence does not support this structure. Native code resolves every phrase and retains qualifications separately.","anyOf":[
        {"type":"null"},
        {"type":"object","additionalProperties":false,"required":["kind","criterion","items"],
            "properties":{"kind":{"const":"comparison"},"criterion":select,
                "items":{"type":"array","minItems":2,"maxItems":16,"items":pair}}},
        {"type":"object","additionalProperties":false,"required":["kind","steps","dependencies"],
            "properties":{"kind":{"const":"process"},
                "steps":{"type":"array","minItems":2,"maxItems":32,"items":select},
                "dependencies":{"type":"array","maxItems":128,"items":{
                    "type":"object","additionalProperties":false,"required":["before","after"],
                    "properties":{"before":{"type":"integer","minimum":0,"maximum":31},
                        "after":{"type":"integer","minimum":0,"maximum":31}}}}}},
        {"type":"object","additionalProperties":false,"required":["kind","subject","bounds","unit"],
            "properties":{"kind":{"const":"quantity"},"subject":select,
                "bounds":{"type":"array","minItems":1,"maxItems":2,"items":pair},
                "unit":{"anyOf":[select,{"type":"null"}]}}},
        {"type":"object","additionalProperties":false,"required":["kind","criterion","categories","exclusion"],
            "properties":{"kind":{"const":"classification"},"criterion":select,
                "categories":{"type":"array","minItems":2,"maxItems":16,"items":category},
                "exclusion":{"anyOf":[select,{"type":"null"}]}}}
    ]})
}
