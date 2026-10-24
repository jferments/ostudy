//! Request-local writing guidance is not admission policy.
//!
//! The ceilings below bound one generated object, not how much a topic deserves
//! to explain. They are shared by the wire schema and native admission, and sit
//! below the existing storage normalizer's truncation boundaries. The aggregate
//! bound also prevents multiplying every per-field allowance at once. Do not
//! increase these ceilings to tune prose; tune WritingBudget instead.
use super::{KnowledgeCatalog, StudyGuideSectionContent};

pub(super) const MAX_GENERATED_SECTION_CHARS: usize = 64 * 1024;
/// A section shortened for the whole guide's length keeps room for its big
/// idea and a few key points.
const MIN_SCALED_CHARS: usize = 512;
/// Below this many catalog units a short section can be complete.
const MIN_UNITS_FOR_THIN_SECTION: usize = 4;
/// Characters per word of written prose, spaces included.
const CHARS_PER_WORD: usize = 6;
/// Words of a section besides its key points: a one-sentence overview and
/// one retrieval check.
const SECTION_FRAME_WORDS: usize = 45;
/// The most words one key point may have.
pub(super) const KEY_POINT_WORDS: usize = 22;
/// Key points a section is asked for, however small or large its share.
const MIN_KEY_POINTS: usize = 2;
const MAX_KEY_POINTS_ASKED: usize = 10;
pub(super) const MAX_OVERVIEW_CHARS: usize = MAX_GENERATED_SECTION_CHARS;
pub(super) const MAX_KEY_POINTS: usize = 32;
pub(super) const MAX_KEY_POINT_CHARS: usize = 16 * 1024;
pub(super) const MAX_EXPLANATIONS: usize = 16;
pub(super) const MAX_EXPLANATION_HEADING_CHARS: usize = 500;
pub(super) const MAX_EXPLANATION_BODY_CHARS: usize = 16 * 1024;
pub(super) const MAX_IMPORTANT_TERMS: usize = 32;
pub(super) const MAX_TERM_CHARS: usize = 500;
pub(super) const MAX_TERM_EXPLANATION_CHARS: usize = 16 * 1024;
pub(super) const MAX_PROCESS_STEPS: usize = 32;
pub(super) const MAX_PROCESS_STEP_CHARS: usize = 16 * 1024;
pub(super) const MAX_COMPARISONS: usize = 16;
pub(super) const MAX_COMPARISON_SUBJECT_CHARS: usize = 500;
pub(super) const MAX_COMPARISON_RELATIONSHIP_CHARS: usize = 16 * 1024;
pub(super) const MAX_MISCONCEPTIONS: usize = 32;
pub(super) const MAX_MISCONCEPTION_CHARS: usize = 16 * 1024;
pub(super) const MAX_CALLOUTS: usize = 16;
pub(super) const MAX_CALLOUT_TITLE_CHARS: usize = 500;
pub(super) const MAX_CALLOUT_BODY_CHARS: usize = 16 * 1024;
pub(super) const MAX_TABLES: usize = 8;
pub(super) const MAX_TABLE_COLUMNS: usize = 8;
pub(super) const MAX_TABLE_ROWS: usize = 32;
// Column labels use the storage owner's 1,000-character label path too.
pub(super) const MAX_TABLE_CELL_CHARS: usize = 1_000;
pub(super) const MAX_WORKED_EXAMPLES: usize = 8;
pub(super) const MAX_WORKED_EXAMPLE_STEPS: usize = 16;
pub(super) const MAX_RETRIEVAL_CHECKS: usize = 16;
pub(super) const MAX_VISUAL_EXPLANATIONS: usize = 16;
pub(super) const MAX_VISUAL_TITLE_CHARS: usize = 500;
pub(super) const MAX_VISUAL_EXPLANATION_CHARS: usize = 16 * 1024;
// UTF-8 plus JSON escaping/keys/support metadata needs a separate byte bound.
// Both draft retention and targeted repair use this one owner.
pub(super) const MAX_CANDIDATE_BYTES: usize = 384 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WritingBudget {
    pub(super) target_chars: usize,
    pub(super) overview_chars: usize,
}

impl WritingBudget {
    pub(super) fn for_knowledge(knowledge: &KnowledgeCatalog) -> Self {
        // Source length is a sizing signal, never an instruction and never a
        // coverage certificate. Saturation keeps hostile/large inputs bounded.
        let source_chars = knowledge.units.iter().fold(0usize, |total, unit| {
            total.saturating_add(unit.content.statement.chars().count())
        });
        Self::from_signals(knowledge.units.len(), source_chars)
    }

    pub(super) fn from_signals(unit_count: usize, source_chars: usize) -> Self {
        // A guide condenses its source to the key ideas, about a tenth of its
        // length; the source and the flashcards keep the details. The floor
        // leaves room for a big idea, key points and checks on a short topic.
        let target_chars = 200usize
            .saturating_add(unit_count.min(64).saturating_mul(4))
            .saturating_add(source_chars.min(64_000) / 20)
            .clamp(600, 3_000);
        Self {
            target_chars,
            // The overview states the big idea in two or three sentences.
            overview_chars: (target_chars / 5).clamp(250, 500),
        }
    }

    /// The guide's share for this section: `scale` is the whole guide's
    /// length target over the sum of its sections' own budgets, at most 1.
    pub(super) fn scaled(self, scale: f64) -> Self {
        if !(scale > 0.0 && scale < 1.0) {
            return self;
        }
        let target_chars = ((self.target_chars as f64 * scale) as usize).max(MIN_SCALED_CHARS);
        Self {
            target_chars,
            overview_chars: (target_chars / 5).clamp(150, 500),
        }
    }

    pub(super) fn fit_output(self, visible_tokens: usize) -> Self {
        // Reserve JSON keys, selections, and escaping before prose. This is a
        // conservative planning estimate, not a claim about provider token usage.
        let prose_chars = visible_tokens.saturating_sub(768).saturating_mul(2);
        let target_chars = self.target_chars.min(prose_chars).max(512);
        Self {
            target_chars,
            overview_chars: self.overview_chars.min(target_chars.saturating_mul(3) / 5),
        }
    }

    /// Words this section may use.
    pub(super) fn words(self) -> usize {
        self.target_chars / CHARS_PER_WORD
    }

    /// Key points that fit the section's words. Models keep a count they are
    /// given but not a length in characters: asked for 500 characters they
    /// wrote seven 23-word key points and two checks, about 1,100.
    pub(super) fn key_points(self) -> usize {
        (self.words().saturating_sub(SECTION_FRAME_WORDS) + KEY_POINT_WORDS / 2) / KEY_POINT_WORDS
    }

    /// `key_points`, kept within what a section can be asked for.
    pub(super) fn key_points_asked(self) -> usize {
        self.key_points()
            .clamp(MIN_KEY_POINTS, MAX_KEY_POINTS_ASKED)
    }

    /// Words the section is told it has: what its key points, overview and
    /// check hold. Told more words than its capped key points hold, a model
    /// fills them with longer key points (30 words instead of 22).
    pub(super) fn words_asked(self) -> usize {
        self.words()
            .min(SECTION_FRAME_WORDS + self.key_points_asked() * KEY_POINT_WORDS)
    }

    pub(super) fn exceeded_by(self, section: &StudyGuideSectionContent) -> bool {
        section.overview.chars().count() > self.overview_chars
            || super::content::generated_section_char_count(section) > self.target_chars
    }

    /// A section of several catalog units that came back as its overview
    /// alone, with no key points, explanations or table, states none of its
    /// key ideas (a dense table topic once answered in 83 tokens). Such a
    /// section is worth one fresh request.
    pub(super) fn thin(self, unit_count: usize, section: &StudyGuideSectionContent) -> bool {
        unit_count >= MIN_UNITS_FOR_THIN_SECTION
            && section.key_points.is_empty()
            && section.explanations.is_empty()
            && section.tables.is_empty()
    }

    pub(super) fn instructions(self) -> String {
        let key_points = self.key_points_asked();
        format!(
            "Length guide: this section has about {} words across all fields. Write at most {key_points} keyPoints, one per key idea, each one sentence of at most {KEY_POINT_WORDS} words; when the topic has more ideas than {key_points}, join related ones into one key point. Write the overview as one sentence of at most 30 words, and exactly one retrievalCheck. A table or an explanation takes the place of a key point: add one only when a key idea cannot be stated without it, and then write one key point fewer. Leave everything else to the source and the flashcards, but do not drop a qualification a key idea needs. Optional arrays may be empty. Finish a complete JSON object within the visible output allowance.",
            self.words_asked()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_introduction_alone_is_thin_for_a_catalog_of_several_units() {
        let overview_only = StudyGuideSectionContent {
            overview: "x".repeat(300),
            ..Default::default()
        };
        let with_key_points = StudyGuideSectionContent {
            key_points: vec!["A key idea stated plainly.".to_string()],
            ..overview_only.clone()
        };
        let budget = WritingBudget::from_signals(8, 2_400);
        assert!(budget.thin(8, &overview_only));
        assert!(!budget.thin(8, &with_key_points));
        assert!(!budget.thin(3, &overview_only));
    }

    #[test]
    fn a_section_is_asked_for_the_key_points_its_words_hold() {
        let budget = |target_chars| WritingBudget {
            target_chars,
            overview_chars: 150,
        };
        // 133 words: 45 for the overview and check, four of 22 for key points.
        assert_eq!(budget(800).key_points_asked(), 4);
        assert_eq!(budget(512).key_points_asked(), MIN_KEY_POINTS);
        assert_eq!(budget(3_000).key_points_asked(), MAX_KEY_POINTS_ASKED);
        assert!(budget(800).instructions().contains("at most 4 keyPoints"));
        // A section with room for more than the most key points it can be
        // asked for is told only the words those key points hold.
        assert_eq!(budget(6_000).words_asked(), 45 + 10 * 22);
        assert_eq!(budget(800).words_asked(), 133);
    }

    #[test]
    fn targets_follow_topic_evidence_not_document_page_count() {
        let small = WritingBudget::from_signals(1, 200);
        let dense = WritingBudget::from_signals(40, 40_000);
        assert!(dense.target_chars > small.target_chars);
        assert!(dense.overview_chars > small.overview_chars);
        assert!(dense.target_chars < MAX_GENERATED_SECTION_CHARS);
        assert_eq!(
            WritingBudget::from_signals(usize::MAX, usize::MAX).target_chars,
            3_000
        );
        // About a tenth of a topic's source, not a restatement of it.
        let typical = WritingBudget::from_signals(40, 13_000);
        assert!(typical.target_chars * 7 < 13_000);
    }

    #[test]
    fn output_headroom_bounds_guidance_without_becoming_admission() {
        let budget = WritingBudget::from_signals(64, 64_000);
        let small = budget.fit_output(1_536);
        let large = budget.fit_output(6_144);
        assert!(small.target_chars < large.target_chars);
        assert!(small.overview_chars <= small.target_chars);
        let section = StudyGuideSectionContent {
            overview: "A supported explanation with its complete condition. ".repeat(100),
            ..Default::default()
        };
        assert!(small.exceeded_by(&section));
        assert!(super::super::content::validate_generated_safety(&section).is_ok());
    }
}
