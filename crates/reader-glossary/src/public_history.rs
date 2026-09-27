use crate::{GlossaryError, ObservedPost, PublicPage, CHANNEL_USERNAME};
use scraper::{Html, Selector};
use serde_json::json;

pub fn parse_public_page(raw: String, before: Option<i64>) -> Result<PublicPage, GlossaryError> {
    let doc = Html::parse_document(&raw);
    let selector = |s| Selector::parse(s).map_err(|_| GlossaryError::Configuration);
    let message = selector(".tgme_widget_message[data-post]")?;
    let text = selector(".tgme_widget_message_text")?;
    let date = selector("time[datetime]")?;
    let mut posts = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for node in doc.select(&message) {
        let key = node
            .value()
            .attr("data-post")
            .ok_or(GlossaryError::Protocol)?;
        let (channel, id) = key.split_once('/').ok_or(GlossaryError::Identity)?;
        if channel != CHANNEL_USERNAME {
            return Err(GlossaryError::Identity);
        }
        let id: i64 = id.parse().map_err(|_| GlossaryError::Identity)?;
        if before.is_some_and(|before| id >= before) {
            continue;
        }
        if !seen.insert(id) {
            return Err(GlossaryError::Conflict);
        }
        let formatted = node
            .select(&text)
            .next()
            .map(|v| v.inner_html())
            .unwrap_or_default();
        let date = node
            .select(&date)
            .next()
            .and_then(|v| v.value().attr("datetime"));
        let value = json!({"id":id,"channel":CHANNEL_USERNAME,"permalink":format!("https://t.me/{CHANNEL_USERNAME}/{id}"),"timestamp":date,"formatted_html":formatted,"raw_post_html":node.html(),"provenance":{"transport":"direct_public_preview"}});
        posts.push(ObservedPost::archive(value.to_string())?);
    }
    if posts.is_empty() {
        return Err(GlossaryError::Protocol);
    }
    let next = posts
        .iter()
        .map(ObservedPost::id)
        .min()
        .filter(|id| *id > 1);
    Ok(PublicPage {
        raw,
        posts,
        before: next,
    })
}
