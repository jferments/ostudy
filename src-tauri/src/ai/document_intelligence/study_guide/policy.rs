//! Request-local writing guidance is not admission policy.
//!
//! The ceilings below bound one generated object, not how much a topic deserves
//! to explain. They are shared by the wire schema and native admission, and sit
//! below the existing storage normalizer's truncation boundaries. The aggregate
//! bound also prevents multiplying every per-field allowance at once. Do not
//! increase these ceilings to tune prose; tune WritingBudget instead.
use super::{KnowledgeCatalog, StudyGuideSectionContent};

pub(super) const MAX_GENERATED_SECTION_CHARS: usize = 64 * 1024;
/// Below this many catalog units a short section can be complete.
const MIN_UNITS_FOR_THIN_SECTION: usize = 4;
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
        let target_chars = 2_000usize
            .saturating_add(unit_count.min(64).saturating_mul(120))
            .saturating_add(source_chars.min(64_000) / 8)
            .clamp(2_000, 12_000);
        Self {
            target_chars,
            // The overview orients in a few sentences; explanations teach.
            overview_chars: (target_chars / 8).clamp(400, 900),
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

    pub(super) fn exceeded_by(self, section: &StudyGuideSectionContent) -> bool {
        section.overview.chars().count() > self.overview_chars
            || super::content::generated_section_char_count(section) > self.target_chars
    }

    /// A section of several catalog units written at a fifth of its target
    /// usually came back as an introduction alone (a dense table topic once
    /// answered in 83 tokens). Such a section is worth one fresh request.
    pub(super) fn thin(self, unit_count: usize, section: &StudyGuideSectionContent) -> bool {
        unit_count >= MIN_UNITS_FOR_THIN_SECTION
            && super::content::generated_section_char_count(section).saturating_mul(5)
                < self.target_chars
    }

    pub(super) fn instructions(self) -> String {
        format!(
            "Source-aware writing target: at most roughly {} human-readable characters across all categories. The overview orients in roughly {} characters or fewer; the explanations carry the teaching. These are flexible editorial targets, not validity limits. Give more space to mechanisms, qualifications, contrasts and necessary definitions when the evidence needs it. A section that says each fact once is usually shorter than the target; do not pad a simple topic or cut a condition to meet one. Optional arrays may be empty. Finish a complete JSON object within the visible output allowance; never claim exhaustive coverage from a length target.",
            self.target_chars, self.overview_chars
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_introduction_alone_is_thin_for_a_catalog_of_several_units() {
        let section = |chars: usize| StudyGuideSectionContent {
            overview: "x".repeat(chars),
            ..Default::default()
        };
        let budget = WritingBudget::from_signals(8, 2_400);
        assert!(budget.thin(8, &section(300)));
        assert!(!budget.thin(8, &section(budget.target_chars / 2)));
        assert!(!budget.thin(3, &section(300)));
    }

    #[test]
    fn targets_follow_topic_evidence_not_document_page_count() {
        let small = WritingBudget::from_signals(1, 200);
        let dense = WritingBudget::from_signals(13, 18_000);
        assert!(dense.target_chars > small.target_chars);
        assert!(dense.overview_chars > small.overview_chars);
        assert!(dense.target_chars < MAX_GENERATED_SECTION_CHARS);
        assert_eq!(
            WritingBudget::from_signals(usize::MAX, usize::MAX).target_chars,
            12_000
        );
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
