use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[tokio::test]
async fn shutdown_finishes_admitted_work_and_wakes_idle_workers() {
    let mut tasks = TaskSupervisor::new();
    let completed = Arc::new(AtomicUsize::new(0));
    let output = completed.clone();
    let (admitted_tx, admitted_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    tasks.spawn("active", move |mut stop| async move {
        admitted_tx.send(()).unwrap();
        release_rx.await.unwrap(); // An admitted operation is not cancelled.
        output.fetch_add(1, Ordering::SeqCst);
        stop.sleep(Duration::from_secs(3600)).await;
        assert!(stop.requested());
        Ok(())
    });
    tasks.spawn("idle", |mut stop| async move {
        stop.wait().await;
        Ok(())
    });
    admitted_rx.await.unwrap();
    tasks.request_shutdown();
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), tasks.drain())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn task_panics_and_unexpected_completion_are_observable() {
    let mut tasks = TaskSupervisor::new();
    tasks.spawn("panic", |_| async { panic!("fixture") });
    assert!(tasks.unexpected_exit().await.contains("panicked"));
    tasks.spawn("early", |_| async { Ok(()) });
    assert!(tasks.unexpected_exit().await.contains("early"));
}

#[tokio::test]
async fn request_context_isolated_between_tasks_and_cleared_after_scope() {
    let a = Context::request();
    let b = Context::operation(uuid::Uuid::new_v4());
    let (left, right) = tokio::join!(
        a.scope(async {
            tokio::task::yield_now().await;
            Context::current().unwrap().request_id
        }),
        b.scope(async {
            tokio::task::yield_now().await;
            Context::current().unwrap().request_id
        })
    );
    assert_eq!(left, a.request_id);
    assert_eq!(right, b.request_id);
    assert_ne!(left, right);
    assert!(Context::current().is_none());
}

#[test]
fn stage_instrumentation_never_formats_provider_payloads() {
    struct Capture(std::sync::Mutex<Vec<String>>);
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
        fn flush(&self) {}
    }
    static CAPTURE: Capture = Capture(std::sync::Mutex::new(Vec::new()));
    log::set_logger(&CAPTURE).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    struct Secret;
    impl std::fmt::Display for Secret {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("must not expose error payload")
        }
    }
    impl std::fmt::Debug for Secret {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("must not expose error payload")
        }
    }
    let result: Result<(), Secret> = observe_sync(Stage::TranslationValidation, || Err(Secret));
    assert!(result.is_err());
    let records = CAPTURE.0.lock().unwrap();
    assert!(records
        .iter()
        .any(|r| r.contains("stage=translation_validation outcome=failed elapsed_us=")));
    assert!(records.iter().all(|r| !r.contains("payload")));
}
