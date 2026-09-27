//! Account-owned channel definitions. Raw observations are retained separately
//! from validated, replayable projections; exact names are never normalized.
mod config;
mod definition_parser;
mod model;
mod post;
mod public_history;
mod repository;
mod service;
mod telegram;

pub use config::*;
pub use definition_parser::*;
pub use model::*;
pub use post::*;
pub use public_history::*;
pub use repository::*;
pub use service::*;
pub use telegram::*;

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum GlossaryError {
    #[error("Invalid glossary configuration")]
    Configuration,
    #[error("Invalid channel identity")]
    Identity,
    #[error("Invalid text or formatting offsets")]
    Formatting,
    #[error("Unsupported channel formatting; original retained for replay")]
    Unsupported,
    #[error("Conflicting channel observation; original records retained")]
    Conflict,
    #[error("Glossary not found or access denied")]
    NotFound,
    #[error("Glossary storage unavailable")]
    Storage,
    #[error("Telegram request failed")]
    Transport,
    #[error("Telegram returned an invalid response")]
    Protocol,
    #[error("Telegram bot is not connected")]
    NotConnected,
    #[error("Telegram bot is already used by another receiver")]
    ReceiverConflict,
    #[error("Telegram retry required after {0} seconds")]
    RateLimited(u64),
    #[error("Glossary operation exceeded a configured limit")]
    Limit,
}

#[cfg(test)]
mod tests;
