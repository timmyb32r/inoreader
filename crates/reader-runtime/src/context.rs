//! Correlation identifiers are generated locally or taken from typed durable job
//! identities. Never accept arbitrary headers, URLs, names or provider text here.
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Context {
    pub request_id: Uuid,
    pub operation_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
}
tokio::task_local! {static CONTEXT: Context;}
impl Context {
    pub fn request() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            operation_id: None,
            job_id: None,
        }
    }
    pub fn operation(id: Uuid) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            operation_id: Some(id),
            job_id: Some(id),
        }
    }
    pub fn job(operation_id: Uuid, job_id: Uuid) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            operation_id: Some(operation_id),
            job_id: Some(job_id),
        }
    }
    pub async fn scope<T>(self, future: impl Future<Output = T>) -> T {
        CONTEXT.scope(self, future).await
    }
    pub fn current() -> Option<Self> {
        CONTEXT.try_with(|context| *context).ok()
    }
}
