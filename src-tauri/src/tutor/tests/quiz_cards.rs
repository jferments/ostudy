//! Quiz cards: a question's place in its run, choose-one questions graded
//! natively, and what met and missed each criterion.
use super::*;
use crate::{ai::AiRuntimeState, paths::AppPaths};

use super::super::contracts::{QuizOutcome, QuizSeries};

fn asked(
    fixture: &Fixture,
    request: &TutorTurnInput,
    value: serde_json::Value,
    previous: Option<&PrivateQuiz>,
) -> super::super::contracts::TutorTurn {
    let mut answer = output::validate(
        value,
        TutorMode::Quiz,
        &fixture.conversation.scope,
        &context(),
        None,
    )
    .unwrap();
    let quiz = answer.quiz.as_mut().unwrap();
    quiz.series = Some(QuizSeries::next(previous, quiz.question_count).unwrap());
    let identity = turns::begin(&fixture.database, request, &fixture.task).unwrap();
    turns::complete(&fixture.database, &identity, &answer, &context(), None).unwrap()
}

fn choose_one() -> serde_json::Value {
    json!({"text":"Which hormone?","sourceIds":[],"feedback":null,"quiz":{
        "question":"A patient's duodenal pH stays low after meals. Which hormone is most likely deficient?",
        "expectedAnswer":"secretin","criteria":["Names secretin."],
        "explanation":"Secretin brings the bicarbonate that neutralizes acid.",
        "sourceIds":[],"knowledgeUnitIds":[],
        "choices":["Gastrin","Secretin","Motilin","GIP"],"correctChoice":1,"questionCount":3}})
}

#[test]
fn a_run_counts_its_questions_and_ends_at_the_length_asked() {
    let first = QuizSeries::next(None, Some(2)).unwrap();
    assert_eq!(
        first,
        QuizSeries {
            number: 1,
            total: Some(2)
        }
    );
    let mut quiz: PrivateQuiz = serde_json::from_value(choose_one()["quiz"].clone()).unwrap();
    quiz.series = Some(first);
    let second = QuizSeries::next(Some(&quiz), Some(9)).unwrap();
    assert_eq!(
        second,
        QuizSeries {
            number: 2,
            total: Some(2)
        }
    );
    quiz.series = Some(second);
    assert!(QuizSeries::next(Some(&quiz), None).is_err());
    quiz.series = Some(QuizSeries {
        number: 7,
        total: None,
    });
    assert_eq!(QuizSeries::next(Some(&quiz), None).unwrap().number, 8);
    // A question saved before runs were counted starts its own.
    quiz.series = None;
    quiz.question_count = None;
    assert_eq!(
        QuizSeries::next(Some(&quiz), None).unwrap(),
        QuizSeries {
            number: 2,
            total: None
        }
    );
}

#[test]
fn a_choose_one_question_keeps_its_right_option_private_until_answered() {
    let f = fixture();
    let question = asked(&f, &input(&f, TutorMode::Quiz), choose_one(), None);
    let public = question.question.clone().unwrap();
    assert_eq!(public.number, 1);
    assert_eq!(public.total, Some(3));
    assert_eq!(public.choices.len(), 4);
    assert!(!public.answered);
    let json = serde_json::to_string(&question).unwrap();
    for private in ["correctChoice", "expectedAnswer", "bicarbonate"] {
        assert!(
            !json.contains(private),
            "{private} showed before the answer"
        );
    }
    // The right option is the expected answer, word for word.
    let fixed = f
        .database
        .with_connection(|c| turns::load_question(c, &f.conversation.id, &question.id))
        .unwrap();
    assert_eq!(fixed.expected_answer, "Secretin");
}

#[test]
fn the_model_cannot_place_a_question_or_offer_broken_options() {
    let f = fixture();
    let mut placed = choose_one();
    placed["quiz"]["series"] = json!({"number":4,"total":5});
    assert!(output::validate(
        placed,
        TutorMode::Quiz,
        &f.conversation.scope,
        &context(),
        None
    )
    .is_err());
    for (choices, correct) in [
        (json!(["Only one"]), json!(0)),
        (json!(["Gastrin", "Secretin"]), json!(2)),
        (json!(["Gastrin", "gastrin "]), json!(0)),
        (json!(["Gastrin", "Secretin"]), json!(null)),
        (json!([]), json!(0)),
    ] {
        let mut broken = choose_one();
        broken["quiz"]["choices"] = choices;
        broken["quiz"]["correctChoice"] = correct;
        assert!(output::validate(
            broken,
            TutorMode::Quiz,
            &f.conversation.scope,
            &context(),
            None
        )
        .is_err());
    }
    let mut endless = choose_one();
    endless["quiz"]["questionCount"] = json!(21);
    assert!(output::validate(
        endless,
        TutorMode::Quiz,
        &f.conversation.scope,
        &context(),
        None
    )
    .is_err());
}

#[tokio::test]
async fn a_choose_one_answer_is_graded_natively_and_reveals_the_right_option() {
    let f = fixture();
    let id = f
        .database
        .with_connection(|connection| generate_id(connection))
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("ostudy-quiz-cards-{}-{id}", std::process::id()));
    let paths = AppPaths::new(directory.join("library")).unwrap();
    // No provider can be reached: grading must not need one.
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let question = asked(&f, &input(&f, TutorMode::Quiz), choose_one(), None);
    let mut reply = input(&f, TutorMode::Answer);
    reply.profile_id = Some("a".repeat(32));
    reply.question_id = Some(question.id.clone());
    reply.text = "Motilin".into();
    reply.choice = Some(1);
    assert!(
        super::super::service::send(&f.database, &paths, &runtime, reply.clone())
            .await
            .is_err()
    );
    reply.choice = None;
    assert!(
        super::super::service::send(&f.database, &paths, &runtime, reply.clone())
            .await
            .is_err()
    );
    reply.choice = Some(0);
    reply.text = "Gastrin".into();
    let graded = super::super::service::send(&f.database, &paths, &runtime, reply)
        .await
        .unwrap();
    assert_eq!(graded.state, "complete");
    assert_eq!(graded.question_id.as_deref(), Some(question.id.as_str()));
    let feedback = graded.feedback.unwrap();
    assert_eq!(feedback.outcome, QuizOutcome::Incorrect);
    assert_eq!(feedback.chosen, Some(0));
    assert!(graded.assistant_text.contains("Expected answer: Secretin"));
    let revealed = f
        .database
        .with_connection(|c| turns::load_public(c, &question.id))
        .unwrap();
    assert!(!revealed.pending_question);
    let public = revealed.question.unwrap();
    assert!(public.answered);
    assert_eq!(public.correct_choice, Some(1));
    assert_eq!(public.expected_answer.as_deref(), Some("Secretin"));
    assert!(public.explanation.unwrap().contains("bicarbonate"));
    // The answer inherits what its question was written from, like any graded answer.
    assert_eq!(graded.total_tokens, None);
    std::fs::remove_dir_all(directory).ok();
}

#[tokio::test]
async fn next_question_takes_the_next_place_and_stops_after_the_last() {
    let f = fixture();
    let id = f
        .database
        .with_connection(|connection| generate_id(connection))
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("ostudy-quiz-next-{}-{id}", std::process::id()));
    let paths = AppPaths::new(directory.join("library")).unwrap();
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let mut two = choose_one();
    two["quiz"]["questionCount"] = json!(2);
    let first = asked(&f, &input(&f, TutorMode::Quiz), two.clone(), None);
    let fixed = f
        .database
        .with_connection(|c| turns::load_asked(c, &f.conversation.id, &first.id))
        .unwrap();
    let mut next = input(&f, TutorMode::Quiz);
    next.text = String::new();
    next.after_question_id = Some(first.id.clone());
    let second = asked(&f, &next, two, Some(&fixed));
    assert_eq!(
        second.question.as_ref().map(|q| (q.number, q.total)),
        Some((2, Some(2)))
    );
    let mut past = input(&f, TutorMode::Quiz);
    past.profile_id = Some("a".repeat(32));
    past.text = String::new();
    past.after_question_id = Some(second.id.clone());
    let error = super::super::service::send(&f.database, &paths, &runtime, past)
        .await
        .unwrap_err();
    assert!(error.message.contains("last question"));
    // Only a quiz question can follow one, and only one of this conversation.
    let mut stray = input(&f, TutorMode::Explain);
    stray.after_question_id = Some(first.id.clone());
    assert!(stray.validate().is_err());
    let mut unknown = input(&f, TutorMode::Quiz);
    unknown.after_question_id = Some("9".repeat(32));
    assert!(turns::begin(&f.database, &unknown, &f.task).is_err());
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn graded_answers_say_which_criteria_were_met() {
    let f = fixture();
    let mut short = choose_one();
    short["quiz"]["choices"] = json!([]);
    short["quiz"]["correctChoice"] = json!(null);
    short["quiz"]["criteria"] = json!(["Names the ductal cells.", "Names bicarbonate."]);
    let question = asked(&f, &input(&f, TutorMode::Quiz), short, None);
    let fixed = f
        .database
        .with_connection(|c| turns::load_question(c, &f.conversation.id, &question.id))
        .unwrap();
    let feedback = |met: serde_json::Value| {
        json!({"text":"x","sourceIds":[],"quiz":null,"feedback":{
        "outcome":"partial","explanation":"Half there.",
        "criteriaFeedback":["Ductal cells, yes.","Bicarbonate is missing."],"criteriaMet":met}})
    };
    assert!(output::validate(
        feedback(json!([true])),
        TutorMode::Answer,
        &f.conversation.scope,
        &context(),
        Some(&fixed)
    )
    .is_err());
    let graded = output::validate(
        feedback(json!([true, false])),
        TutorMode::Answer,
        &f.conversation.scope,
        &context(),
        Some(&fixed),
    )
    .unwrap();
    assert_eq!(graded.feedback.unwrap().criteria_met, vec![true, false]);
    // A choose-one question never goes to the model to be graded.
    let mc = asked(&f, &input(&f, TutorMode::Quiz), choose_one(), None);
    let fixed = f
        .database
        .with_connection(|c| turns::load_question(c, &f.conversation.id, &mc.id))
        .unwrap();
    assert!(output::validate(
        feedback(json!([true])),
        TutorMode::Answer,
        &f.conversation.scope,
        &context(),
        Some(&fixed)
    )
    .is_err());
}
