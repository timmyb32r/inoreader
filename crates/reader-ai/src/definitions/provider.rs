use super::*;
use crate::DeepSeekProvider;
use reader_web_runtime::{DnsResolver, ExternalRequestObserver, OutboundTransport};

impl<R, T, O> DeepSeekProvider<R, T, O>
where
    R: DnsResolver + 'static,
    T: OutboundTransport + 'static,
    O: ExternalRequestObserver + 'static,
{
    pub(crate) async fn define_entities(
        &self,
        key: &str,
        input: DefinitionsInput,
    ) -> Result<DefinitionReply, AiError> {
        let mut response = self
            .http
            .execute_stream(
                crate::provider::request("/chat/completions", key, Some(input.body()?))?,
                "deepseek",
                "article_definitions",
            )
            .await
            .map_err(|_| AiError::Provider)?;
        let mut body = Vec::new();
        let mut interrupted = false;
        loop {
            match response.next_chunk().await {
                Ok(Some(chunk)) => body.extend(chunk),
                Ok(None) => break,
                Err(_) => {
                    interrupted = true;
                    break;
                }
            }
        }
        Ok(DefinitionReply {
            status: response.status.as_u16(),
            body,
            interrupted,
        })
    }
}
