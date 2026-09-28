//! Private, namespace-scoped wiki contracts. HTTP and persistence are adapters.
mod command;
mod model;
mod port;
pub use command::*;
pub use model::*;
pub use port::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Wiki object not found")]
    NotFound,
    #[error("This role cannot perform the action")]
    Forbidden,
    #[error("The page changed; compare your draft with the current revision")]
    Conflict,
    #[error("This name is already reserved by a page or alias")]
    NameTaken,
    #[error("Invalid wiki input: {0}")]
    Invalid(String),
    #[error("Wiki storage failed")]
    Storage,
}
#[cfg(test)]
mod tests;
