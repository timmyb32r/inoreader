//! Bounded health state: only configured host/endpoint pairs can exist. Recovery
//! probes use real requests after the configured quarantine, not background HTTP.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::failure;
use crate::{
    ExternalRequestCompletion, ExternalRequestObserver, ExternalRequestOutcome, ResponseBody,
    TransportError,
};

#[derive(Clone, Debug, Default)]
struct EndpointHealth {
    failed_at: Option<Instant>,
    probing: bool,
    healthy: bool,
}

#[derive(Debug)]
pub(super) struct RouteHealth {
    hosts: Mutex<HashMap<String, Vec<EndpointHealth>>>,
    quarantine: Duration,
}
impl RouteHealth {
    pub(super) fn new(
        hosts: impl Iterator<Item = String>,
        endpoints: usize,
        quarantine: Duration,
    ) -> Self {
        Self {
            hosts: Mutex::new(
                hosts
                    .map(|host| (host, vec![EndpointHealth::default(); endpoints]))
                    .collect(),
            ),
            quarantine,
        }
    }

    /// At most one caller claims an expired quarantine. Each request tries each
    /// configured endpoint once; after its first recovery probe, prefer a known
    /// healthy endpoint before spending time on further expired quarantines.
    pub(super) fn claim(
        self: &Arc<Self>,
        host: &str,
        attempted: &[bool],
        now: Instant,
    ) -> Result<Option<EndpointAttempt>, TransportError> {
        let mut hosts = self
            .hosts
            .lock()
            .map_err(|_| failure("proxy_health_state"))?;
        let states = hosts
            .get_mut(host)
            .ok_or_else(|| failure("proxy_health_host"))?;
        if attempted.len() != states.len() {
            return Err(failure("proxy_health_endpoints"));
        }
        let first = !attempted.iter().any(|value| *value);
        let index = states
            .iter()
            .enumerate()
            .filter(|(i, state)| {
                !attempted[*i]
                    && !state.probing
                    && state
                        .failed_at
                        .is_none_or(|at| now.duration_since(at) >= self.quarantine)
            })
            .min_by_key(|(i, state)| {
                let priority = match (state.failed_at.is_some(), state.healthy, first) {
                    (true, _, true) => 0,
                    (false, true, _) => 1,
                    (false, false, _) => 2,
                    (true, _, false) => 3,
                };
                (priority, *i)
            })
            .map(|(i, _)| i);
        let Some(index) = index else {
            return Ok(None);
        };
        let probe = states[index].failed_at.is_some();
        if probe {
            states[index].probing = true;
        }
        Ok(Some(EndpointAttempt {
            state: self.clone(),
            host: host.to_owned(),
            index,
            probe,
            finished: false,
        }))
    }
}

pub(super) struct EndpointAttempt {
    state: Arc<RouteHealth>,
    host: String,
    pub(super) index: usize,
    probe: bool,
    finished: bool,
}
impl EndpointAttempt {
    pub(super) fn is_probe(&self) -> bool {
        self.probe
    }
    pub(super) fn finish(&mut self, success: bool, now: Instant) -> Result<(), TransportError> {
        let mut hosts = self
            .state
            .hosts
            .lock()
            .map_err(|_| failure("proxy_health_state"))?;
        let state = &mut hosts
            .get_mut(&self.host)
            .ok_or_else(|| failure("proxy_health_host"))?[self.index];
        // A request started before quarantine may finish while a recovery probe
        // is active. Only that probe owns its single-flight reservation.
        if self.probe {
            state.probing = false;
        }
        state.healthy = success;
        state.failed_at = if success { None } else { Some(now) };
        self.finished = true;
        Ok(())
    }
}
impl Drop for EndpointAttempt {
    fn drop(&mut self) {
        if self.probe && !self.finished {
            // Cancellation is not evidence that a public endpoint failed.
            // Release single-flight eligibility; never leave a permanent lock.
            if let Ok(mut hosts) = self.state.hosts.lock() {
                if let Some(states) = hosts.get_mut(&self.host) {
                    states[self.index].probing = false;
                }
            }
        }
    }
}

pub(super) fn observe(
    observer: &Option<Arc<dyn ExternalRequestObserver>>,
    operation: &'static str,
    outcome: ExternalRequestOutcome,
    started: Instant,
) {
    if let Some(observer) = observer {
        observer.completed(ExternalRequestCompletion {
            system: "public_proxy",
            operation,
            outcome,
            elapsed: started.elapsed(),
        });
    }
}

/// Keeps the endpoint attempt's original deadline while streaming. A partial
/// body is an explicit failure and is never replaced with another endpoint's
/// bytes. Releasing/dropping a body cancels its underlying connection driver.
pub(super) struct HealthBody {
    pub(super) body: Option<Box<dyn ResponseBody>>,
    pub(super) attempt: Option<EndpointAttempt>,
    pub(super) started: Instant,
    pub(super) budget: Duration,
    pub(super) observer: Option<Arc<dyn ExternalRequestObserver>>,
    pub(super) origin_authoritative: bool,
    pub(super) finished: bool,
}
impl HealthBody {
    fn complete(&mut self, success: bool) -> Result<(), TransportError> {
        self.finished = true;
        self.body.take();
        let probe = self.attempt.as_ref().is_some_and(EndpointAttempt::is_probe);
        if let Some(mut attempt) = self.attempt.take() {
            // Received HTTP error statuses prove reachability and must not
            // trigger rotation, even if their optional response body fails.
            attempt.finish(success || self.origin_authoritative, Instant::now())?;
        }
        let operation = if probe {
            "quarantine_probe"
        } else if !success && !self.origin_authoritative {
            "endpoint_quarantined"
        } else {
            "endpoint_attempt"
        };
        observe(
            &self.observer,
            operation,
            if success {
                ExternalRequestOutcome::Success
            } else {
                ExternalRequestOutcome::Failed
            },
            self.started,
        );
        Ok(())
    }
}
#[async_trait::async_trait]
impl ResponseBody for HealthBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        if self.finished {
            return Ok(None);
        }
        let remaining = self.budget.checked_sub(self.started.elapsed());
        let result = match remaining {
            Some(remaining) => tokio::time::timeout(
                remaining,
                self.body
                    .as_mut()
                    .ok_or_else(|| failure("proxy_body_state"))?
                    .next_chunk(),
            )
            .await
            .map_err(|_| failure("proxy_endpoint_body_timeout"))
            .and_then(|result| result),
            None => Err(failure("proxy_endpoint_body_timeout")),
        };
        match &result {
            Ok(None) => self.complete(true)?,
            Err(_) => self.complete(false)?,
            Ok(Some(_)) => {}
        }
        result
    }
}
impl Drop for HealthBody {
    fn drop(&mut self) {
        if !self.finished {
            // Dropped redirects/errors may intentionally leave the body unread.
            // Their status remains authoritative; cancellations of successful
            // responses do not manufacture a transport failure/quarantine.
            if self.origin_authoritative {
                if let Some(mut attempt) = self.attempt.take() {
                    let _ = attempt.finish(true, Instant::now());
                }
            }
            observe(
                &self.observer,
                "endpoint_attempt",
                ExternalRequestOutcome::Failed,
                self.started,
            );
        }
    }
}
