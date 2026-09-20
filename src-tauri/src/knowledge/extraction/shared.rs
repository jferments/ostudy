//! The shared producer runs the existing decoder/checkpoint path. Subscribers
//! rehydrate from current durable state rather than trusting another caller's DTO.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Window<'a> {
    pub database: &'a Database,
    pub sources: &'a [SourceRecord],
    pub contract: &'a str,
    pub catalog: &'a EvidenceCatalog,
    pub authorization: &'a str,
    pub retry_partial: bool,
}

pub(super) async fn run<F, Fut>(
    input: Window<'_>,
    subscriber: &CancellationSignal,
    call: F,
) -> Result<HashMap<String, Vec<KnowledgeUnit>>, CommandError>
where
    F: FnMut(WindowCall) -> Fut,
    Fut: Future<Output = Result<Value, CommandError>>,
{
    let identity = hash(
        &serde_json::to_string(&(
            "shared-interpretation-v1",
            analysis_store::window_id(input.sources, input.contract)?,
            input.authorization,
            input.retry_partial,
        ))
        .map_err(CommandError::serialization)?,
    );
    super::super::shared_preparation::join_or_run(
        input.database,
        &identity,
        subscriber,
        |signal| async move {
            run_window(
                input.database,
                input.sources,
                input.contract,
                input.catalog,
                &signal,
                input.retry_partial,
                call,
            )
            .await
            .map(|_| ())
        },
    )
    .await?;
    check_canceled(subscriber)?;
    analysis_store::Checkpoint::open(input.database, input.sources, input.contract)?.units()
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, task::Poll};

    use super::*;

    #[tokio::test]
    async fn concurrent_windows_share_one_request_and_the_same_durable_units() {
        let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let contract = hash("shared receipt");
        let authorization = hash("same profile, scope and repair policy");
        let input = Window {
            database: &database,
            sources: &sources,
            contract: &contract,
            catalog: &catalog,
            authorization: &authorization,
            retry_partial: false,
        };
        let signal = CancellationSignal::new();
        let release = tokio::sync::Notify::new();
        let calls = Cell::new(0);
        let mut first = Box::pin(run(input, &signal, |call| {
            let release = &release;
            let calls = &calls;
            async move {
                calls.set(calls.get() + 1);
                release.notified().await;
                check_canceled(&call.cancellation)?;
                Ok(json!({"units":[{
                    "title":"Triangle","kind":"definition",
                    "statement":"A triangle has three sides.","qualifications":[],
                    "topic":"Geometry","importance":"core","evidenceIds":["E0"]
                }],"noLearningSourceIds":[]}))
            }
        }));
        std::future::poll_fn(|context| {
            assert!(first.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        let mut second = Box::pin(run(input, &signal, |_| async {
            panic!("the second consumer must not call the provider")
        }));
        std::future::poll_fn(|context| {
            assert!(second.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        release.notify_one();
        let (first, second) = tokio::join!(first, second);
        let first = first.unwrap();
        assert_eq!(first[&sources[0].id].len(), 1);
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(second.unwrap()).unwrap()
        );
        assert_eq!(calls.get(), 1);
    }
}
