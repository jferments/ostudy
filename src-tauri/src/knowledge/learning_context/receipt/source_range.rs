//! Exact block locations in the hashed source text. Ambiguity is not resolved
//! by selecting the first repeated phrase or by normalizing literal evidence.

pub(super) fn unique(source: &str, block: &str) -> Option<(usize, usize)> {
    if block.is_empty() {
        return None;
    }
    let start = source.find(block)?;
    (source.rfind(block) == Some(start)).then_some((start, start + block.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_structure_range_uses_exact_utf8_byte_offsets() {
        let text = "α\nOnly when x ≤ 4.\nω";
        let (start, end) = unique(text, "Only when x ≤ 4.").unwrap();
        assert_eq!(start, "α\n".len());
        assert_eq!(&text[start..end], "Only when x ≤ 4.");
    }

    #[test]
    fn document_structure_range_rejects_repeated_and_overlapping_matches() {
        assert_eq!(unique("note note", "note"), None);
        assert_eq!(unique("ababa", "aba"), None);
        assert_eq!(unique("ααα", "αα"), None);
    }

    #[test]
    fn document_structure_range_does_not_guess_missing_or_normalized_text() {
        assert_eq!(unique("x  < 4", "x < 4"), None);
        assert_eq!(unique("source", ""), None);
        assert_eq!(unique("", "source"), None);
        assert_eq!(unique("source", "source"), Some((0, 6)));
    }
}
