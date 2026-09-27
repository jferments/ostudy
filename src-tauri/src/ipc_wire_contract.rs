//! Native-owned IPC wire contract fixtures and TypeScript declarations.
//!
//! Product DTOs remain owned by their domain modules. This boundary records the
//! serialization conventions shared with the frontend and renders checked-in
//! TypeScript declarations from Rust-owned descriptors. Representative native
//! values verify field names/nullability/numbers, while an encoding fixture
//! verifies tagged unions and byte-array lowering.

use std::fmt::Write as _;

use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    decks::DeckSummaryDto,
    error::CommandError,
    library_backup::live::LiveRestoreResult,
    library_session::SessionStatus,
    settings::AppSettingsDto,
};

#[derive(Clone, Copy)]
enum TypeScriptType {
    String,
    Number,
    Boolean,
    NullableNumber,
    Named(&'static str),
}

impl TypeScriptType {
    const fn source(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::NullableNumber => "number | null",
            Self::Named(source) => source,
        }
    }
}

#[derive(Clone, Copy)]
struct FieldContract {
    name: &'static str,
    ty: TypeScriptType,
}

#[derive(Clone, Copy)]
struct ObjectContract {
    name: &'static str,
    fields: &'static [FieldContract],
}

const COMMAND_ERROR_FIELDS: &[FieldContract] = &[
    FieldContract {
        name: "code",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "message",
        ty: TypeScriptType::String,
    },
];

const DECK_SUMMARY_FIELDS: &[FieldContract] = &[
    FieldContract {
        name: "id",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "name",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "description",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "sourceNoteCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "generatedCardCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "studyableDueCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "archivedAt",
        ty: TypeScriptType::NullableNumber,
    },
    FieldContract {
        name: "createdAt",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "updatedAt",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "revision",
        ty: TypeScriptType::Number,
    },
];

const APP_SETTINGS_FIELDS: &[FieldContract] = &[
    FieldContract {
        name: "theme",
        ty: TypeScriptType::Named("\"light\" | \"dark\" | \"console\" | \"synthwave\""),
    },
    FieldContract {
        name: "uiFont",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "cardFont",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "uiScale",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "cardBaseSize",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "motionPreference",
        ty: TypeScriptType::Named("\"system\" | \"full\" | \"reduced\" | \"none\""),
    },
    FieldContract {
        name: "materialPreference",
        ty: TypeScriptType::Named("\"automatic\" | \"rich\" | \"simple\""),
    },
    FieldContract {
        name: "decorativeArtwork",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "celebrationsEnabled",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "tutorialsEnabled",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "confirmDeletions",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "practiceShuffle",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "displayQuestionOnAnswer",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "desiredRetention",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "dailyNewLimit",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "dailyReviewLimit",
        ty: TypeScriptType::Number,
    },
];

const COMPLETED_RESTORE_FIELDS: &[FieldContract] = &[
    FieldContract {
        name: "operationId",
        ty: TypeScriptType::String,
    },
    FieldContract {
        name: "deckCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "cardCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "documentCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "templateCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "reviewCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "assetCount",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "restartRequired",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "completed",
        ty: TypeScriptType::Boolean,
    },
];

const LIBRARY_SESSION_FIELDS: &[FieldContract] = &[
    FieldContract {
        name: "generation",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "ready",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "restoring",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "recoveryRequired",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "lastRestore",
        ty: TypeScriptType::Named("NativeCompletedLibraryRestore | null"),
    },
];

const OBJECT_CONTRACTS: &[ObjectContract] = &[
    ObjectContract {
        name: "NativeCommandError",
        fields: COMMAND_ERROR_FIELDS,
    },
    ObjectContract {
        name: "NativeDeckSummary",
        fields: DECK_SUMMARY_FIELDS,
    },
    ObjectContract {
        name: "NativeAppSettings",
        fields: APP_SETTINGS_FIELDS,
    },
    ObjectContract {
        name: "NativeCompletedLibraryRestore",
        fields: COMPLETED_RESTORE_FIELDS,
    },
    ObjectContract {
        name: "NativeLibrarySession",
        fields: LIBRARY_SESSION_FIELDS,
    },
];

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EncodingFixture {
    Ready {
        bytes: Vec<u8>,
        optional: Option<String>,
        signed: i64,
        unsigned: u64,
    },
    Empty,
}

pub fn render_typescript() -> String {
    let mut output = String::from(
        "/* Generated from src-tauri/src/ipc_wire_contract.rs. Do not edit by hand. */\n\n",
    );
    for contract in OBJECT_CONTRACTS {
        writeln!(&mut output, "export interface {} {{", contract.name)
            .expect("write to String cannot fail");
        for field in contract.fields {
            writeln!(&mut output, "  {}: {};", field.name, field.ty.source())
                .expect("write to String cannot fail");
        }
        output.push_str("}\n\n");
    }
    output.push_str(
        "export type NativeWireEncodingFixture =\n\
         \  | { kind: \"ready\"; bytes: number[]; optional: string | null; signed: number; unsigned: number }\n\
         \  | { kind: \"empty\" };\n",
    );
    output
}

pub fn representative_fixtures() -> Value {
    let restore = LiveRestoreResult {
        operation_id: "22222222222222222222222222222222".to_string(),
        deck_count: 2,
        card_count: 5,
        document_count: 1,
        template_count: 3,
        review_count: 8,
        asset_count: 4,
        restart_required: false,
        completed: true,
    };
    json!({
        "commandError": CommandError::new("example_error", "Example safe message."),
        "deckSummary": DeckSummaryDto {
            id: "11111111111111111111111111111111".to_string(),
            name: "Physiology".to_string(),
            description: "Representative deck".to_string(),
            source_note_count: 3,
            generated_card_count: 5,
            studyable_due_count: 2,
            archived_at: None,
            created_at: 10,
            updated_at: 20,
            revision: 4,
        },
        "appSettings": AppSettingsDto {
            theme: "dark".to_string(),
            ui_font: "plex".to_string(),
            card_font: "plex".to_string(),
            ui_scale: 100,
            card_base_size: 18,
            motion_preference: "system".to_string(),
            material_preference: "automatic".to_string(),
            decorative_artwork: true,
            celebrations_enabled: true,
            tutorials_enabled: false,
            confirm_deletions: true,
            practice_shuffle: true,
            display_question_on_answer: false,
            desired_retention: 0.9,
            daily_new_limit: 20,
            daily_review_limit: 200,
        },
        "completedRestore": restore.clone(),
        "librarySession": SessionStatus {
            generation: 7,
            ready: true,
            restoring: false,
            recovery_required: false,
            last_restore: Some(restore),
        },
        "encoding": EncodingFixture::Ready {
            bytes: vec![0, 127, 255],
            optional: None,
            signed: -7,
            unsigned: 42,
        },
        "emptyEncoding": EncodingFixture::Empty,
    })
}

pub fn render_fixtures_json() -> String {
    let mut output =
        serde_json::to_string_pretty(&representative_fixtures()).expect("fixture serialization");
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATED_TYPESCRIPT: &str =
        include_str!("../../src/ipc/generated/nativeWireContracts.ts");
    const GENERATED_FIXTURES: &str =
        include_str!("../../src/ipc/generated/nativeWireFixtures.json");

    #[test]
    fn checked_in_typescript_matches_native_contract_renderer() {
        assert_eq!(GENERATED_TYPESCRIPT, render_typescript());
    }

    #[test]
    fn checked_in_fixtures_match_native_serialization() {
        let checked: Value = serde_json::from_str(GENERATED_FIXTURES).unwrap();
        assert_eq!(checked, representative_fixtures());
    }

    #[test]
    fn object_contracts_cover_every_representative_product_field() {
        let fixtures = representative_fixtures();
        for (fixture_name, contract) in [
            ("commandError", OBJECT_CONTRACTS[0]),
            ("deckSummary", OBJECT_CONTRACTS[1]),
            ("appSettings", OBJECT_CONTRACTS[2]),
            ("completedRestore", OBJECT_CONTRACTS[3]),
            ("librarySession", OBJECT_CONTRACTS[4]),
        ] {
            let object = fixtures[fixture_name].as_object().unwrap();
            let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
            let mut expected = contract.fields.iter().map(|field| field.name).collect::<Vec<_>>();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{fixture_name} field drift");
        }
    }
}
