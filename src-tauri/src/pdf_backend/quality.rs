use super::contracts::TextQualityAssessment;

const MIN_SUBSTANTIVE_NONSPACE_CHARS: usize = 24;
const MIN_SUBSTANTIVE_ALPHA_CHARS: usize = 8;
const MIN_SUBSTITUTION_ALPHA_CHARS: usize = 48;

pub(crate) fn native_page_requires_ocr(text: &str, assessment: &TextQualityAssessment) -> bool {
    if !assessment.suspicious {
        return false;
    }
    if assessment.reasons.contains(&"probable_font_substitution") {
        return true;
    }

    let mut visible = 0_usize;
    let mut damaged = 0_usize;
    for character in text.chars() {
        if character.is_whitespace() {
            continue;
        }
        visible = visible.saturating_add(1);
        if is_unmapped_pdf_glyph(character) {
            damaged = damaged.saturating_add(1);
        }
    }
    damaged >= 24 && damaged.saturating_mul(100) >= visible.max(1).saturating_mul(30)
}

pub(crate) fn assess_text_quality(text: &str) -> TextQualityAssessment {
    let mut nonspace = 0_usize;
    let mut alphabetic = 0_usize;
    let mut ascii_alphabetic = 0_usize;
    let mut latin1_alphabetic = 0_usize;
    let mut private_use = 0_usize;
    let mut replacement = 0_usize;
    let mut invalid_controls = 0_usize;

    for character in text.chars() {
        if !character.is_whitespace() {
            nonspace = nonspace.saturating_add(1);
        }
        if character.is_alphabetic() {
            alphabetic = alphabetic.saturating_add(1);
            if character.is_ascii_alphabetic() {
                ascii_alphabetic = ascii_alphabetic.saturating_add(1);
            } else if ('\u{00C0}'..='\u{00FF}').contains(&character) {
                latin1_alphabetic = latin1_alphabetic.saturating_add(1);
            }
        }
        if is_private_use(character) {
            private_use = private_use.saturating_add(1);
        }
        if character == '\u{FFFD}' {
            replacement = replacement.saturating_add(1);
        }
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
            invalid_controls = invalid_controls.saturating_add(1);
        }
    }

    let mut reasons = Vec::new();
    if private_use >= 2
        && (nonspace < 50 || private_use.saturating_mul(100) >= nonspace.saturating_mul(1))
    {
        reasons.push("private_use_unicode");
    }
    if replacement >= 3 && replacement.saturating_mul(100) >= nonspace.max(1) {
        reasons.push("unicode_replacement_character");
    }
    if invalid_controls >= 8
        && invalid_controls.saturating_mul(100) >= nonspace.max(1).saturating_mul(5)
    {
        reasons.push("invalid_control_characters");
    }
    if alphabetic >= MIN_SUBSTITUTION_ALPHA_CHARS
        && latin1_alphabetic.saturating_mul(100) >= alphabetic.saturating_mul(55)
        && ascii_alphabetic.saturating_mul(100) <= alphabetic.saturating_mul(25)
    {
        reasons.push("probable_font_substitution");
    }

    TextQualityAssessment {
        suspicious: !reasons.is_empty(),
        substantive: nonspace >= MIN_SUBSTANTIVE_NONSPACE_CHARS
            && alphabetic >= MIN_SUBSTANTIVE_ALPHA_CHARS,
        reasons,
    }
}

fn is_unmapped_pdf_glyph(character: char) -> bool {
    let value = character as u32;
    (character.is_control() && !matches!(character, '\n' | '\t'))
        || character == '\u{fffd}'
        || is_private_use(character)
        || (0xFDD0..=0xFDEF).contains(&value)
        || value & 0xFFFF == 0xFFFE
        || value & 0xFFFF == 0xFFFF
}

fn is_private_use(character: char) -> bool {
    matches!(
        character as u32,
        0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_use_font_mapping_is_detected() {
        let assessment = assess_text_quality(
            "Th\u{e01c} purp\u{e01d}\u{e01e}\u{e01c} of HDL appears to transport cholesterol.",
        );
        assert!(assessment.suspicious);
        assert!(assessment.reasons.contains(&"private_use_unicode"));
    }

    #[test]
    fn latin1_substitution_does_not_flag_normal_accents() {
        let corrupt = assess_text_quality(
            "Õâóïáäèå ÖîããóÛó îì ÖèïÝáîãîÛ× èåÝ çôó Õâóïáäèå ëóèïç Õííîäáèçáîå ñðæãáíôóÝ åóØ ÛðáÝóãáåóí",
        );
        assert!(corrupt.reasons.contains(&"probable_font_substitution"));
        let normal = assess_text_quality(
            "L'étude cardiovasculaire évalue la pression artérielle et l'activité physique régulière.",
        );
        assert!(!normal.suspicious);
    }

    #[test]
    fn sparse_control_glyphs_do_not_force_ocr() {
        let assessment = assess_text_quality(
            "The active blood-gas barrier is around 0.3 \u{001e}m thick and the capillary is under 3 \u{001e}m in scale.",
        );
        assert!(!assessment.suspicious);
    }

    #[test]
    fn dense_control_corruption_requires_recovery() {
        let corrupt = "A\u{0001}B\u{0002}C\u{0003}D\u{0004}E\u{0005}F\u{0006}G\u{0007}H\u{0008}I\u{0009}J\u{000b}K\u{000c}L\u{000e}M\u{000f}N";
        assert!(assess_text_quality(corrupt).suspicious);
    }
}
