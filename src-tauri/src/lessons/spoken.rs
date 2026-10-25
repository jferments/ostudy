//! Formulas and ions as a teacher says them aloud.
//!
//! Lesson text prints formulas with sub- and superscripts (HCO₃⁻, Ca²⁺) and a
//! model may still write some flat (HCO3-, Ca2+); a voice reads either as
//! letters and marks, or skips the marks. Before text is spoken, each word the
//! chemistry rules read as a formula is said by its common name (bicarbonate,
//! calcium, carbon dioxide) or, without one, symbol by symbol with its counts
//! and charge (P C O two). Words the rules leave alone, such as B12 or an H2
//! receptor, are spoken as written.

use crate::document_study_guide_export::chemistry::{self, Formula};

/// Formulas with a name a teacher would say, keyed by their flat spelling.
const NAMES: &[(&str, &str)] = &[
    ("H+", "hydrogen ion"),
    ("Na+", "sodium"),
    ("K+", "potassium"),
    ("Li+", "lithium"),
    ("Ca2+", "calcium"),
    ("Mg2+", "magnesium"),
    ("Zn2+", "zinc"),
    ("Fe2+", "ferrous iron"),
    ("Fe3+", "ferric iron"),
    ("Cl-", "chloride"),
    ("F-", "fluoride"),
    ("Br-", "bromide"),
    ("I-", "iodide"),
    ("H2", "hydrogen"),
    ("N2", "nitrogen"),
    ("O2", "oxygen"),
    ("Cl2", "chlorine"),
    ("H2O", "water"),
    ("H2O2", "hydrogen peroxide"),
    ("H3O+", "hydronium"),
    ("OH-", "hydroxide"),
    ("CO2", "carbon dioxide"),
    ("H2CO3", "carbonic acid"),
    ("HCO3-", "bicarbonate"),
    ("CO32-", "carbonate"),
    ("NaHCO3", "sodium bicarbonate"),
    ("CaCO3", "calcium carbonate"),
    ("NH3", "ammonia"),
    ("NH4+", "ammonium"),
    ("NO3-", "nitrate"),
    ("NO2-", "nitrite"),
    ("PO43-", "phosphate"),
    ("HPO42-", "hydrogen phosphate"),
    ("H2PO4-", "dihydrogen phosphate"),
    ("SO42-", "sulfate"),
    ("H2SO4", "sulfuric acid"),
    ("CH4", "methane"),
    ("C6H12O6", "glucose"),
];

const NUMBERS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

/// `text` with every formula in it written as it is said.
pub(crate) fn spoken(text: &str) -> String {
    // Printed marks are read as the flat spelling the rules know, keeping
    // where each flat byte came from so the rest of the text is unchanged.
    let mut flat = String::with_capacity(text.len());
    let mut origin = Vec::with_capacity(text.len() + 1);
    for (index, character) in text.char_indices() {
        let before = flat.len();
        flat.push(unmarked(character).unwrap_or(character));
        origin.resize(flat.len(), index);
        debug_assert!(origin.len() > before);
    }
    origin.push(text.len());

    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end, formula) in chemistry::formulas(&flat) {
        let (start, end) = (origin[start], origin[end]);
        out.push_str(&text[from..start]);
        out.push_str(&say(&formula, &text[end..]));
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

fn unmarked(character: char) -> Option<char> {
    Some(match character {
        '⁰' | '₀' => '0',
        '¹' | '₁' => '1',
        '²' | '₂' => '2',
        '³' | '₃' => '3',
        '⁴' | '₄' => '4',
        '⁵' | '₅' => '5',
        '⁶' | '₆' => '6',
        '⁷' | '₇' => '7',
        '⁸' | '₈' => '8',
        '⁹' | '₉' => '9',
        '⁺' | '₊' => '+',
        '⁻' | '₋' => '−',
        _ => return None,
    })
}

/// How `formula` is said before `following`.
fn say(formula: &Formula<'_>, following: &str) -> String {
    let mut key = formula
        .atoms
        .iter()
        .map(|atom| format!("{}{}", atom.symbol, atom.count))
        .collect::<String>();
    if let Some((magnitude, sign)) = formula.charge {
        key.push_str(magnitude);
        key.push(if sign == '+' { '+' } else { '-' });
    }
    if let Some((_, name)) = NAMES.iter().find(|(flat, _)| *flat == key) {
        // "H⁺ ions" and "the H⁺-K⁺ ATPase" are "hydrogen ions" and the
        // "hydrogen-potassium ATPase".
        let next = following.trim_start().to_lowercase();
        let joined = following.starts_with(['-', '–', '−'])
            || next.starts_with("ion ")
            || next.starts_with("ions")
            || next == "ion"
            || next.starts_with("ion.")
            || next.starts_with("ion,");
        return match name.strip_suffix(" ion") {
            Some(short) if joined => short.to_string(),
            _ => (*name).to_string(),
        };
    }
    let mut words = Vec::new();
    for atom in &formula.atoms {
        words.extend(
            atom.symbol
                .chars()
                .map(|letter| letter.to_ascii_uppercase().to_string()),
        );
        if !atom.count.is_empty() {
            words.push(number(atom.count));
        }
    }
    if let Some((magnitude, sign)) = formula.charge {
        if !magnitude.is_empty() {
            words.push(number(magnitude));
        }
        words.push(if sign == '+' { "plus" } else { "minus" }.to_string());
    }
    words.join(" ")
}

fn number(digits: &str) -> String {
    digits
        .parse::<usize>()
        .ok()
        .and_then(|value| NUMBERS.get(value))
        .map_or_else(|| digits.to_string(), |word| (*word).to_string())
}

#[cfg(test)]
mod tests {
    use super::spoken;

    #[test]
    fn says_printed_and_flat_formulas_by_name() {
        assert_eq!(
            spoken("Parietal cells secrete H⁺ and return HCO₃⁻ as CO₂ and H₂O form H₂CO₃."),
            "Parietal cells secrete hydrogen ion and return bicarbonate as carbon dioxide and water form carbonic acid."
        );
        assert_eq!(
            spoken("Ca2+ and Na+ enter; H+ ions leave through the H⁺-K⁺ ATPase."),
            "calcium and sodium enter; hydrogen ions leave through the hydrogen-potassium ATPase."
        );
        assert_eq!(
            spoken("The Cl⁻-HCO₃⁻ exchanger and SO₄²⁻."),
            "The chloride-bicarbonate exchanger and sulfate."
        );
    }

    #[test]
    fn spells_formulas_without_a_name() {
        assert_eq!(spoken("PCO₂ rises."), "P C O two rises.");
        assert_eq!(spoken("Cu2+ binds."), "C U two plus binds.");
        assert_eq!(spoken("MgSO4 is given."), "M G S O four is given.");
    }

    #[test]
    fn leaves_other_words_as_written() {
        for text in [
            "H2 receptor blockers and vitamin B12.",
            "An area of 4 m² in 2021.",
            "IL-2, CD4 and the S1 sound.",
            "ATP and NaCl.",
        ] {
            assert_eq!(spoken(text), text);
        }
    }
}
