//! A collection copy that release migration 6 merged into its original is the
//! same fact, so every analytics view counts the copy's reviews under the
//! original instead of losing them with the retired copy.

use super::*;
use crate::database::subjects_fixture::{advance_release_library, release_library, seed};

fn all_time() -> AnalyticsQueryDto {
    AnalyticsQueryDto {
        range: "all".to_string(),
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        custom_start_local_date: None,
        custom_end_local_date: None,
        highlighted_session_id: None,
    }
}

#[test]
fn merged_copy_reviews_count_under_their_originals() {
    let mut connection = release_library(5);
    let library = seed(&connection);
    advance_release_library(&connection, 5, 6);

    let analytics = load_analytics(&mut connection, all_time()).unwrap();
    // Twelve reviews of eight facts. Six of the reviews were of copies the
    // upgrade retired; they count under the originals.
    assert_eq!(analytics.history.review_count, 12);
    assert_eq!(analytics.history.cards_practiced, 8);
    assert_eq!(
        analytics
            .timeline
            .iter()
            .map(|bucket| bucket.review_count)
            .sum::<i64>(),
        12
    );
    assert_eq!(
        analytics
            .question_types
            .iter()
            .map(|kind| kind.review_count)
            .sum::<i64>(),
        12
    );

    let deck = |id: &str| {
        let stats = analytics
            .deck_stats
            .iter()
            .find(|deck| deck.deck_id == id)
            .unwrap();
        (stats.review_count, stats.cards_practiced)
    };
    // Three Mechanics cards, studied directly and through copies in both
    // bundles.
    assert_eq!(deck(&library.k_mech), (6, 3));
    // An Optics card studied only through its copy.
    assert_eq!(deck(&library.k_optics), (1, 1));
    let optics = analytics
        .dimension_options
        .decks
        .iter()
        .find(|deck| deck.id == library.k_optics)
        .unwrap();
    assert!(optics.has_history);

    let review_of = |card: &str| {
        library
            .reviews
            .iter()
            .find(|(reviewed, _)| reviewed == card)
            .map(|(_, review)| review.clone())
            .unwrap()
    };
    let copy_review = analytics
        .recent_reviews
        .iter()
        .find(|review| review.id == review_of(library.cb[0].card()))
        .unwrap();
    assert_eq!(copy_review.card_id, library.o_bio[0].card());
    // The edited copy stayed a card of its own, with its own review.
    let kept_review = analytics
        .recent_reviews
        .iter()
        .find(|review| review.id == review_of(library.cb[2].card()))
        .unwrap();
    assert_eq!(kept_review.card_id, library.cb[2].card());
}

#[test]
fn a_deck_filter_counts_merged_reviews_in_the_original_deck() {
    let mut connection = release_library(5);
    let library = seed(&connection);
    advance_release_library(&connection, 5, 6);

    let analytics = load_analytics(
        &mut connection,
        AnalyticsQueryDto {
            deck_ids: vec![library.k_optics.clone()],
            ..all_time()
        },
    )
    .unwrap();
    assert_eq!(analytics.history.review_count, 1);
    assert_eq!(analytics.history.cards_practiced, 1);

    // The copy was filed in Misc when it was reviewed; it retired into Optics.
    let misc = load_analytics(
        &mut connection,
        AnalyticsQueryDto {
            deck_ids: vec![library.k_misc.clone()],
            ..all_time()
        },
    )
    .unwrap();
    assert_eq!(misc.history.review_count, 0);
}

#[test]
fn a_dated_window_counts_merged_reviews_under_their_originals() {
    let mut connection = release_library(5);
    seed(&connection);
    advance_release_library(&connection, 5, 6);

    // A dated window over the whole library starts from the review dates, a
    // different join from the all-time and deck views above.
    let analytics = load_analytics(
        &mut connection,
        AnalyticsQueryDto {
            range: "custom".to_string(),
            custom_start_local_date: Some("2025-10-01".to_string()),
            custom_end_local_date: Some("2025-10-31".to_string()),
            ..all_time()
        },
    )
    .unwrap();
    assert_eq!(analytics.history.review_count, 12);
    assert_eq!(analytics.history.cards_practiced, 8);
    assert_eq!(
        analytics
            .question_types
            .iter()
            .map(|kind| kind.review_count)
            .sum::<i64>(),
        12
    );
}
