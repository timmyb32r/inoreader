//! Offline validation: the input envelope contains exact source text and a saved
//! ProviderReply. It accepts no credential and never constructs a network client.
use reader_ai::ProviderReply;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    source: String,
    reply: ProviderReply,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: replay_translation <audit-envelope.json>")?;
    let envelope: Envelope = serde_json::from_slice(&std::fs::read(path)?)?;
    let result = envelope.reply.translation_result(&envelope.source)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
