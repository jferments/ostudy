//! Bounded material selections. Legacy single-source scope JSON is unchanged.
//! Selection membership is authority, not a retrieval hint. No nested selections
//! or wildcard members are accepted; canonical ordering stabilizes permissions.
use super::contracts::invalid;
use crate::{error::CommandError, util::validate_entity_id};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    General,
    Deck,
    Document,
    Guide,
    Collection,
    Library,
    Selection,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    Deck,
    Document,
    Guide,
    Collection,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialScope {
    pub kind: MaterialKind,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", try_from = "ScopeWire")]
pub struct StudyScope {
    pub kind: ScopeKind,
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<MaterialScope>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScopeWire {
    kind: ScopeKind,
    id: Option<String>,
    #[serde(default)]
    members: Vec<MaterialScope>,
}
impl TryFrom<ScopeWire> for StudyScope {
    type Error = String;
    fn try_from(wire: ScopeWire) -> Result<Self, Self::Error> {
        let mut scope = Self {
            kind: wire.kind,
            id: wire.id,
            members: wire.members,
        };
        if scope.members.len() > 16 {
            return Err("Select at most 16 study sources.".into());
        }
        scope.members.sort();
        scope.validate().map_err(|error| error.message)?;
        // A one-member selection has the same consent identity as that member.
        if scope.kind == ScopeKind::Selection && scope.members.len() == 1 {
            return Ok(scope.material_scopes().remove(0));
        }
        Ok(scope)
    }
}
impl StudyScope {
    pub(crate) fn single(kind: ScopeKind, id: Option<String>) -> Self {
        Self {
            kind,
            id,
            members: Vec::new(),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.kind == ScopeKind::Selection {
            if self.id.is_some() || self.members.is_empty() || self.members.len() > 16 {
                return Err(invalid(
                    "Select between one and sixteen explicit study sources.",
                ));
            }
            if self.members.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid(
                    "Study source selections must be distinct and canonically ordered.",
                ));
            }
            for member in &self.members {
                validate_entity_id(&member.id, "selected study source")?;
            }
            return Ok(());
        }
        if !self.members.is_empty() {
            return Err(invalid("Only a selection scope may contain members."));
        }
        match self.kind {
            ScopeKind::General | ScopeKind::Library if self.id.is_none() => Ok(()),
            ScopeKind::Deck | ScopeKind::Document | ScopeKind::Guide | ScopeKind::Collection => {
                validate_entity_id(
                    self.id
                        .as_deref()
                        .ok_or_else(|| invalid("Select study material."))?,
                    "study scope",
                )
            }
            _ => Err(invalid("This study scope must not contain an entity ID.")),
        }
    }
    /// Returns only leaf scopes. The wire representation cannot express nesting.
    pub(crate) fn material_scopes(&self) -> Vec<Self> {
        if self.kind != ScopeKind::Selection {
            return vec![self.clone()];
        }
        self.members
            .iter()
            .map(|member| {
                Self::single(
                    match member.kind {
                        MaterialKind::Deck => ScopeKind::Deck,
                        MaterialKind::Document => ScopeKind::Document,
                        MaterialKind::Guide => ScopeKind::Guide,
                        MaterialKind::Collection => ScopeKind::Collection,
                    },
                    Some(member.id.clone()),
                )
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn selection_is_order_independent_without_broadening_legacy_scopes() {
        let a = json!({"kind":"document","id":"a".repeat(32)});
        let b = json!({"kind":"deck","id":"b".repeat(32)});
        let left: StudyScope =
            serde_json::from_value(json!({"kind":"selection","id":null,"members":[a,b]})).unwrap();
        let right: StudyScope =
            serde_json::from_value(json!({"kind":"selection","members":[b,a]})).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            serde_json::to_string(&left).unwrap(),
            serde_json::to_string(&right).unwrap()
        );
        let old: StudyScope = serde_json::from_value(a.clone()).unwrap();
        assert_eq!(serde_json::to_value(&old).unwrap(), a);
        let single: StudyScope =
            serde_json::from_value(json!({"kind":"selection","members":[a]})).unwrap();
        assert_eq!(single, old);
    }
    #[test]
    fn forged_ambiguous_or_unbounded_scopes_are_rejected() {
        let a = json!({"kind":"document","id":"a".repeat(32)});
        for value in [
            json!({"kind":"selection","members":[]}),
            json!({"kind":"selection","members":[a,a]}),
            json!({"kind":"selection","members":[{"kind":"library","id":null}]}),
            json!({"kind":"selection","members":[{"kind":"selection","members":[a]}]}),
            json!({"kind":"selection","id":"a".repeat(32),"members":[a]}),
            json!({"kind":"general","members":[a]}),
            json!({"kind":"selection","members":vec![a;17]}),
        ] {
            assert!(serde_json::from_value::<StudyScope>(value).is_err());
        }
    }
}
