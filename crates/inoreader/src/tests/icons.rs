use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn icon_page_bounds_admission_persists_immediately_and_drains_on_shutdown() {
    let started = Arc::new(AtomicUsize::new(0));
    let saved = Arc::new(AtomicUsize::new(0));
    let permits = Arc::new(tokio::sync::Semaphore::new(0));
    let mut supervisor = reader_runtime::TaskSupervisor::new();
    let (admitted, persisted, gate) = (started.clone(), saved.clone(), permits.clone());
    supervisor.spawn("icon fixture", move |stop| async move {
        run_page(
            (0..100).map(|index| (index.to_string(), vec![index.to_string()])),
            3,
            &stop,
            move |(_, ids)| {
                let admitted = admitted.clone();
                let gate = gate.clone();
                async move {
                    admitted.fetch_add(1, Ordering::SeqCst);
                    gate.acquire().await.unwrap().forget();
                    Ok((ids, "data:image/png;base64,fixture".into()))
                }
            },
            move |_| {
                let persisted = persisted.clone();
                async move {
                    persisted.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .map_err(|e| e.to_string())
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while started.load(Ordering::SeqCst) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(started.load(Ordering::SeqCst), 3);
    assert_eq!(saved.load(Ordering::SeqCst), 0);
    permits.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while started.load(Ordering::SeqCst) < 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        saved.load(Ordering::SeqCst),
        1,
        "persist before replacing completed slot while other requests are blocked"
    );
    supervisor.request_shutdown();
    permits.add_permits(3);
    supervisor.drain().await.unwrap();
    assert_eq!(started.load(Ordering::SeqCst), 4);
    assert_eq!(
        saved.load(Ordering::SeqCst),
        4,
        "accepted requests are persisted on shutdown"
    );
}
