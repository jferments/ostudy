use super::*;

#[test]
fn narrative_style_is_repairable_without_becoming_a_hard_source_safety_failure() {
    let mut section = StudyGuideSectionContent {
        overview: "Enteric neurotransmitters / GI hormones / intrinsic nervous system".into(),
        key_points: vec!["Gastrin".into()],
        ..StudyGuideSectionContent::default()
    };
    assert!(validate_generated_safety(&section).is_ok());
    let error =
        validate_generated_writing_style(&section).expect_err("inventory should request repair");
    assert_eq!(error.code, "ai_document_output_style");

    section.overview =
        "Gastrointestinal function is coordinated by neural and hormonal signaling. Enteric and extrinsic pathways integrate motility, secretion, and feedback across the gut.\n\nThese pathways work together rather than as a disconnected inventory of mediators.".into();
    section.key_points.clear();
    assert!(validate_generated_safety(&section).is_ok());
    assert!(validate_generated_writing_style(&section).is_ok());
    assert!(inventory_like_prose("A / B / C"));
    assert!(!inventory_like_prose(&section.overview));
    assert!(!inventory_like_prose(
        "Metabolic syndrome links central adiposity; insulin resistance; dyslipidemia; and hypertension. These abnormalities reinforce cardiovascular risk."
    ));

    section.key_points = vec!["Gastrin".into()];
    assert!(validate_generated_safety(&section).is_ok());
    assert!(validate_generated_writing_style(&section).is_err());

    section.overview.clear();
    assert!(validate_generated_safety(&section).is_err());
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
            initial_visible_tokens: 4_096,
            repair_visible_tokens: 6_144,
        }
    );
    assert_eq!(
        COLLECTION_GUIDE_COMPLETION_PLANS[0],
        DOCUMENT_GUIDE_COMPLETION_PLAN
    );
    assert!(COLLECTION_GUIDE_COMPLETION_PLANS
        .iter()
        .all(|plan| plan.repair_visible_tokens > plan.initial_visible_tokens));
    assert!(COLLECTION_GUIDE_COMPLETION_PLANS
        .windows(2)
        .all(|pair| pair[0].repair_visible_tokens > pair[1].repair_visible_tokens));
}

#[test]
fn collection_prompt_asks_for_condensed_key_ideas_without_internal_diagnostics() {
    let prompt = collection_study_guide_system_prompt();
    assert!(prompt.contains("condensed summary of the collection's key ideas"));
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
    assert!(user.contains("source-aware output plan"));
    assert!(!user.contains("under 4,200 characters"));
    assert!(user.contains("use [] for optional categories"));
    assert!(validate_generated_writing_style(&StudyGuideSectionContent {
        overview: "Review preload.".to_string(),
        key_points: vec!["Review preload.".to_string()],
        ..Default::default()
    })
    .is_err());
    assert!(validate_generated_writing_style(&StudyGuideSectionContent {
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
fn prompt_asks_for_a_condensed_summary_of_key_ideas_with_lists_in_tables() {
    let prompt = study_guide_system_prompt();
    assert!(!prompt.contains("PDF"));
    assert!(prompt.contains("canonical"));
    assert!(prompt.to_ascii_lowercase().contains("study-guide author"));
    assert!(prompt.contains("condensed summary of the key ideas, not a rewrite"));
    assert!(prompt.contains("keyPoints: the key ideas"));
    assert!(prompt.contains("The length guide names how many"));
    assert!(prompt.contains("subjectA, subjectB, and relationship"));
    assert!(prompt.contains("workedExamples"));
    assert!(prompt.contains("retrievalChecks"));
    assert!(prompt.contains("Each idea appears once in the section"));
    assert!(prompt.contains("overview: the big idea in one sentence"));
    assert!(prompt.contains("put them in one table"));
    assert!(prompt.contains(&format!(
        "Study-guide prompt version: {STUDY_GUIDE_PROMPT_VERSION}"
    )));
}

#[test]
fn concise_generated_section_passes_native_style_validation() {
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
fn narrative_only_section_is_valid_and_schema_does_not_require_bullets() {
    let parsed = parse_study_guide_section(json!({
        "overview": "ACE inhibitors reduce angiotensin II formation and therefore decrease vasoconstriction and aldosterone-mediated sodium retention.\n\nTheir clinical value and adverse effects follow from the same renin–angiotensin–aldosterone pathway, so monitoring and contraindications should be understood as part of the mechanism.",
        "keyPoints": [],
        "explanations": [],
        "importantTerms": [],
        "processSteps": [],
        "comparisons": [],
        "commonMisconceptions": [],
        "visualExplanations": []
    }))
    .unwrap();
    assert!(parsed.key_points.is_empty());
    let schema = study_guide_section_schema();
    assert!(schema["properties"]["keyPoints"].get("minItems").is_none());
    assert_eq!(
        schema["properties"]["overview"]["maxLength"],
        MAX_OVERVIEW_CHARS
    );
}

#[test]
fn soft_style_target_does_not_reject_or_truncate_valid_content() {
    let overview = [
        "Preload reflects ventricular filling before systole and helps explain changes in stroke volume.";
        25
    ]
    .join(" ");
    assert!(overview.chars().count() > WritingBudget::from_signals(1, 0).overview_chars);
    assert!(overview.chars().count() < MAX_OVERVIEW_CHARS);
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

#[test]
fn ragged_tables_are_repaired_instead_of_failing_the_section() {
    let parsed = parse_study_guide_section(json!({
        "overview": "Minerals differ in their daily intake and in the problems a deficiency causes.",
        "keyPoints": [],
        "explanations": [],
        "importantTerms": [],
        "processSteps": [],
        "comparisons": [],
        "commonMisconceptions": [],
        "tables": [
            {"title": "Minerals", "columns": ["Mineral", "Intake", "Deficiency"], "caption": "",
                "rows": [["Calcium", "1000 mg"], ["Sodium", "1500 mg", "Rare", "cramps"], ["Iron", " ", "Anemia"]]},
            {"title": "One column", "columns": ["Only"], "rows": [["x"]], "caption": ""}
        ],
        "visualExplanations": []
    }))
    .unwrap();
    assert_eq!(parsed.tables.len(), 1);
    assert_eq!(
        parsed.tables[0].rows,
        vec![
            vec!["Calcium", "1000 mg", "—"],
            vec!["Sodium", "1500 mg", "Rare; cramps"],
            vec!["Iron", "—", "Anemia"],
        ]
    );
}

#[test]
fn repeated_knowledge_targets_are_kept_once() {
    let catalog = crate::knowledge::authoring::tests::catalog_fixture();
    let section = parse_grounded_section(
        json!({"content":{
            "overview":"A triangle is a polygon with three sides. Its three edges meet at three vertices.",
            "keyPoints":[],"explanations":[],"importantTerms":[],"processSteps":[],"comparisons":[],
            "commonMisconceptions":[],"visualExplanations":[]},
            "knowledgeUnitIds":["K0","K0"]}),
        &catalog,
    )
    .unwrap();
    assert_eq!(section.knowledge_support.len(), 1);
}

#[test]
fn catalog_subjects_keep_source_order_and_the_largest_past_the_limit() {
    let mut catalog = crate::knowledge::authoring::tests::catalog_fixture();
    let template = catalog.units[0].clone();
    let with_topic = |topic: &str| {
        let mut unit = template.clone();
        unit.content.topic = topic.to_string();
        unit
    };
    assert!(catalog_subjects(&catalog).is_empty());
    catalog.units = [
        "Swallowing",
        "swallowing",
        "Gastric emptying",
        " ",
        "Defecation",
    ]
    .into_iter()
    .map(with_topic)
    .collect();
    assert_eq!(
        catalog_subjects(&catalog),
        ["Swallowing (2)", "Gastric emptying (1)", "Defecation (1)"]
    );
    catalog.units = [
        "Food intake: leptin",
        "Food intake",
        "Food intake: ghrelin",
        "Motility: slow waves",
    ]
    .into_iter()
    .map(with_topic)
    .collect();
    assert_eq!(
        catalog_subjects(&catalog),
        ["Food intake (3)", "Motility: slow waves (1)"]
    );
    catalog.units = (0..MAX_CATALOG_SUBJECTS + 2)
        .map(|index| with_topic(&format!("S{index}")))
        .chain([with_topic("S5"), with_topic("S9")])
        .collect();
    let subjects = catalog_subjects(&catalog);
    assert_eq!(subjects.len(), MAX_CATALOG_SUBJECTS);
    assert_eq!(subjects[0], "S0 (1)");
    assert!(subjects.contains(&"S5 (2)".to_string()));
    assert!(subjects.contains(&"S9 (2)".to_string()));
    assert!(!subjects.contains(&"S25 (1)".to_string()));
}

#[test]
fn a_guide_length_scale_shortens_each_section_to_a_floor() {
    let budget = WritingBudget::from_signals(64, 64_000);
    assert_eq!(budget.target_chars, 3_000);
    assert_eq!(budget.scaled(1.0), budget);
    assert_eq!(budget.scaled(0.0), budget);
    assert_eq!(budget.scaled(f64::NAN), budget);
    let half = budget.scaled(0.5);
    assert_eq!(half.target_chars, 1_500);
    assert_eq!(half.overview_chars, 300);
    let tiny = budget.scaled(0.01);
    assert_eq!(tiny.target_chars, 512);
    assert_eq!(tiny.overview_chars, 150);
}
