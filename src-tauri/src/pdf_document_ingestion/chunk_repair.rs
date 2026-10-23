//! Rebuilds a semantic chunk from its page's text where the structure
//! extractor broke its lines apart.
//!
//! The structure extractor starts a new element at every small glyph, so a
//! paragraph with H₂O or Ca²⁺ in it comes out as pieces: "secretion of HCO",
//! then a lone "−", then "3", with the pieces out of order and the marks lost.
//! The page's own text keeps that paragraph whole and, once its notation is
//! restored, prints the marks. A chunk is replaced by the stretch of page text
//! that holds the same words in the same order; a chunk the page text does not
//! cover that way, such as a table or a figure's labels, is kept as it is.

use std::ops::Range;

/// Chunks with fewer words are headings or stray marks and are kept.
const MIN_WORDS: usize = 5;
/// Words of a matched run; shorter common runs ("of the") are coincidence.
const MIN_RUN: usize = 3;
/// Share of the chunk's words the page text must hold in order, in runs.
const MIN_COVERAGE: f64 = 0.85;
/// The page stretch may hold at most this many words per chunk word.
const MAX_STRETCH: f64 = 1.15;
/// Chunk and page words compared at most, which bounds the table at 8 MB.
const MAX_CELLS: usize = 4_000_000;

/// The stretch of `page_text` that holds `chunk_text`'s words in order, when
/// the two clearly hold the same passage.
pub(crate) fn page_passage(chunk_text: &str, page_text: &str) -> Option<String> {
    let chunk = words(chunk_text);
    let page = words(page_text);
    if chunk.len() < MIN_WORDS || chunk.len().saturating_mul(page.len()) > MAX_CELLS {
        return None;
    }
    let pairs = common_words(&chunk, &page);
    let runs = runs(&pairs);
    let (first, last) = (runs.first()?, runs.last()?);
    let matched = runs.iter().map(|run| run.len()).sum::<usize>();
    if (matched as f64) < MIN_COVERAGE * chunk.len() as f64 {
        return None;
    }
    // Words the chunk has before its first run or after its last are broken
    // pieces; the page's words in the same places stand in for them.
    let (first_chunk, first_page) = pairs[first.start];
    let (last_chunk, last_page) = pairs[last.end - 1];
    let start = first_page.saturating_sub(first_chunk);
    let end = (last_page + chunk.len() - last_chunk).min(page.len());
    if ((end - start) as f64) > MAX_STRETCH * chunk.len() as f64 {
        return None;
    }
    let passage = &page_text[page[start].span.start..page[end - 1].span.end];
    (passage != chunk_text).then(|| passage.to_string())
}

struct Word {
    key: String,
    span: Range<usize>,
}

/// Words keyed by their letters, lower-cased, so "HCO" and "HCO₃⁻" match and
/// numbers and stray marks are left out.
fn words(text: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices().chain([(text.len(), ' ')]) {
        match (character.is_whitespace(), start) {
            (false, None) => start = Some(index),
            (true, Some(begin)) => {
                let key = text[begin..index]
                    .chars()
                    .filter(|character| character.is_alphabetic())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if !key.is_empty() {
                    words.push(Word {
                        key,
                        span: begin..index,
                    });
                }
                start = None;
            }
            _ => {}
        }
    }
    words
}

/// Index pairs of a longest common subsequence of the two word lists.
fn common_words(chunk: &[Word], page: &[Word]) -> Vec<(usize, usize)> {
    let width = page.len() + 1;
    let mut lengths = vec![0u16; (chunk.len() + 1) * width];
    for i in (0..chunk.len()).rev() {
        for j in (0..page.len()).rev() {
            lengths[i * width + j] = if chunk[i].key == page[j].key {
                lengths[(i + 1) * width + j + 1].saturating_add(1)
            } else {
                lengths[(i + 1) * width + j].max(lengths[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pairs = Vec::new();
    while i < chunk.len() && j < page.len() {
        if chunk[i].key == page[j].key {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if lengths[(i + 1) * width + j] >= lengths[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Ranges of `pairs` where both sides advance one word at a time for at
/// least `MIN_RUN` words.
fn runs(pairs: &[(usize, usize)]) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    for index in 1..=pairs.len() {
        let continues = index < pairs.len()
            && pairs[index].0 == pairs[index - 1].0 + 1
            && pairs[index].1 == pairs[index - 1].1 + 1;
        if !continues {
            if index - start >= MIN_RUN {
                runs.push(start..index);
            }
            start = index;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "Secretin is secreted by the S cells of \nthe duodenum in response to H⁺ and fatty acids in the \nlumen of the small intestine. The function of secretin is to promote the secretion \nof pancreatic and biliary HCO₃⁻, which then neutralizes H⁺ in the lumen of the small intestine.\nCholecystokinin\nThe functions of CCK are coordinated to promote fat digestion.";

    #[test]
    fn rebuilds_a_chunk_broken_at_its_marks() {
        let chunk = "the duodenum in response to H\nand fatty acids in the \nlumen of the small intestine. The function of secretin is to promote the secretion \nof pancreatic and biliary HCO3\n, which then neutr\nalizes H\nin the lumen of the small intestine.";
        assert_eq!(
            page_passage(chunk, PAGE).as_deref(),
            Some("the duodenum in response to H⁺ and fatty acids in the \nlumen of the small intestine. The function of secretin is to promote the secretion \nof pancreatic and biliary HCO₃⁻, which then neutralizes H⁺ in the lumen of the small intestine.")
        );
    }

    #[test]
    fn keeps_chunks_the_page_text_does_not_hold() {
        // A table's cells, a heading, and a passage from another page.
        assert_eq!(
            page_passage(
                "Hormone | Site | Stimulus\nGastrin | G cells | Small peptides",
                PAGE
            ),
            None
        );
        assert_eq!(page_passage("Cholecystokinin", PAGE), None);
        assert_eq!(
            page_passage(
                "Saliva is produced by three pairs of glands in the mouth.",
                PAGE
            ),
            None
        );
    }

    #[test]
    fn keeps_a_chunk_the_page_already_spells_the_same_way() {
        let chunk = "The functions of CCK are coordinated to promote fat digestion.";
        assert_eq!(page_passage(chunk, PAGE), None);
    }
}
