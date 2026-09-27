use super::*;
use crate::{provider::request, DeepSeekProvider};
use reader_web_runtime::{DnsResolver, ExternalRequestObserver, OutboundTransport};

impl<R, T, O> DeepSeekProvider<R, T, O>
where
    R: DnsResolver + 'static,
    T: OutboundTransport + 'static,
    O: ExternalRequestObserver + 'static,
{
    pub(crate) async fn translate_paragraph(
        &self,
        key: &str,
        input: TranslationInput,
    ) -> Result<crate::ProviderReply, AiError> {
        let mut response = self
            .http
            .execute_stream(
                request("/chat/completions", key, Some(input.body))?,
                "deepseek",
                "paragraph_translation",
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
        Ok(crate::ProviderReply {
            status: response.status.as_u16(),
            body,
            interrupted,
        })
    }
}
