//! Qualified structure annotates one authoritative statement, not another copy
//! of its values. Native code assigns UTF-8 offsets from literal selections.
//! Location and shape validation do not establish semantic entailment.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::contracts::{bounded, invalid, KnowledgeKind, MAX_UNIT_STATEMENT_CHARS};
use crate::error::CommandError;

mod wire;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TypedPayload {
    pub version: u8,
    pub shape: Shape<StatementSpan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatementSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Shape<T> {
    Comparison {
        criterion: T,
        items: Vec<Pair<T>>,
    },
    Process {
        steps: Vec<T>,
        dependencies: Vec<Dependency>,
    },
    Quantity {
        subject: T,
        bounds: Vec<Pair<T>>,
        unit: Option<T>,
    },
    Classification {
        criterion: T,
        categories: Vec<Category<T>>,
        exclusion: Option<T>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pair<T> {
    pub label: T,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Category<T> {
    pub label: T,
    pub members: Vec<T>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dependency {
    pub before: usize,
    pub after: usize,
}

impl<T> Shape<T> {
    fn check_bounds(&self) -> Result<(), CommandError> {
        let valid = match self {
            Self::Comparison { items, .. } => (2..=16).contains(&items.len()),
            Self::Process {
                steps,
                dependencies,
            } => (2..=32).contains(&steps.len()) && dependencies.len() <= 128,
            Self::Quantity { bounds, .. } => (1..=2).contains(&bounds.len()),
            Self::Classification { categories, .. } => {
                (2..=16).contains(&categories.len())
                    && categories.iter().all(|c| (1..=32).contains(&c.members.len()))
            }
        };
        if !valid {
            return Err(invalid("Qualified knowledge exceeds its structural bounds."));
        }
        Ok(())
    }

    fn map<U>(
        self,
        mut f: impl FnMut(T) -> Result<U, CommandError>,
    ) -> Result<Shape<U>, CommandError> {
        self.check_bounds()?;
        Ok(match self {
            Self::Comparison { criterion, items } => Shape::Comparison {
                criterion: f(criterion)?,
                items: items
                    .into_iter()
                    .map(|p| {
                        Ok(Pair {
                            label: f(p.label)?,
                            value: f(p.value)?,
                        })
                    })
                    .collect::<Result<_, CommandError>>()?,
            },
            Self::Process {
                steps,
                dependencies,
            } => Shape::Process {
                steps: steps.into_iter().map(&mut f).collect::<Result<_, _>>()?,
                dependencies,
            },
            Self::Quantity {
                subject,
                bounds,
                unit,
            } => Shape::Quantity {
                subject: f(subject)?,
                bounds: bounds
                    .into_iter()
                    .map(|p| {
                        Ok(Pair {
                            label: f(p.label)?,
                            value: f(p.value)?,
                        })
                    })
                    .collect::<Result<_, CommandError>>()?,
                unit: unit.map(&mut f).transpose()?,
            },
            Self::Classification {
                criterion,
                categories,
                exclusion,
            } => Shape::Classification {
                criterion: f(criterion)?,
                categories: categories
                    .into_iter()
                    .map(|c| {
                        Ok(Category {
                            label: f(c.label)?,
                            members: c.members.into_iter().map(&mut f).collect::<Result<_, _>>()?,
                        })
                    })
                    .collect::<Result<_, CommandError>>()?,
                exclusion: exclusion.map(&mut f).transpose()?,
            },
        })
    }
}

impl StatementSpan {
    fn text<'a>(&self, statement: &'a str) -> Result<&'a str, CommandError> {
        if self.start >= self.end {
            return Err(invalid("A qualified statement span is empty."));
        }
        statement
            .get(self.start..self.end)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| invalid("A qualified statement span is not a UTF-8 boundary."))
    }
}

impl TypedPayload {
    pub(crate) fn admit(
        value: Value,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<Self, CommandError> {
        bounded(
            statement,
            MAX_UNIT_STATEMENT_CHARS,
            "knowledge statement",
            false,
        )?;
        let shape: Shape<wire::Selection> = serde_json::from_value(value)
            .map_err(|_| invalid("Malformed qualified knowledge structure."))?;
        let payload = Self {
            version: 1,
            shape: shape.map(|item| item.resolve(statement))?,
        };
        payload.validate(statement, kind)?;
        Ok(payload)
    }

    pub(crate) fn validate(
        &self,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<(), CommandError> {
        if self.version != 1 {
            return Err(invalid("Unsupported qualified knowledge version."));
        }
        self.shape.check_bounds()?;
        if let Shape::Quantity { subject, bounds, .. } = &self.shape {
            for bound in bounds {
                bound_direction(subject, bound)?;
                token_boundary(statement, &bound.label, |ch| "<>=≤≥≠".contains(ch))?;
                token_boundary(statement, &bound.value, |ch| {
                    ch.is_ascii_digit() || "+-−.eE".contains(ch)
                })?;
            }
        }
        // Bound before cloning, and resolve every reference, not just the fields
        // used by a particular consumer. The statement remains the sole value owner.
        let shape = self
            .shape
            .clone()
            .map(|span| Ok(span.text(statement)?.to_string()))?;
        let compatible = match shape {
            Shape::Comparison { .. } => kind == KnowledgeKind::Comparison,
            Shape::Process {
                steps,
                dependencies,
            } => {
                validate_dependencies(steps.len(), &dependencies)?;
                kind == KnowledgeKind::Process
            }
            Shape::Quantity { bounds, .. } => {
                for bound in bounds {
                    if !matches!(
                        bound.label.as_str(),
                        "=" | "<" | ">" | "<=" | ">=" | "≤" | "≥" | "≠"
                    ) || !bound
                        .value
                        .replace('−', "-")
                        .parse::<f64>()
                        .is_ok_and(f64::is_finite)
                    {
                        return Err(invalid(
                            "A quantity requires explicit operators and numeric literals.",
                        ));
                    }
                }
                kind == KnowledgeKind::Fact
            }
            Shape::Classification { .. } => {
                matches!(kind, KnowledgeKind::Definition | KnowledgeKind::Relationship)
            }
        };
        if !compatible {
            return Err(invalid("Qualified structure does not match the knowledge kind."));
        }
        Ok(())
    }

    pub(crate) fn prompt(&self, statement: &str) -> Result<Value, CommandError> {
        self.shape.check_bounds()?;
        let resolved = self
            .shape
            .clone()
            .map(|span| Ok(span.text(statement)?.to_string()))?;
        let mut value = serde_json::to_value(resolved).map_err(CommandError::serialization)?;
        if let Shape::Quantity { subject, bounds, .. } = &self.shape {
            for (index, bound) in bounds.iter().enumerate() {
                value["bounds"][index]["subjectOnLeft"] = Value::Bool(bound_direction(subject, bound)?);
            }
        }
        Ok(value)
    }

    pub(crate) fn schema() -> Value {
        wire::schema()
    }
}

// Avoid turning a strict/inclusive operator or a signed number into a different
// token through a substring selection. This is lexical integrity, not entailment.
fn token_boundary(
    statement: &str,
    span: &StatementSpan,
    belongs: impl Fn(char) -> bool,
) -> Result<(), CommandError> {
    span.text(statement)?;
    if statement[..span.start].chars().next_back().is_some_and(&belongs)
        || statement[span.end..].chars().next().is_some_and(belongs)
    {
        return Err(invalid("A qualified selection cuts a numeric or comparison token."));
    }
    Ok(())
}

fn bound_direction(subject: &StatementSpan, bound: &Pair<StatementSpan>) -> Result<bool, CommandError> {
    if subject.end <= bound.label.start && bound.label.end <= bound.value.start {
        Ok(true)
    } else if bound.value.end <= bound.label.start && bound.label.end <= subject.start {
        Ok(false)
    } else {
        Err(invalid("A quantity's operator must remain between its subject and value."))
    }
}

fn validate_dependencies(steps: usize, dependencies: &[Dependency]) -> Result<(), CommandError> {
    use std::collections::BTreeSet;
    let mut edges = BTreeSet::new();
    for edge in dependencies {
        if edge.before >= steps
            || edge.after >= steps
            || edge.before == edge.after
            || !edges.insert((edge.before, edge.after))
        {
            return Err(invalid("Invalid or duplicate process dependency."));
        }
    }
    let mut removed = BTreeSet::new();
    loop {
        let ready = (0..steps).find(|step| {
            !removed.contains(step)
                && edges
                    .iter()
                    .all(|(before, after)| after != step || removed.contains(before))
        });
        match ready {
            Some(step) => {
                removed.insert(step);
            }
            None => break,
        }
    }
    if removed.len() != steps {
        return Err(invalid("Process dependencies contain a cycle."));
    }
    // No edges means no asserted order. Never invent order from list position.
    Ok(())
}

#[cfg(test)]
mod tests;
