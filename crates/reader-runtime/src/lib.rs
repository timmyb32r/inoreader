//! Process-local task ownership. Durable work remains in the owning repository.
use std::{future::Future, time::Duration};
use tokio::{sync::watch, task::JoinSet};

/// Cancellation closes admission. Callers finish their current durable operation
/// before observing it, so a shutdown does not silently replay paid requests.
#[derive(Clone)]
pub struct Shutdown(watch::Receiver<bool>);
impl Shutdown {
    pub fn requested(&self) -> bool {
        *self.0.borrow()
    }
    pub async fn wait(&mut self) {
        while !self.requested() && self.0.changed().await.is_ok() {}
    }
    pub async fn sleep(&mut self, duration: Duration) {
        tokio::select! { _ = self.wait() => {}, _ = tokio::time::sleep(duration) => {} }
    }
}

/// Owns every background task. Unexpected exit/panic is surfaced to the process;
/// shutdown is bounded by the caller's validated configuration, never detached.
pub struct TaskSupervisor {
    stop: watch::Sender<bool>,
    tasks: JoinSet<(&'static str, Result<(), String>)>,
}
impl Default for TaskSupervisor {
    fn default() -> Self {
        Self::new()
    }
}
impl TaskSupervisor {
    pub fn new() -> Self {
        let (stop, _) = watch::channel(false);
        Self {
            stop,
            tasks: JoinSet::new(),
        }
    }
    pub fn spawn<F, Fut>(&mut self, name: &'static str, task: F)
    where
        F: FnOnce(Shutdown) -> Fut,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let future = task(Shutdown(self.stop.subscribe()));
        self.tasks.spawn(async move { (name, future.await) });
    }
    pub async fn unexpected_exit(&mut self) -> String {
        match self.tasks.join_next().await {
            Some(Ok((name, Ok(())))) => format!("background task {name} exited unexpectedly"),
            Some(Ok((name, Err(error)))) => format!("background task {name} failed: {error}"),
            Some(Err(_)) => "background task panicked".into(),
            None => "no supervised tasks remain".into(),
        }
    }
    pub fn request_shutdown(&self) {
        self.stop.send_replace(true);
    }
    pub async fn drain(&mut self) -> Result<(), String> {
        let mut first_error = None;
        while let Some(result) = self.tasks.join_next().await {
            let error = match result {
                Ok((_, Ok(()))) => None,
                Ok((name, Err(e))) => Some(format!("{name}: {e}")),
                Err(_) => Some("background task panicked".into()),
            };
            if first_error.is_none() {
                first_error = error;
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
#[cfg(test)]
mod tests;

mod context;
pub use context::Context;
mod metrics;
pub use metrics::{observe, observe_sync, Stage};

pub async fn termination_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { result = tokio::signal::ctrl_c() => result, _ = terminate.recv() => Ok(()) }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
