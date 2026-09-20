//! Conservative source-role decisions. No provider, guessed URL, or OCR rewrite.

use serde::{Deserialize, Serialize};

use crate::pdf_ocr_backend::regions::{OcrLine, PixelBounds};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegionRole {
    Learning,
    Metadata,
    Interface,
    Notice,
    OutsideSelection,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegionOverride {
    Include,
    Exclude,
    Metadata,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegionDecision {
    pub id: String,
    pub text: String,
    pub bounds: Option<PixelBounds>,
    pub automatic_role: RegionRole,
    pub role: RegionRole,
    pub reason: String,
    pub override_action: Option<RegionOverride>,
    pub equivalent_to: Option<String>,
}

impl RegionDecision {
    pub(crate) fn admitted(&self) -> bool {
        matches!(self.role, RegionRole::Learning | RegionRole::Unresolved)
    }
}

pub(crate) fn decide(
    id: String,
    lines: &[OcrLine],
    bounds: Option<PixelBounds>,
    height: u32,
    include_interface: bool,
    override_action: Option<RegionOverride>,
) -> RegionDecision {
    let text = lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n");
    let located = bounds.is_some();
    let confident = lines.iter().all(|line| line.confidence.is_some_and(|value| value >= 0.8));
    let edge = bounds.is_some_and(|b| b.bottom <= height / 5 || b.top >= height * 4 / 5);
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    // Do not classify prose merely because it mentions a control or copyright.
    // Literal short labels are eligible only at an image edge, with native geometry.
    let control = matches!(normalized.as_str(),
        "accept all cookies" | "reject all cookies" | "cookie settings" |
        "manage cookies" | "skip to main content" | "sign in" | "log in" |
        "forgot password?" | "privacy policy" | "terms of service" | "advertisement" |
        "sponsored" | "subscribe" | "related articles" | "recommended for you" | "share this article");
    let notice = normalized.len() <= 240 && (
        normalized.starts_with("we use cookies to ") ||
        normalized.starts_with("this website uses cookies "));
    let attribution = text.chars().count() <= 240 && (
        text.starts_with('©') || normalized.starts_with("copyright ©") ||
        normalized.starts_with("author: ") || normalized.starts_with("published: ") ||
        normalized.starts_with("last updated: "));
    let (automatic_role, reason) = if include_interface {
        (RegionRole::Learning, "All source content is included by your document policy.")
    } else if confident && located && edge && attribution {
        (RegionRole::Metadata, "Source attribution is retained as metadata, not discarded.")
    } else if confident && located && edge && control {
        (RegionRole::Interface, "A standalone interface label at the image edge is excluded.")
    } else if confident && located && edge && notice {
        (RegionRole::Notice, "A standalone cookie notice at the image edge is excluded.")
    } else if !located || !confident {
        (RegionRole::Unresolved, "Geometry or recognition confidence is uncertain; the literal evidence is retained.")
    } else {
        (RegionRole::Learning, "No sufficiently specific boilerplate rule applies; the source is retained.")
    };
    let (role, reason) = match override_action {
        Some(RegionOverride::Include) => (RegionRole::Learning, "You restored this source region."),
        Some(RegionOverride::Exclude) => (RegionRole::Interface, "You excluded this region from learning inputs."),
        Some(RegionOverride::Metadata) => (RegionRole::Metadata, "You retained this region as source metadata."),
        None => (automatic_role, reason),
    };
    RegionDecision { id, text, bounds, automatic_role, role, reason: reason.to_string(), override_action, equivalent_to: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(text: &str, top: u32, policy: bool, action: Option<RegionOverride>) -> RegionDecision {
        let bounds = PixelBounds { left: 2, top, right: 90, bottom: top + 10 };
        decide("a".repeat(64), &[OcrLine { text: text.to_string(), bounds: Some(bounds),
            group: "1".to_string(), confidence: Some(0.95) }], Some(bounds), 100, policy, action)
    }

    #[test]
    fn image_filter_classifies_only_specific_edge_controls_and_retains_attribution() {
        assert_eq!(decision("Accept all cookies", 1, false, None).role, RegionRole::Interface);
        assert_eq!(decision("© 2026 Example. All rights reserved.", 88, false, None).role, RegionRole::Metadata);
        assert_eq!(decision("We use cookies to remember settings.", 1, false, None).role, RegionRole::Notice);
        for text in ["Select Accept all cookies to test this interface.", "Do not sign in with another account.",
            "Copyright exceptions and fair use", "5 mg, not 50 mg", "Author: means the creator in this example."] {
            assert!(decision(text, 40, false, None).admitted());
        }
    }

    #[test]
    fn image_filter_restore_and_interface_subject_policy_are_reversible() {
        assert!(decision("Sign in", 1, true, None).admitted());
        let restored = decision("Sign in", 1, false, Some(RegionOverride::Include));
        assert!(restored.admitted());
        assert_eq!(restored.automatic_role, RegionRole::Interface);
        assert!(!decision("Sign in", 1, false, None).admitted());
        assert_eq!(decision("Important reference", 40, false, Some(RegionOverride::Metadata)).role, RegionRole::Metadata);
    }

    #[test]
    fn image_filter_never_guesses_geometry_or_recovers_obscured_words() {
        let text = "Accept all cookies";
        let result = decide("b".repeat(64), &[OcrLine { text: text.to_string(), bounds: None,
            group: String::new(), confidence: Some(0.1) }], None, 100, false, None);
        assert_eq!(result.role, RegionRole::Unresolved);
        assert!(result.admitted());
        assert_eq!(result.text, text);
    }
}
