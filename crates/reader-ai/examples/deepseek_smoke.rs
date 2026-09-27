//! Research-only single-request harness. Running without explicit execution
//! arguments validates local inputs and never reads a key or opens a connection.
#[path = "smoke/mod.rs"]
mod smoke;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match smoke::run(std::env::args().skip(1).collect()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // Errors are static classifications, never raw I/O/provider errors.
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
