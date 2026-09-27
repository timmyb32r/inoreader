use std::{
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
/// Fixed stage vocabulary prevents account/article/URL cardinality and leakage.
#[derive(Clone, Copy)]
pub enum Stage {
    TranslationProvider,
    TranslationValidation,
    DefinitionsProvider,
    DefinitionsValidation,
    ChatProvider,
}
impl Stage {
    fn index(self) -> usize {
        self as usize
    }
    fn name(self) -> &'static str {
        match self {
            Self::TranslationProvider => "translation_provider",
            Self::TranslationValidation => "translation_validation",
            Self::DefinitionsProvider => "definitions_provider",
            Self::DefinitionsValidation => "definitions_validation",
            Self::ChatProvider => "chat_provider",
        }
    }
}
static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
static FAILURES: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
fn complete<T, E>(stage: Stage, started: Instant, result: &Result<T, E>) {
    let count = COUNTS[stage.index()].fetch_add(1, Ordering::Relaxed) + 1;
    if result.is_err() {
        FAILURES[stage.index()].fetch_add(1, Ordering::Relaxed);
    }
    log::info!(target:"reader_stage","stage={} outcome={} elapsed_us={} count={} failures={}",stage.name(),if result.is_ok(){"ok"}else{"failed"},started.elapsed().as_micros(),count,FAILURES[stage.index()].load(Ordering::Relaxed));
}
pub async fn observe<T, E>(
    stage: Stage,
    future: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    let started = Instant::now();
    let result = future.await;
    complete(stage, started, &result);
    result
}
pub fn observe_sync<T, E>(stage: Stage, run: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let started = Instant::now();
    let result = run();
    complete(stage, started, &result);
    result
}
