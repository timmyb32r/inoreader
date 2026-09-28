use crate::{parse_xml, FeedError, ParsedRecord};
use std::collections::HashMap;
use url::Url;

/// Explicit, opt-in RSS policy: retain the first occurrence of byte-identical
/// `<item>` elements with the same identity, preserving their original order.
/// Compare the complete original element, including unsupported metadata; never
/// infer equality from the smaller ParsedRecord projection. Formatting outside
/// an item is irrelevant. Any different repeated item rejects the entire poll.
/// Only UTF-8 RSS with one channel is supported; unsupported formats fail closed.
pub fn parse_rss_coalescing_identical(
    bytes: &[u8],
    feed_url: &Url,
) -> Result<(Vec<ParsedRecord>, usize), FeedError> {
    let invalid = |message: &str| FeedError::Invalid(message.to_owned());
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid("identical RSS policy requires UTF-8"))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|_| invalid("invalid XML for identical RSS policy"))?;
    let root = document.root_element();
    if !root.has_tag_name("rss") {
        return Err(invalid("identical RSS policy requires RSS"));
    }
    let channels: Vec<_> = root
        .children()
        .filter(|n| n.has_tag_name("channel"))
        .collect();
    if channels.len() != 1 {
        return Err(invalid("identical RSS policy requires exactly one channel"));
    }
    let items: Vec<_> = channels[0]
        .children()
        .filter(|n| n.has_tag_name("item"))
        .collect();
    let records = parse_xml(bytes, feed_url)?;
    if records.len() != items.len() {
        return Err(invalid("RSS item and parsed record counts differ"));
    }
    let mut seen = HashMap::new();
    let mut unique = Vec::with_capacity(records.len());
    let mut repeated = 0;
    for (record, item) in records.into_iter().zip(items) {
        let original = &text[item.range()];
        if let Some(previous) = seen.get(record.upstream_id.as_str()) {
            if *previous != original {
                return Err(invalid("conflicting repeated RSS identity"));
            }
            repeated += 1;
        } else {
            seen.insert(record.upstream_id.clone(), original);
            unique.push(record);
        }
    }
    Ok((unique, repeated))
}

#[cfg(test)]
#[path = "tests/identical_rss.rs"]
mod tests;
