//! Valid-by-construction domain contracts. This crate has no infrastructure dependencies.

mod article;
mod identity;
mod jobs;
mod reason;
mod rules;
mod subscription;
mod workspace;

pub use article::*;
pub use identity::*;
pub use jobs::*;
pub use reason::*;
pub use rules::*;
pub use subscription::*;
pub use workspace::*;
