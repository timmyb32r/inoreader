use super::super::health::{HealthBody, RouteHealth};
use super::*;
use std::sync::Mutex;

#[test]
fn quarantine_is_per_host_shared_by_clones_and_recovery_probe_is_single_flight() {
    let start = Instant::now();
    let health = Arc::new(RouteHealth::new(
        ["a.test".into(), "b.test".into()].into_iter(),
        2,
        Duration::from_secs(60),
    ));
    let mut failed = health
        .claim("a.test", &[false, false], start)
        .unwrap()
        .unwrap();
    assert_eq!(failed.index, 0);
    failed.finish(false, start).unwrap();
    let clone = health.clone();
    let mut good = clone
        .claim("a.test", &[false, false], start)
        .unwrap()
        .unwrap();
    assert_eq!(good.index, 1);
    good.finish(true, start).unwrap();
    assert_eq!(
        health
            .claim("b.test", &[false, false], start)
            .unwrap()
            .unwrap()
            .index,
        0
    );
    assert_eq!(
        health
            .claim("a.test", &[false, false], start + Duration::from_secs(59))
            .unwrap()
            .unwrap()
            .index,
        1
    );

    let now = start + Duration::from_secs(60);
    let mut probe = health
        .claim("a.test", &[false, false], now)
        .unwrap()
        .unwrap();
    assert_eq!(probe.index, 0);
    assert!(probe.is_probe());
    let parallel = clone
        .claim("a.test", &[false, false], now)
        .unwrap()
        .unwrap();
    assert_eq!(parallel.index, 1);
    probe.finish(true, now).unwrap();
    let recovered = health
        .claim("a.test", &[false, false], now)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.index, 0);
    assert!(!recovered.is_probe());
}

#[test]
fn failed_probe_restarts_quarantine_and_cancellation_releases_eligibility() {
    let start = Instant::now();
    let health = Arc::new(RouteHealth::new(
        ["a.test".into()].into_iter(),
        1,
        Duration::from_secs(5),
    ));
    health
        .claim("a.test", &[false], start)
        .unwrap()
        .unwrap()
        .finish(false, start)
        .unwrap();
    assert!(health.claim("a.test", &[false], start).unwrap().is_none());
    let now = start + Duration::from_secs(5);
    let probe = health.claim("a.test", &[false], now).unwrap().unwrap();
    assert!(health.claim("a.test", &[false], now).unwrap().is_none());
    drop(probe);
    let mut retry = health.claim("a.test", &[false], now).unwrap().unwrap();
    assert!(retry.is_probe());
    retry.finish(false, now).unwrap();
    assert!(health
        .claim("a.test", &[false], now + Duration::from_secs(4))
        .unwrap()
        .is_none());
    assert!(health
        .claim("a.test", &[true], now + Duration::from_secs(5))
        .unwrap()
        .is_none());
}

#[test]
fn after_one_recovery_probe_healthy_endpoint_precedes_other_quarantines() {
    let start = Instant::now();
    let health = Arc::new(RouteHealth::new(
        ["a.test".into()].into_iter(),
        3,
        Duration::from_secs(5),
    ));
    for (attempted, success) in [
        ([false, false, false], false),
        ([true, false, false], false),
        ([true, true, false], true),
    ] {
        health
            .claim("a.test", &attempted, start)
            .unwrap()
            .unwrap()
            .finish(success, start)
            .unwrap();
    }
    let now = start + Duration::from_secs(5);
    let mut probe = health
        .claim("a.test", &[false, false, false], now)
        .unwrap()
        .unwrap();
    assert_eq!(probe.index, 0);
    probe.finish(false, now).unwrap();
    assert_eq!(
        health
            .claim("a.test", &[true, false, false], now)
            .unwrap()
            .unwrap()
            .index,
        2
    );
    assert!(health
        .claim("unknown.test", &[false, false, false], now)
        .is_err());
    assert!(health.claim("a.test", &[false], now).is_err());
}

#[derive(Default)]
struct Observer(Mutex<Vec<crate::ExternalRequestCompletion>>);
impl ExternalRequestObserver for Observer {
    fn completed(&self, value: crate::ExternalRequestCompletion) {
        self.0.lock().unwrap().push(value);
    }
}
struct ErrorBody;
#[async_trait]
impl ResponseBody for ErrorBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        Err(failure("fixture_body_error"))
    }
}
struct StallBody;
#[async_trait]
impl ResponseBody for StallBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn body_failure_and_attempt_timeout_quarantine_without_replay() {
    for inner in [
        Box::new(ErrorBody) as Box<dyn ResponseBody>,
        Box::new(StallBody),
    ] {
        let health = Arc::new(RouteHealth::new(
            ["a.test".into()].into_iter(),
            1,
            Duration::from_secs(60),
        ));
        let observer = Arc::new(Observer::default());
        let start = Instant::now();
        let mut body = HealthBody {
            body: Some(inner),
            attempt: health.claim("a.test", &[false], start).unwrap(),
            started: start,
            budget: Duration::from_millis(10),
            observer: Some(observer.clone()),
            origin_authoritative: false,
            finished: false,
        };
        assert!(body.next_chunk().await.is_err());
        assert!(body.body.is_none());
        assert!(health
            .claim("a.test", &[false], Instant::now())
            .unwrap()
            .is_none());
        assert!(body.next_chunk().await.unwrap().is_none());
        let events = observer.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].system, "public_proxy");
        assert_eq!(events[0].operation, "endpoint_quarantined");
        assert_eq!(events[0].outcome, ExternalRequestOutcome::Failed);
    }
}

#[tokio::test]
async fn authoritative_http_error_body_does_not_quarantine_or_rotate() {
    let health = Arc::new(RouteHealth::new(
        ["a.test".into()].into_iter(),
        1,
        Duration::from_secs(60),
    ));
    let start = Instant::now();
    let mut body = HealthBody {
        body: Some(Box::new(ErrorBody)),
        attempt: health.claim("a.test", &[false], start).unwrap(),
        started: start,
        budget: Duration::from_secs(1),
        observer: None,
        origin_authoritative: true,
        finished: false,
    };
    assert!(body.next_chunk().await.is_err());
    assert!(health
        .claim("a.test", &[false], Instant::now())
        .unwrap()
        .is_some());
}

#[test]
fn older_inflight_attempt_cannot_release_another_requests_recovery_probe() {
    let start = Instant::now();
    let health = Arc::new(RouteHealth::new(
        ["a.test".into()].into_iter(),
        1,
        Duration::from_secs(1),
    ));
    let mut first = health.claim("a.test", &[false], start).unwrap().unwrap();
    let mut older = health.claim("a.test", &[false], start).unwrap().unwrap();
    first.finish(false, start).unwrap();
    let now = start + Duration::from_secs(1);
    let mut probe = health.claim("a.test", &[false], now).unwrap().unwrap();
    older.finish(false, now).unwrap();
    assert!(health
        .claim("a.test", &[false], now + Duration::from_secs(1))
        .unwrap()
        .is_none());
    probe.finish(true, now + Duration::from_secs(1)).unwrap();
    assert!(!health
        .claim("a.test", &[false], now + Duration::from_secs(1))
        .unwrap()
        .unwrap()
        .is_probe());
}

#[test]
fn expired_quarantines_rotate_across_requests_with_one_attempt_budget() {
    let start = Instant::now();
    let health = Arc::new(RouteHealth::new(
        ["a.test".into()].into_iter(),
        3,
        Duration::from_secs(60),
    ));
    for i in 0..3 {
        let mut attempt = health.claim("a.test", &[false; 3], start).unwrap().unwrap();
        assert_eq!(attempt.index, i);
        attempt.finish(false, start).unwrap();
    }
    for round in 0..9 {
        let now = start + Duration::from_secs(120 * (round + 1));
        let mut attempt = health.claim("a.test", &[false; 3], now).unwrap().unwrap();
        assert_eq!(attempt.index, round as usize % 3);
        attempt.finish(false, now).unwrap();
    }
}
