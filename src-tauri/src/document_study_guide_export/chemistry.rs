//! Chemical formulas and ions as a guide prints them: HCO₃⁻, Ca²⁺, H₂O.
//!
//! Text extracted from a PDF, and a model writing from it, often spells them
//! flat (HCO3−, Ca2+, H2O). A word is set with sub- and superscripts only when
//! it reads as a formula built of element symbols: two or more elements with
//! a count (CO2, H2O, PCO2), a charge (Na+, Cl−, HCO3−, SO42−), or one of the
//! diatomic gases (O2, N2, H2, Cl2). Anything else, such as B12, S1, C3 or an
//! H2 receptor, is left as written.

/// A piece of text: plain, a subscript, a superscript or an organism's
/// italic name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Text(String),
    Sub(String),
    Sup(String),
    Italic(String),
}

const ELEMENTS: &[&str] = &[
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pt", "Au", "Hg", "Pb", "Bi", "Ra", "U",
];
const DIATOMIC: &[&str] = &["H", "N", "O", "F", "Cl", "Br", "I"];
/// Words after which an H2 or similar is a receptor's name, not a gas.
const NAME_FOLLOWERS: &[&str] = &[
    "receptor",
    "receptors",
    "blocker",
    "blockers",
    "antagonist",
    "antagonists",
    "agonist",
    "agonists",
    "antihistamine",
    "antihistamines",
];

fn is_sign(c: char) -> bool {
    matches!(c, '+' | '-' | '−' | '–')
}

/// Splits `text` into plain pieces, the sub- and superscripts of the
/// formulas in it and the italic names of organisms (H. pylori, E. coli).
pub(crate) fn pieces(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut from = 0;
    for (start, end) in species(text) {
        out.extend(formula_pieces(&text[from..start]));
        out.push(Piece::Italic(text[start..end].to_string()));
        from = end;
    }
    out.extend(formula_pieces(&text[from..]));
    out
}

/// Where binomial names abbreviated to their genus initial sit: a capital and
/// a full stop, a space, then a lowercase species name of four letters or more.
fn species(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index + 4 < bytes.len() {
        let boundary = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if boundary
            && bytes[index].is_ascii_uppercase()
            && bytes[index + 1] == b'.'
            && bytes[index + 2] == b' '
        {
            let mut end = index + 3;
            while end < bytes.len() && bytes[end].is_ascii_lowercase() {
                end += 1;
            }
            let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
            if end - (index + 3) >= 4 && after_ok {
                found.push((index, end));
                index = end;
                continue;
            }
        }
        index += 1;
    }
    found
}

fn formula_pieces(text: &str) -> Vec<Piece> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut out = Vec::<Piece>::new();
    let mut plain_from = 0_usize;
    let mut index = 0_usize;
    while index < chars.len() {
        let (start, c) = chars[index];
        let boundary = index == 0 || !is_word_char(chars[index - 1].1);
        if boundary && c.is_ascii_uppercase() {
            if let Some((formula, end)) = formula_at(text, start) {
                if !followed_by_name(&text[end..]) {
                    push_text(&mut out, &text[plain_from..start]);
                    for piece in formula {
                        match piece {
                            Piece::Text(value) => push_text(&mut out, &value),
                            other => out.push(other),
                        }
                    }
                    plain_from = end;
                    while index < chars.len() && chars[index].0 < end {
                        index += 1;
                    }
                    continue;
                }
            }
        }
        index += 1;
    }
    push_text(&mut out, &text[plain_from..]);
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn push_text(out: &mut Vec<Piece>, value: &str) {
    if value.is_empty() {
        return;
    }
    if let Some(Piece::Text(last)) = out.last_mut() {
        last.push_str(value);
    } else {
        out.push(Piece::Text(value.to_string()));
    }
}

fn followed_by_name(rest: &str) -> bool {
    let next = rest
        .trim_start_matches([' ', '\u{a0}', '-'])
        .split(|c: char| !c.is_alphabetic())
        .next()
        .unwrap_or("")
        .to_lowercase();
    NAME_FOLLOWERS.contains(&next.as_str())
}

/// One element symbol and its count, as written.
struct Atom<'a> {
    symbol: &'a str,
    count: &'a str,
}

/// The formula starting at byte `start`, as pieces, and the byte where it
/// ends; `None` when the word there is not a formula.
fn formula_at(text: &str, start: usize) -> Option<(Vec<Piece>, usize)> {
    let bytes = text.as_bytes();
    let mut position = start;
    let mut atoms = Vec::<Atom<'_>>::new();
    while position < bytes.len() && bytes[position].is_ascii_uppercase() {
        let two = text
            .get(position..position + 2)
            .filter(|pair| pair.as_bytes()[1].is_ascii_lowercase() && ELEMENTS.contains(pair));
        let symbol = match two {
            Some(pair) => pair,
            None => {
                let one = &text[position..position + 1];
                if !ELEMENTS.contains(&one) {
                    return None;
                }
                one
            }
        };
        position += symbol.len();
        let digits_from = position;
        while position < bytes.len() && bytes[position].is_ascii_digit() {
            position += 1;
        }
        atoms.push(Atom {
            symbol,
            count: &text[digits_from..position],
        });
    }
    if atoms.is_empty() {
        return None;
    }
    // A lowercase letter or digit run on means a word, not a formula (Cat, Hb).
    let rest = &text[position..];
    let mut charge = None::<(usize, char)>;
    if let Some(sign) = rest.chars().next().filter(|c| is_sign(*c)) {
        let after = &rest[sign.len_utf8()..];
        let next = after.chars().next();
        // A dash before a capital or a word joins two names (Cl−-HCO3−,
        // H+-K+ ATPase, B-cell); only a sign that ends the word is a charge.
        let ends_word = next.is_none_or(|c| !c.is_alphanumeric());
        let joins = sign != '+' && next.is_some_and(|c| c.is_alphanumeric());
        if ends_word && !joins {
            charge = Some((sign.len_utf8(), sign));
        }
    } else if rest.chars().next().is_some_and(is_word_char) {
        return None;
    }
    // A charge's magnitude is written before its sign: Ca2+ is Ca²⁺. In a
    // many-atom ion only a second digit is the charge: SO42− is SO₄²⁻, but
    // HCO3− is HCO₃⁻.
    let mut magnitude = "";
    let single = atoms.len() == 1;
    if charge.is_some() {
        if let Some(last) = atoms.last_mut() {
            if single && last.count.len() == 1 {
                magnitude = last.count;
                last.count = "";
            } else if last.count.len() == 2 && matches!(&last.count[1..], "2" | "3") {
                magnitude = &last.count[1..];
                last.count = &last.count[..1];
            }
        }
    }
    let counted = atoms.iter().any(|atom| !atom.count.is_empty());
    let formula = if charge.is_some() {
        true
    } else if atoms.len() >= 2 {
        counted
    } else {
        counted && DIATOMIC.contains(&atoms[0].symbol) && atoms[0].count == "2"
    };
    if !formula {
        return None;
    }
    let mut pieces = Vec::new();
    for atom in &atoms {
        pieces.push(Piece::Text(atom.symbol.to_string()));
        if !atom.count.is_empty() {
            pieces.push(Piece::Sub(atom.count.to_string()));
        }
    }
    let mut end = position;
    if let Some((length, sign)) = charge {
        let sign = if sign == '+' { '+' } else { '−' };
        pieces.push(Piece::Sup(format!("{magnitude}{sign}")));
        end += length;
    }
    Some((pieces, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(text: &str) -> String {
        pieces(text)
            .into_iter()
            .map(|piece| match piece {
                Piece::Text(value) => value,
                Piece::Sub(value) => format!("_{{{value}}}"),
                Piece::Sup(value) => format!("^{{{value}}}"),
                Piece::Italic(value) => format!("*{value}*"),
            })
            .collect()
    }

    #[test]
    fn sets_ions_and_molecules() {
        assert_eq!(show("Na+ and K+"), "Na^{+} and K^{+}");
        assert_eq!(show("HCO3− secretion"), "HCO_{3}^{−} secretion");
        assert_eq!(show("Ca2+ enters"), "Ca^{2+} enters");
        assert_eq!(show("H2O and CO2."), "H_{2}O and CO_{2}.");
        assert_eq!(show("SO42- and NH4+"), "SO_{4}^{2−} and NH_{4}^{+}");
        assert_eq!(show("PCO2 rises"), "PCO_{2} rises");
        assert_eq!(show("O2 delivery"), "O_{2} delivery");
        assert_eq!(show("Cl−-HCO3− exchange"), "Cl^{−}-HCO_{3}^{−} exchange");
        assert_eq!(show("the H+-K+ ATPase"), "the H^{+}-K^{+} ATPase");
        assert_eq!(show("Na+-K+-ATPase"), "Na^{+}-K^{+}-ATPase");
        assert_eq!(show("(HCO3−)"), "(HCO_{3}^{−})");
    }

    #[test]
    fn italicises_organisms() {
        assert_eq!(show("H. pylori and E. coli."), "*H. pylori* and *E. coli*.");
        assert_eq!(show("See A. The end"), "See A. The end");
    }

    #[test]
    fn leaves_words_and_names_alone() {
        for text in [
            "vitamin B12",
            "the S1 heart sound",
            "C3 complement",
            "H2 receptor blockers",
            "B-cells and T-cells",
            "CCK and GIP",
            "NaCl",
            "ATP",
            "Cat",
            "pH 7",
            "CD4",
            "Figure 3.2",
            "In 2021",
            "IL-2",
        ] {
            assert_eq!(show(text), text, "{text}");
        }
    }
}
