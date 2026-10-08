use super::*;

impl BuiltInAdapterCollector {
    pub(super) async fn telegram(
        &self,
        source: &SourceDefinition,
        max_pages: usize,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let channel = source.url().path().trim_matches('/');
        if source.url().scheme() != "https"
            || source.url().host_str() != Some("t.me")
            || channel.is_empty()
            || !channel
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err(rejected("invalid_telegram_channel_url"));
        }
        let mut url = Url::parse(&format!("https://t.me/s/{channel}"))
            .map_err(|_| rejected("invalid_telegram_channel_url"))?;
        let mut records = Vec::new();
        let mut seen = HashSet::new();
        for _ in 0..max_pages {
            let body = self.request(Method::GET, url.clone(), None, None).await?;
            let text = std::str::from_utf8(&body).map_err(|_| rejected("telegram_invalid_utf8"))?;
            let (page, next) = parse_page(source, channel, text)?;
            for record in page {
                if !seen.insert(record.upstream_id().to_owned()) {
                    return Err(rejected("telegram_repeated_message_identity"));
                }
                records.push(record);
            }
            let Some(next) = next else {
                break;
            };
            let next = url
                .join(&next)
                .map_err(|_| rejected("invalid_telegram_history_cursor"))?;
            if next.scheme() != "https"
                || next.host_str() != Some("t.me")
                || next.path() != url.path()
                || next
                    .query_pairs()
                    .any(|(key, val)| key != "before" || val.parse::<u64>().is_err())
                || next.query().is_none()
            {
                return Err(rejected("invalid_telegram_history_cursor"));
            }
            url = next;
        }
        Ok(records)
    }
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static Telegram selector")
}
fn attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn parse_page(
    source: &SourceDefinition,
    channel: &str,
    text: &str,
) -> Result<(Vec<SourceRecord>, Option<String>), FetchError> {
    let html = Html::parse_document(text);
    if html
        .select(&selector(".tgme_channel_history"))
        .next()
        .is_none()
    {
        return Err(rejected("telegram_history_missing"));
    }
    let mut records = Vec::new();
    for card in html.select(&selector(".tgme_widget_message[data-post]")) {
        let identity = card
            .value()
            .attr("data-post")
            .ok_or_else(|| rejected("telegram_message_identity_missing"))?;
        let (actual_channel, id) = identity
            .split_once('/')
            .ok_or_else(|| rejected("telegram_invalid_message_identity"))?;
        if actual_channel != channel || id.parse::<u64>().is_err() {
            return Err(rejected("telegram_invalid_message_identity"));
        }
        let permalink = format!("https://t.me/{identity}");
        let body = card
            .select(&selector(".tgme_widget_message_bubble"))
            .next()
            .ok_or_else(|| rejected("telegram_message_body_missing"))?;
        let caption = card.select(&selector(".tgme_widget_message_text")).next();
        // Telegram posts have no authored title. Keep absence; caption remains body.
        let description = caption.map(|v| v.text().collect::<String>());
        let mut content = body.inner_html();
        for photo in card.select(&selector(".tgme_widget_message_photo_wrap")) {
            let style = photo
                .value()
                .attr("style")
                .ok_or_else(|| rejected("telegram_photo_missing_url"))?;
            let value = style
                .split_once("url(")
                .ok_or_else(|| rejected("telegram_photo_missing_url"))?
                .1
                .trim_start();
            let raw = if let Some(quote) = value.chars().next().filter(|v| matches!(v, '\'' | '"'))
            {
                let remainder = &value[quote.len_utf8()..];
                let (raw, ending) = remainder
                    .split_once(quote)
                    .ok_or_else(|| rejected("telegram_photo_missing_url"))?;
                if !ending.trim_start().starts_with(')') {
                    return Err(rejected("telegram_photo_missing_url"));
                }
                raw
            } else {
                value
                    .split_once(')')
                    .ok_or_else(|| rejected("telegram_photo_missing_url"))?
                    .0
                    .trim()
            };
            let image = Url::parse(raw).map_err(|_| rejected("telegram_invalid_media_url"))?;
            if image.scheme() != "https" {
                return Err(rejected("telegram_invalid_media_url"));
            }
            content.push_str(&format!("<img src=\"{}\" alt=\"\">", attribute(raw)));
        }
        // The shared safe renderer does not render audio/video tags. Preserve
        // their exact public media URLs as explicit links in the display body.
        for media in card.select(&selector(
            "video[src], audio[src], video source[src], audio source[src]",
        )) {
            let raw = media
                .value()
                .attr("src")
                .ok_or_else(|| rejected("telegram_media_missing_url"))?;
            let media_url = Url::parse(raw).map_err(|_| rejected("telegram_invalid_media_url"))?;
            if media_url.scheme() != "https" {
                return Err(rejected("telegram_invalid_media_url"));
            }
            content.push_str(&format!(
                "<p><a href=\"{}\">{}</a></p>",
                attribute(raw),
                attribute(raw)
            ));
        }
        let date = card
            .select(&selector(".tgme_widget_message_date time[datetime]"))
            .next()
            .and_then(|v| v.value().attr("datetime"))
            .ok_or_else(|| rejected("telegram_message_date_missing"))?;
        let published_at = reader_core::PublicationDate::parse(date)
            .ok_or_else(|| rejected("telegram_invalid_date"))?;
        let record = SourceRecord::from_parsed(
            reader_core::SourceRecordId::new(),
            source.id(),
            ParsedRecord {
                categories: None,
                upstream_id: permalink.clone(),
                original_url: permalink.clone(),
                absolute_url: Some(
                    Url::parse(&permalink).map_err(|_| rejected("telegram_invalid_message_url"))?,
                ),
                title: String::new(),
                description,
                description_media_type: Some("text/plain".into()),
                content_html: Some(content),
                published_at: Some(published_at),
            },
        )
        .map_err(|e| rejected(&e.to_string()))?;
        records.push(record);
    }
    let next = html
        .select(&selector("a.tme_messages_more[data-before]"))
        .next()
        .and_then(|v| v.value().attr("href"))
        .map(str::to_owned);
    Ok((records, next))
}

#[cfg(test)]
#[path = "../tests/telegram_adapter.rs"]
mod tests;
