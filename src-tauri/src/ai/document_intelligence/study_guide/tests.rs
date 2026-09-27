use super::*;

#[test]
fn narrative_overview_rejects_keyword_inventory_and_bare_term_key_points() {
    let mut section = StudyGuideSectionContent {
        overview: "Enteric neurotransmitters / GI hormones / intrinsic nervous system".into(),
        key_points: vec!["Gastrin".into()],
        ..StudyGuideSectionContent::default()
    };
    let error = validate_generated_safety(&section).expect_err("inventory must fail");
    assert_eq!(error.code, "ai_document_output_invalid");

    section.overview =
        "Gastrointestinal function is coordinated by neural and hormonal signaling. Enteric and extrinsic pathways integrate motility, secretion, and feedback across the gut.".into();
    section.key_points = vec!["Gastrin promotes gastric acid secretion.".into()];
    assert!(validate_generated_safety(&section).is_ok());
    assert!(inventory_like_prose("A / B / C"));
    assert!(!inventory_like_prose(&section.overview));
}

#[test]
fn guide_schema_is_bare_and_strict_at_every_object_boundary() {
    let root = study_guide_section_schema();
    assert_eq!(root["type"], "object");
    assert!(root.get("name").is_none());
    assert!(root.get("strict").is_none());
    assert!(root.get("schema").is_none());
    assert_eq!(root["additionalProperties"], false);
    assert_eq!(
        root["properties"]["explanations"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        root["properties"]["comparisons"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        root["properties"]["comparisons"]["items"]["required"],
        json!(["subjectA", "subjectB", "relationship"])
    );
    assert_eq!(
        root["properties"]["tables"]["items"]["required"],
        json!(["title", "columns", "rows", "caption"])
    );
    assert_eq!(
        root["properties"]["callouts"]["items"]["additionalProperties"],
        false
    );
}

#[test]
fn collection_request_budget_can_reduce_catalog_without_changing_source_authority() {
    let knowledge = crate::knowledge::authoring::tests::catalog_fixture();
    let mut expanded = knowledge.clone();
    expanded.units = (0..24)
        .map(|index| {
            let mut unit = knowledge.units[0].clone();
            unit.id = format!("{:032x}", index + 1);
            unit.content.title = format!("Concept {index}");
            unit.content.statement = "Supported explanation. ".repeat(60);
            unit
        })
        .collect();
    let mut bounded = expanded.clone();
    while protocol::catalog_prompt(&bounded).unwrap().len() > 6_000 {
        bounded.units.pop();
    }
    assert!(!bounded.units.is_empty());
    assert!(bounded.units.len() < expanded.units.len());
    assert_eq!(
        bounded.units[0].content.statement,
        expanded.units[0].content.statement
    );
}

#[test]
fn collection_completion_plans_reserve_retry_headroom_before_input() {
    assert_eq!(
        DOCUMENT_GUIDE_COMPLETION_PLAN,
        StudyGuideCompletionPlan {
            initial_visible_tokens: 6_144,
            repair_visible_tokens: 12_288,
        }
    );
    assert_eq!(COLLECTION_GUIDE_COMPLETION_PLANS[0], DOCUMENT_GUIDE_COMPLETION_PLAN);
    assert!(COLLECTION_GUIDE_COMPLETION_PLANS
        .iter()
        .all(|plan| plan.repair_visible_tokens == plan.initial_visible_tokens * 2));
    assert!(COLLECTION_GUIDE_COMPLETION_PLANS
        .windows(2)
        .all(|pair| pair[0].repair_visible_tokens > pair[1].repair_visible_tokens));
}

#[test]
fn collection_prompt_requires_readable_narrative_without_internal_diagnostics() {
    let prompt = collection_study_guide_system_prompt();
    assert!(prompt.contains("readable teaching outline"));
    assert!(prompt.contains("Never reduce a section to 'Review <topic>'"));
    assert!(prompt.contains("Never") || prompt.contains("never"));
    let user = collection_study_guide_user_prompt(
        "Week 1",
        "Cardiac physiology",
        "• Preload — ventricular filling",
        "No additional validated relationship.",
        &crate::knowledge::authoring::tests::catalog_fixture(),
    )
    .unwrap();
    assert!(user.contains("Complete the required JSON object"));
    assert!(user.contains("under 4,200 characters"));
    assert!(user.contains("use [] for optional categories"));
    assert!(
        validate_collection_section_quality(&StudyGuideSectionContent {
            overview: "Review preload.".to_string(),
            key_points: vec!["Review preload.".to_string()],
            ..Default::default()
        })
        .is_err()
    );
    assert!(validate_collection_section_quality(&StudyGuideSectionContent {
        overview: "Preload describes ventricular filling before systole and helps connect venous return to subsequent stroke volume within the supported physiologic context.".to_string(),
        key_points: vec![
            "Venous return contributes to ventricular filling.".to_string(),
            "Filling changes can alter myocardial stretch.".to_string(),
        ],
        important_terms: vec![crate::document_study_guides::StudyGuideTerm {
            term: "Preload".to_string(),
            explanation: "Ventricular filling or stretch before contraction.".to_string(),
        }],
        ..Default::default()
    }).is_ok());
}

#[test]
fn prompt_requires_outline_style_and_prioritized_key_points() {
    let prompt = study_guide_system_prompt();
    assert!(!prompt.contains("PDF"));
    assert!(prompt.contains("canonical"));
    assert!(prompt.contains("study guide"));
    assert!(prompt.contains("narrative sentences"));
    assert!(prompt.contains("keyPoints are concise supported factual propositions"));
    assert!(prompt.contains("subjectA, subjectB, and relationship"));
    assert!(prompt.contains("workedExamples"));
    assert!(prompt.contains("retrievalChecks"));
    assert!(prompt.contains(&format!(
        "Study-guide prompt version: {STUDY_GUIDE_PROMPT_VERSION}"
    )));
}

#[test]
fn concise_generated_outline_passes_native_style_validation() {
    let parsed = parse_study_guide_section(json!({
        "overview": "Preload is ventricular filling/stretch before systole and influences stroke volume.",
        "keyPoints": [
            "Higher preload increases sarcomere stretch within the physiologic range.",
            "Greater stretch can increase contraction force through the Frank-Starling mechanism.",
            "Preload is influenced by venous return and ventricular filling."
        ],
        "explanations": [{
            "heading": "Frank-Starling mechanism",
            "body": "Increased end-diastolic filling stretches myocardial fibers, which can increase force of contraction and stroke volume within the normal operating range."
        }],
        "importantTerms": [{
            "term": "Preload",
            "explanation": "Ventricular wall stretch associated with end-diastolic filling."
        }],
        "processSteps": [],
        "comparisons": [],
        "commonMisconceptions": [],
        "visualExplanations": []
    }))
    .unwrap();
    assert_eq!(parsed.key_points.len(), 3);
}

#[test]
fn soft_style_target_does_not_reject_or_truncate_valid_content() {
    let overview = [
        "Preload reflects ventricular filling before systole and helps explain changes in stroke volume.";
        5
    ]
    .join(" ");
    assert!(overview.chars().count() > MAX_OVERVIEW_CHARS);
    let result = parse_study_guide_section(json!({
        "overview": overview,
        "keyPoints": ["Preload reflects ventricular filling before systole."],
        "explanations": [],
        "importantTerms": [],
        "processSteps": [],
        "comparisons": [],
        "commonMisconceptions": [],
        "visualExplanations": []
    }))
    .unwrap();
    assert_eq!(result.overview, overview);
}
#[test]
fn generated_guide_retains_exact_unit_selections_outside_reader_content() {
    let catalog = crate::knowledge::authoring::tests::catalog_fixture();
    let mut response = json!({"content":{
        "overview":"A triangle is a polygon with three sides. Its three edges meet at three vertices.","keyPoints":["Triangles have exactly three sides."],
        "explanations":[],"importantTerms":[],"processSteps":[],"comparisons":[],
        "commonMisconceptions":[],"visualExplanations":[]
    }, "knowledgeUnitIds":["a".repeat(32)]});
    let result = parse_grounded_section(response.clone(), &catalog).unwrap();
    assert_eq!(result.knowledge_support[0].unit_id, "a".repeat(32));
    assert_eq!(result.catalog_fingerprint, catalog.fingerprint().unwrap());
    assert!(!serde_json::to_string(&result.content)
        .unwrap()
        .contains("knowledge"));
    response["knowledgeUnitIds"] = json!(["f".repeat(32)]);
    assert!(parse_grounded_section(response, &catalog).is_err());
}
#[test]
fn guide_contract_handles_null_optionals_and_rejects_unseen_visuals() {
    let catalog = crate::knowledge::authoring::tests::catalog_fixture();
    let mut response = json!({"content":{"overview":"A triangle is a polygon with three sides. Its three edges meet at three vertices.","keyPoints":["Triangles have exactly three sides."],"explanations":null},"knowledgeUnitIds":["K0"]});
    assert!(parse_section_with_images(response.clone(), &catalog, false).is_ok());
    response["content"]["visualExplanations"] =
        json!([{"title":"Diagram","explanation":"A displayed triangle."}]);
    let text_only = parse_section_with_images(response.clone(), &catalog, false).unwrap();
    assert!(text_only.content.visual_explanations.is_empty());
    assert_eq!(
        text_only.content.overview,
        "A triangle is a polygon with three sides. Its three edges meet at three vertices."
    );
    assert_eq!(
        parse_section_with_images(response.clone(), &catalog, true)
            .unwrap()
            .content
            .visual_explanations
            .len(),
        1
    );
    response["knowledgeUnitIds"] = json!(["K999"]);
    assert!(parse_section_with_images(response, &catalog, false).is_err());
    assert_eq!(
        protocol::schema(&catalog, false)["properties"]["content"]["properties"]
            ["visualExplanations"]["maxItems"],
        0
    );
    assert_eq!(
        protocol::schema(&catalog, true)["properties"]["content"]["properties"]
            ["visualExplanations"]["maxItems"],
        MAX_VISUAL_EXPLANATIONS
    );
    assert!(guide_retryable(&CommandError::new(
        "ai_document_output_truncated",
        "limit"
    )));
    assert!(!guide_retryable(&CommandError::new(
        "ai_request_canceled",
        "cancel"
    )));
}
#[test]
fn guide_shape_errors_never_echo_provider_strings() {
    let error = parse_study_guide_section(
        json!({"overview":"A triangle.","keyPoints":["Three sides."],
        "explanations":[{"heading":"Mechanism","body":{"PRIVATE FIELD":"PRIVATE TEXT"}}]}),
    )
    .unwrap_err();
    assert!(error.message.contains("explanations/0/body"));
    assert!(!error.message.contains("PRIVATE FIELD"));
    assert!(!error.message.contains("PRIVATE TEXT"));
}
