use serde::Deserialize;

use super::super::ClozeMode;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GeneratedImportDeckMode {
    Create,
    Append,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GeneratedImportFile {
    pub(super) format: String,
    pub(super) format_version: i64,
    pub(super) deck: GeneratedImportDeck,
    #[serde(default)]
    pub(super) default_tags: Vec<String>,
    pub(super) cards: Vec<GeneratedImportCard>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GeneratedImportDeck {
    pub(super) mode: GeneratedImportDeckMode,
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum GeneratedImportCard {
    Basic {
        front: String,
        back: String,
        #[serde(default, rename = "reverseEnabled")]
        reverse_enabled: bool,
        #[serde(default)]
        tags: Vec<String>,
    },
    Cloze {
        text: String,
        #[serde(default)]
        mode: ClozeMode,
        #[serde(default)]
        tags: Vec<String>,
    },
}
