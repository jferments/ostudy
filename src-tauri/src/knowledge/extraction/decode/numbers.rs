//! A number the model states must be one the cited sources print. Values read
//! from a page render (a table cell, a dose) are checked against the page's
//! own text layer, so a misread render cannot become an authoritative unit.
//! Single digits are left alone: they are often written as words ("four
//! chambers") and carry little risk of invented precision.

use std::collections::HashSet;

use super::super::super::contracts::{KnowledgeUnitInput, SourceRecord};

/// Text placeholders that mean a source has no text layer to check against.
const NO_TEXT_MARKERS: &[&str] = &[
    "has no extractable structured text",
    "contains embedded visual material with no extractable text",
];

/// Digit runs with internal separators ("1,200", "0.75"), commas dropped.
fn numbers(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let mut number = String::new();
        while index < chars.len() {
            let ch = chars[index];
            let joins_digits = matches!(ch, '.' | ',')
                && chars.get(index + 1).is_some_and(char::is_ascii_digit)
                && !number.is_empty();
            if ch.is_ascii_digit() || joins_digits {
                if ch != ',' {
                    number.push(ch);
                }
                index += 1;
            } else {
                break;
            }
        }
        found.push(number);
    }
    found
}

fn checked(number: &str) -> bool {
    number.chars().filter(char::is_ascii_digit).count() >= 2 || number.contains('.')
}

/// The first stated number that none of the cited sources prints, if any.
/// Sources without a text layer cannot vouch for or against a number, so a
/// unit cited only on them is not checked.
pub(super) fn unsupported_number(
    content: &KnowledgeUnitInput,
    cited: &[&SourceRecord],
) -> Option<String> {
    let texts = cited
        .iter()
        .filter(|source| {
            !source.text.trim().is_empty()
                && !NO_TEXT_MARKERS
                    .iter()
                    .any(|marker| source.text.contains(marker))
        })
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    if texts.is_empty() {
        return None;
    }
    let printed = texts
        .iter()
        .flat_map(|text| numbers(text))
        .collect::<HashSet<_>>();
    std::iter::once(content.statement.as_str())
        .chain(content.qualifications.iter().map(String::as_str))
        .flat_map(numbers)
        .filter(|number| checked(number))
        .find(|number| !printed.contains(number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::contracts::KnowledgeKind;

    fn unit(statement: &str, qualifications: &[&str]) -> KnowledgeUnitInput {
        KnowledgeUnitInput {
            title: "Vitamin C".into(),
            kind: KnowledgeKind::Fact,
            statement: statement.into(),
            qualifications: qualifications.iter().map(|q| q.to_string()).collect(),
            topic: "Vitamins".into(),
            supporting_excerpt: String::new(),
            grounding: None,
            details: None,
        }
    }

    fn source(text: &str) -> SourceRecord {
        let (_, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
        source.text = text.into();
        source
    }

    #[test]
    fn numbers_keep_decimals_and_drop_thousands_separators() {
        assert_eq!(
            numbers("Adults need 75–90 mg, up to 2,000 mg; ratio 0.75. Section 19.2."),
            vec!["75", "90", "2000", "0.75", "19.2"]
        );
    }

    #[test]
    fn a_number_the_page_does_not_print_is_unsupported() {
        let page = source("Vitamin C | 75-90 mg | Collagen synthesis\nUpper limit 2,000 mg");
        assert_eq!(
            unsupported_number(
                &unit(
                    "Adults need 75 to 90 mg of vitamin C.",
                    &["Upper limit 2000 mg."]
                ),
                &[&page]
            ),
            None
        );
        assert_eq!(
            unsupported_number(
                &unit("Adults need 65 to 90 mg of vitamin C.", &[]),
                &[&page]
            ),
            Some("65".into())
        );
        assert_eq!(
            unsupported_number(&unit("The limit is 2.5 g.", &[]), &[&page]),
            Some("2.5".into())
        );
    }

    #[test]
    fn single_digits_and_pages_without_text_are_not_checked() {
        let page = source("The heart has four chambers.");
        assert_eq!(
            unsupported_number(&unit("The heart has 4 chambers.", &[]), &[&page]),
            None
        );
        let figure = source(
            "Page 3 contains embedded visual material with no extractable text. Use the attached source assets as evidence.",
        );
        assert_eq!(
            unsupported_number(&unit("Output is 5,000 mL per minute.", &[]), &[&figure]),
            None
        );
    }
}
