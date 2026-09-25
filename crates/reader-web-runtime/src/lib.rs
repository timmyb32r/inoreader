//! Security boundary for every user-controlled web request.
//!
//! Callers cannot obtain an executable request without URL and address validation.
//! Transport adapters must disable their own redirect handling and connect to the
//! exact address carried by [`ConnectionAuthorization`].

mod browser;
mod config;
mod http_client;
mod network;
mod render;
mod reqwest_transport;

pub use browser::*;
pub use config::*;
pub use http_client::*;
pub use network::*;
pub use render::*;
pub use reqwest_transport::*;
