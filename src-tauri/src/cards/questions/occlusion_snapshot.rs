//! Native self-assessment projection from a captured target. This adapter exposes
//! outlines, not source pixels. Only the authorized raster service selects a
//! render phase; ordinary objective projection still refuses an occlusion image.

use super::*;
use crate::cards::{
    collect_asset_ids,
    questions::occlusion::{CanonicalImage, RenderPhase, RenderedOcclusion},
};

impl QuestionInstance {
    pub fn kind(&self) -> QuestionKind {
        self.question.kind()
    }

    pub fn question_for_attempt(&self) -> Result<PublicQuestion, QuestionError> {
        if self.kind() != QuestionKind::ImageOcclusion {
            return self.public_question();
        }
        let target = self.occlusion_target()?;
        let plan = self.occlusion_mask_plan()?;
        if collect_asset_ids(&target.prompt)?.contains(&plan.image_asset_id) {
            return Err(QuestionError::new(
                "occlusion_source_in_prompt",
                "The unmasked source image cannot also be embedded in the question prompt.",
            ));
        }
        Ok(PublicQuestion::ImageOcclusion {
            prompt: target.prompt.clone(),
            target_rectangles: plan
                .hidden_masks
                .iter()
                .filter(|mask| plan.target_mask_ids.contains(&mask.id))
                .map(|mask| mask.rect)
                .collect(),
        })
    }

    /// Native-only access to the frozen target. The response owner decides when
    /// its answer/explanation can leave Rust; the source definition stays private.
    pub fn occlusion_target(&self) -> Result<&OcclusionQuestion, QuestionError> {
        let QuestionBody::ImageOcclusion { questions, .. } = &self.question.definition().question
        else {
            return Err(invalid("This is not a self-assessed image question."));
        };
        questions
            .iter()
            .find(|question| question.id == self.presentation_key)
            .ok_or_else(|| invalid("The captured image target is unavailable."))
    }

    pub fn render_occlusion(
        &self,
        image: &CanonicalImage,
        phase: RenderPhase,
        maximum_edge: u32,
    ) -> Result<RenderedOcclusion, QuestionError> {
        image.render(&self.question, &self.presentation_key, phase, maximum_edge)
    }
}
