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
    home_summary::{HomeSubjectSummaryDto, HomeSummaryDto, HomeTodayDto, SubjectCoverDto},
    library_backup::live::LiveRestoreResult,
    library_session::SessionStatus,
    settings::AppSettingsDto,
    subjects::{
        DocumentStudyRangeDto, SubjectDeckPlacementDto, SubjectDocumentPlacementDto, SubjectDto,
        SubjectFilingReviewDto, SubjectLibraryDto,
    },
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
    FieldContract {
        name: "newDayStartHour",
        ty: TypeScriptType::Number,
    },
    FieldContract {
        name: "guidePdfTheme",
        ty: TypeScriptType::Named("\"navy\" | \"sage\" | \"slate\" | \"plum\""),
    },
    FieldContract {
        name: "guidePdfIncludeNotes",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "lectureAudioWithNewLessons",
        ty: TypeScriptType::Boolean,
    },
    FieldContract {
        name: "speechAutoSendSeconds",
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

const fn field(name: &'static str, ty: TypeScriptType) -> FieldContract {
    FieldContract { name, ty }
}

const NULLABLE_STRING: TypeScriptType = TypeScriptType::Named("string | null");

const SUBJECT_FIELDS: &[FieldContract] = &[
    field("id", TypeScriptType::String),
    field("name", TypeScriptType::String),
    field("sortOrder", TypeScriptType::Number),
    field("createdAt", TypeScriptType::Number),
    field("updatedAt", TypeScriptType::Number),
    field("revision", TypeScriptType::Number),
];

const SUBJECT_DECK_PLACEMENT_FIELDS: &[FieldContract] = &[
    field("deckId", TypeScriptType::String),
    field("subjectId", NULLABLE_STRING),
    field("sortOrder", TypeScriptType::Number),
];

const SUBJECT_DOCUMENT_PLACEMENT_FIELDS: &[FieldContract] = &[
    field("documentId", TypeScriptType::String),
    field("subjectId", NULLABLE_STRING),
];

const DOCUMENT_STUDY_RANGE_FIELDS: &[FieldContract] = &[
    field("id", TypeScriptType::String),
    field("documentId", TypeScriptType::String),
    field("unitKind", TypeScriptType::String),
    field("startUnit", TypeScriptType::Number),
    field("endUnit", TypeScriptType::Number),
    field("sourceSha256", TypeScriptType::String),
    field("createdAt", TypeScriptType::Number),
];

const SUBJECT_LIBRARY_FIELDS: &[FieldContract] = &[
    field("libraryRevision", TypeScriptType::Number),
    field("documentFilingRevision", TypeScriptType::Number),
    field("subjects", TypeScriptType::Named("NativeSubject[]")),
    field(
        "decks",
        TypeScriptType::Named("NativeSubjectDeckPlacement[]"),
    ),
    field(
        "documents",
        TypeScriptType::Named("NativeSubjectDocumentPlacement[]"),
    ),
    field(
        "studyRanges",
        TypeScriptType::Named("NativeDocumentStudyRange[]"),
    ),
    field("leftOutDocumentIds", TypeScriptType::Named("string[]")),
    field("checkTheseCount", TypeScriptType::Number),
];

const SUBJECT_FILING_REVIEW_FIELDS: &[FieldContract] = &[
    field(
        "entityKind",
        TypeScriptType::Named("\"subject\" | \"deck\" | \"document\" | \"note\""),
    ),
    field("entityId", TypeScriptType::String),
    field("reason", TypeScriptType::String),
    field("detail", TypeScriptType::Named("Record<string, unknown>")),
    field("label", TypeScriptType::String),
];

const HOME_SUBJECT_SUMMARY_FIELDS: &[FieldContract] = &[
    field("subjectId", NULLABLE_STRING),
    field("dueReviews", TypeScriptType::Number),
    field("dueNew", TypeScriptType::Number),
    field("scheduledReviews", TypeScriptType::Number),
    field("scheduledNew", TypeScriptType::Number),
    field("cardCount", TypeScriptType::Number),
    field("deckCount", TypeScriptType::Number),
    field("sourceCount", TypeScriptType::Number),
    field("hasOverviewGuide", TypeScriptType::Boolean),
    field("guideCount", TypeScriptType::Number),
    field("lessonCount", TypeScriptType::Number),
    field("podcastCount", TypeScriptType::Number),
    field("cover", TypeScriptType::Named("NativeSubjectCover | null")),
    field("dueDeckIds", TypeScriptType::Named("string[]")),
];

const SUBJECT_COVER_FIELDS: &[FieldContract] = &[
    field("documentId", TypeScriptType::String),
    field("pageNumber", TypeScriptType::Number),
    field("pageWidth", TypeScriptType::Number),
    field("pageHeight", TypeScriptType::Number),
    field("left", TypeScriptType::Number),
    field("top", TypeScriptType::Number),
    field("width", TypeScriptType::Number),
    field("height", TypeScriptType::Number),
];

const HOME_TODAY_FIELDS: &[FieldContract] = &[
    field("dueReviews", TypeScriptType::Number),
    field("dueNew", TypeScriptType::Number),
    field("scheduledReviews", TypeScriptType::Number),
    field("scheduledNew", TypeScriptType::Number),
    field("reviewsRemaining", TypeScriptType::Number),
    field("newRemaining", TypeScriptType::Number),
    field("subjectCount", TypeScriptType::Number),
    field("deckIds", TypeScriptType::Named("string[]")),
    field("estimatedMinutes", TypeScriptType::Number),
    field("estimateMsPerCard", TypeScriptType::Number),
    field("estimateReviewCount", TypeScriptType::Number),
];

const HOME_SUMMARY_FIELDS: &[FieldContract] = &[
    field("studyDate", TypeScriptType::String),
    field("today", TypeScriptType::Named("NativeHomeToday")),
    field(
        "subjects",
        TypeScriptType::Named("NativeHomeSubjectSummary[]"),
    ),
    field(
        "noSubject",
        TypeScriptType::Named("NativeHomeSubjectSummary"),
    ),
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
    ObjectContract {
        name: "NativeSubject",
        fields: SUBJECT_FIELDS,
    },
    ObjectContract {
        name: "NativeSubjectDeckPlacement",
        fields: SUBJECT_DECK_PLACEMENT_FIELDS,
    },
    ObjectContract {
        name: "NativeSubjectDocumentPlacement",
        fields: SUBJECT_DOCUMENT_PLACEMENT_FIELDS,
    },
    ObjectContract {
        name: "NativeDocumentStudyRange",
        fields: DOCUMENT_STUDY_RANGE_FIELDS,
    },
    ObjectContract {
        name: "NativeSubjectLibrary",
        fields: SUBJECT_LIBRARY_FIELDS,
    },
    ObjectContract {
        name: "NativeSubjectFilingReview",
        fields: SUBJECT_FILING_REVIEW_FIELDS,
    },
    ObjectContract {
        name: "NativeSubjectCover",
        fields: SUBJECT_COVER_FIELDS,
    },
    ObjectContract {
        name: "NativeHomeSubjectSummary",
        fields: HOME_SUBJECT_SUMMARY_FIELDS,
    },
    ObjectContract {
        name: "NativeHomeToday",
        fields: HOME_TODAY_FIELDS,
    },
    ObjectContract {
        name: "NativeHomeSummary",
        fields: HOME_SUMMARY_FIELDS,
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
    output.push_str(concat!(
        "export type NativeWireEncodingFixture =\n",
        "  | { kind: \"ready\"; bytes: number[]; optional: string | null; signed: number; unsigned: number }\n",
        "  | { kind: \"empty\" };\n",
    ));
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
            new_day_start_hour: 4,
            guide_pdf_theme: "sage".to_string(),
            guide_pdf_include_notes: true,
            lecture_audio_with_new_lessons: true,
            speech_auto_send_seconds: 2,
        },
        "completedRestore": restore.clone(),
        "librarySession": SessionStatus {
            generation: 7,
            ready: true,
            restoring: false,
            recovery_required: false,
            last_restore: Some(restore),
        },
        "subjectLibrary": SubjectLibraryDto {
            library_revision: 9,
            document_filing_revision: 4,
            subjects: vec![SubjectDto {
                id: "33333333333333333333333333333333".to_string(),
                name: "Pharmacology".to_string(),
                sort_order: 1000,
                created_at: 10,
                updated_at: 20,
                revision: 2,
            }],
            decks: vec![
                SubjectDeckPlacementDto {
                    deck_id: "11111111111111111111111111111111".to_string(),
                    subject_id: Some("33333333333333333333333333333333".to_string()),
                    sort_order: 1000,
                },
                SubjectDeckPlacementDto {
                    deck_id: "44444444444444444444444444444444".to_string(),
                    subject_id: None,
                    sort_order: 1000,
                },
            ],
            documents: vec![SubjectDocumentPlacementDto {
                document_id: "55555555555555555555555555555555".to_string(),
                subject_id: Some("33333333333333333333333333333333".to_string()),
            }],
            study_ranges: vec![DocumentStudyRangeDto {
                id: "66666666666666666666666666666666".to_string(),
                document_id: "55555555555555555555555555555555".to_string(),
                unit_kind: "page".to_string(),
                start_unit: 10,
                end_unit: 40,
                source_sha256: "7".repeat(64),
                created_at: 30,
            }],
            left_out_document_ids: vec!["55555555555555555555555555555555".to_string()],
            check_these_count: 1,
        },
        "subjectFilingReview": SubjectFilingReviewDto {
            entity_kind: "deck".to_string(),
            entity_id: "44444444444444444444444444444444".to_string(),
            reason: "moved_to_collection_subject".to_string(),
            detail: json!({
                "folderSubjectId": null,
                "subjectIds": ["33333333333333333333333333333333"],
            }),
            label: "Pharmacology study bundle".to_string(),
        },
        "homeSummary": HomeSummaryDto {
            study_date: "2026-10-05".to_string(),
            today: HomeTodayDto {
                due_reviews: 31,
                due_new: 12,
                scheduled_reviews: 31,
                scheduled_new: 12,
                reviews_remaining: 180,
                new_remaining: 15,
                subject_count: 2,
                deck_ids: vec![
                    "11111111111111111111111111111111".to_string(),
                    "44444444444444444444444444444444".to_string(),
                ],
                estimated_minutes: 7,
                estimate_ms_per_card: 9_500,
                estimate_review_count: 120,
            },
            subjects: vec![HomeSubjectSummaryDto {
                subject_id: Some("33333333333333333333333333333333".to_string()),
                due_reviews: 28,
                due_new: 12,
                scheduled_reviews: 28,
                scheduled_new: 12,
                card_count: 340,
                deck_count: 1,
                source_count: 1,
                has_overview_guide: true,
                guide_count: 2,
                lesson_count: 16,
                podcast_count: 1,
                cover: Some(SubjectCoverDto {
                    document_id: "55555555555555555555555555555555".to_string(),
                    page_number: 4,
                    page_width: 612.0,
                    page_height: 792.0,
                    left: 72.0,
                    top: 120.5,
                    width: 300.0,
                    height: 200.0,
                }),
                due_deck_ids: vec!["11111111111111111111111111111111".to_string()],
            }],
            no_subject: HomeSubjectSummaryDto {
                subject_id: None,
                due_reviews: 3,
                due_new: 0,
                scheduled_reviews: 3,
                scheduled_new: 0,
                card_count: 25,
                deck_count: 1,
                source_count: 0,
                has_overview_guide: false,
                guide_count: 0,
                lesson_count: 0,
                podcast_count: 0,
                cover: None,
                due_deck_ids: vec!["44444444444444444444444444444444".to_string()],
            },
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
        let covered = [
            ("/commandError", "NativeCommandError"),
            ("/deckSummary", "NativeDeckSummary"),
            ("/appSettings", "NativeAppSettings"),
            ("/completedRestore", "NativeCompletedLibraryRestore"),
            ("/librarySession", "NativeLibrarySession"),
            ("/subjectLibrary/subjects/0", "NativeSubject"),
            ("/subjectLibrary/decks/1", "NativeSubjectDeckPlacement"),
            (
                "/subjectLibrary/documents/0",
                "NativeSubjectDocumentPlacement",
            ),
            ("/subjectLibrary/studyRanges/0", "NativeDocumentStudyRange"),
            ("/subjectLibrary", "NativeSubjectLibrary"),
            ("/subjectFilingReview", "NativeSubjectFilingReview"),
            ("/homeSummary/subjects/0/cover", "NativeSubjectCover"),
            ("/homeSummary/subjects/0", "NativeHomeSubjectSummary"),
            ("/homeSummary/today", "NativeHomeToday"),
            ("/homeSummary", "NativeHomeSummary"),
        ];
        let mut named = covered.iter().map(|(_, name)| *name).collect::<Vec<_>>();
        let mut declared = OBJECT_CONTRACTS
            .iter()
            .map(|contract| contract.name)
            .collect::<Vec<_>>();
        named.sort_unstable();
        declared.sort_unstable();
        assert_eq!(named, declared, "every object contract needs a fixture");
        for (fixture_name, contract_name) in covered {
            let contract = OBJECT_CONTRACTS
                .iter()
                .find(|contract| contract.name == contract_name)
                .unwrap();
            let object = fixtures.pointer(fixture_name).unwrap().as_object().unwrap();
            let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
            let mut expected = contract
                .fields
                .iter()
                .map(|field| field.name)
                .collect::<Vec<_>>();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{fixture_name} field drift");
        }
    }
}
