use super::*;
use crate::{BuiltInAdapter, SourceKind};
use reader_collectors::ParsedRecord;
use reader_core::{PublicationDate, SourceRecordId};
use std::collections::HashSet;
impl Service {
    pub(super) async fn collect_inner(
        &self,
        source: &SourceDefinition,
        pages: NonZeroUsize,
    ) -> Result<Vec<SourceRecord>, Error> {
        if !matches!(
            source.kind(),
            SourceKind::BuiltIn(BuiltInAdapter::Zhihu { .. })
        ) {
            return Err(Error::Protocol);
        }
        let author = author(source.url()).ok_or(Error::Protocol)?;
        let owner = self.store.source_owner(source).await?;
        let cookies = self.cookies(owner).await?;
        let mut records = Vec::new();
        let mut identities = HashSet::new();
        let mut offset = 0usize;
        // This is an explicitly configured recent-history window, like Telegram.
        // Never follow provider-controlled paging URLs with credentials.
        for _ in 0..pages.get() {
            let value = self
                .request(
                    &cookies,
                    &format!("/api/v4/members/{author}/articles?limit=20&offset={offset}&include=data%5B%2A%5D.content"),
                )
                .await?;
            let rows = value
                .get("data")
                .and_then(|v| v.as_array())
                .ok_or(Error::Protocol)?;
            let end = value
                .pointer("/paging/is_end")
                .and_then(|v| v.as_bool())
                .ok_or(Error::Protocol)?;
            if rows.is_empty() && !end {
                return Err(Error::Protocol);
            }
            for row in rows {
                let id = row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()))
                    .ok_or(Error::Protocol)?
                    .to_owned();
                if !identities.insert(id.clone()) {
                    return Err(Error::Protocol);
                }
                let title = row
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or(Error::Protocol)?
                    .to_owned();
                let excerpt = row
                    .get("excerpt")
                    .and_then(|v| v.as_str())
                    .ok_or(Error::Protocol)?
                    .to_owned();
                let created = row
                    .get("created")
                    .and_then(|v| v.as_i64())
                    .ok_or(Error::Protocol)?;
                let date = chrono::DateTime::from_timestamp(created, 0).ok_or(Error::Protocol)?;
                // Only complete, public, non-paid bodies belong in the shared
                // source cache. Never silently save a gated excerpt as full text.
                if row.get("content_need_truncated").and_then(|v| v.as_bool()) != Some(false)
                    || row
                        .get("force_login_when_click_read_more")
                        .and_then(|v| v.as_bool())
                        != Some(false)
                    || !row
                        .get("paid_info")
                        .and_then(|v| v.as_object())
                        .ok_or(Error::Protocol)?
                        .is_empty()
                {
                    return Err(Error::RestrictedContent);
                }
                let content = row
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or(Error::Protocol)?
                    .to_owned();
                records.push(
                    SourceRecord::from_parsed(
                        SourceRecordId::new(),
                        source.id(),
                        ParsedRecord {
                            categories: None,
                            upstream_id: id.clone(),
                            original_url: format!("https://zhuanlan.zhihu.com/p/{id}"),
                            absolute_url: Some(
                                Url::parse(&format!("https://zhuanlan.zhihu.com/p/{id}"))
                                    .map_err(|_| Error::Protocol)?,
                            ),
                            title,
                            description: Some(excerpt),
                            description_media_type: Some("text/html".into()),
                            content_html: Some(content),
                            published_at: Some(
                                PublicationDate::try_from(date.to_rfc3339())
                                    .map_err(|_| Error::Protocol)?,
                            ),
                        },
                    )
                    .map_err(|_| Error::Protocol)?,
                );
            }
            if end {
                break;
            }
            offset = offset.checked_add(rows.len()).ok_or(Error::Protocol)?;
        }
        Ok(records)
    }
}
