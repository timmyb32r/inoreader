use std::{future::Future, sync::Arc, time::Duration};

use tokio::sync::watch;

use crate::{
    BrowserCollector, FeedFetcher, FullTextExtractor, IngestError, IngestStore, IngestWorker,
};

/// Runs a fixed number of workers. Shutdown closes admission immediately and
/// waits for the currently leased operation to finish; its heartbeat remains
/// active during that wait. There is no process-local work queue.
pub async fn run_until_shutdown<S, F, X, B, Q>(
    worker: Arc<IngestWorker<S, F, X, B>>,
    worker_count: usize,
    idle_poll: Duration,
    shutdown: Q,
) -> Result<(), String>
where
    S: IngestStore + 'static,
    F: FeedFetcher + 'static,
    X: FullTextExtractor + 'static,
    B: BrowserCollector + 'static,
    Q: Future<Output = ()>,
{
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..worker_count {
        let worker = worker.clone();
        let mut stop = stop_rx.clone();
        tasks.spawn(async move {
            let identity = format!("worker-{index}");
            loop {
                if *stop.borrow() { break; }
                match worker.run_one(&identity, chrono::Utc::now()).await {
                    Ok(true) => continue,
                    Ok(false) => {
                        tokio::select! {
                            _ = tokio::time::sleep(idle_poll) => {},
                            changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } }
                        }
                    }
                    Err(error) => {
                        eprintln!("ingest worker {identity} failed: {error}");
                        if !matches!(error, IngestError::Store(_)) {
                            continue;
                        }
                        tokio::select! {
                            _ = tokio::time::sleep(idle_poll) => {},
                            changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } }
                        }
                    }
                }
            }
        });
    }
    let failure = tokio::select! {
        _ = shutdown => None,
        _ = tasks.join_next() => Some("ingest worker exited unexpectedly".to_owned()),
    };
    stop_tx.send_replace(true);
    let mut failure = failure;
    while let Some(result) = tasks.join_next().await {
        if result.is_err() {
            failure.get_or_insert_with(|| "ingest worker panicked".to_owned());
        }
    }
    failure.map_or(Ok(()), Err)
}
