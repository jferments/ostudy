//! Keep sibling artifact futures on the heap and await both terminal results.
//! A failed guide must not drop an in-flight card publication, or vice versa.
use std::{future::Future, pin::Pin};

use crate::error::CommandError;

type ArtifactFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CommandError>> + Send + 'a>>;
type ArtifactPair<'a, C, G> =
    Pin<Box<dyn Future<Output = (Result<C, CommandError>, Result<G, CommandError>)> + Send + 'a>>;

pub(super) fn run<'a, C: Send + 'a, G: Send + 'a>(
    cards: ArtifactFuture<'a, C>,
    guide: ArtifactFuture<'a, G>,
) -> ArtifactPair<'a, C, G> {
    Box::pin(async move { tokio::join!(cards, guide) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::Poll,
    };

    #[tokio::test]
    async fn guide_failure_does_not_drop_unfinished_cards() {
        let entered = Arc::new(AtomicBool::new(false));
        let released = tokio::sync::Notify::new();
        let cards = Box::pin(async {
            entered.store(true, Ordering::SeqCst);
            released.notified().await;
            Ok::<_, CommandError>(7)
        });
        let guide = Box::pin(async { Err::<(), _>(CommandError::new("guide_failed", "fixture")) });
        let mut pair = run(cards, guide);
        assert_eq!(
            std::mem::size_of_val(&pair),
            2 * std::mem::size_of::<usize>()
        );
        std::future::poll_fn(|context| {
            assert!(pair.as_mut().poll(context).is_pending());
            assert!(entered.load(Ordering::SeqCst));
            Poll::Ready(())
        })
        .await;
        released.notify_one();
        let (cards, guide) = pair.await;
        assert_eq!(cards.unwrap(), 7);
        assert_eq!(guide.unwrap_err().code, "guide_failed");
    }

    #[tokio::test]
    async fn card_failure_does_not_short_circuit_guide_publication() {
        let released = tokio::sync::Notify::new();
        let entered = Arc::new(AtomicBool::new(false));
        let cards = Box::pin(async { Err::<(), _>(CommandError::new("cards_failed", "fixture")) });
        let guide = Box::pin(async {
            entered.store(true, Ordering::SeqCst);
            released.notified().await;
            Ok::<_, CommandError>(9)
        });
        let mut pair = run(cards, guide);
        std::future::poll_fn(|context| {
            assert!(pair.as_mut().poll(context).is_pending());
            assert!(entered.load(Ordering::SeqCst));
            Poll::Ready(())
        })
        .await;
        released.notify_one();
        let (cards, guide) = pair.await;
        assert_eq!(cards.unwrap_err().code, "cards_failed");
        assert_eq!(guide.unwrap(), 9);
    }
}
