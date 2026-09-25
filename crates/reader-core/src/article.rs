use crate::{ArticleId, SourceId, SourceRecordId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ArticleLocation {
    /// Exact source spelling participates in identity; the parsed URL is used
    /// only for outbound I/O. URL parsing must not silently merge identifiers.
    Url { exact: String, fetch: Url },
    /// URL-less entries are only idempotent within their own source identity;
    /// they never merge merely because title/description happen to match.
    SourceRecord {
        source_id: SourceId,
        upstream_id: String,
    },
}

impl ArticleLocation {
    pub fn url(exact: String, fetch: Url) -> Self {
        Self::Url { exact, fetch }
    }
    pub fn exact_url(&self) -> Option<&str> {
        match self {
            Self::Url { exact, .. } => Some(exact),
            Self::SourceRecord { .. } => None,
        }
    }
    pub fn fetch_url(&self) -> Option<&Url> {
        match self {
            Self::Url { fetch, .. } => Some(fetch),
            Self::SourceRecord { .. } => None,
        }
    }
}
impl From<Url> for ArticleLocation {
    fn from(fetch: Url) -> Self {
        let exact = fetch.as_str().to_owned();
        Self::url(exact, fetch)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct DedupKey {
    pub location: ArticleLocation,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArticleState {
    pub read: bool,
    pub saved: bool,
    pub later: bool,
    pub trashed: bool,
    pub protect_unread: bool,
    pub protect_restored: bool,
}

impl ArticleState {
    pub fn merge(states: impl IntoIterator<Item = Self>) -> Option<Self> {
        let values: Vec<_> = states.into_iter().collect();
        if values.is_empty() {
            return None;
        }
        Some(Self {
            read: values.iter().all(|s| s.read),
            saved: values.iter().any(|s| s.saved),
            later: values.iter().any(|s| s.later),
            trashed: values.iter().all(|s| s.trashed),
            protect_unread: values.iter().any(|s| s.protect_unread),
            protect_restored: values.iter().any(|s| s.protect_restored),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub id: ArticleId,
    pub key: DedupKey,
    pub state: ArticleState,
    pub first_arrived_at: DateTime<Utc>,
    pub origins: Vec<SourceRecordId>,
    pub revision: u64,
}

impl Article {
    pub fn attach_origin(&mut self, origin: SourceRecordId) {
        if !self.origins.contains(&origin) {
            self.origins.push(origin);
            self.revision += 1;
        }
    }
    pub fn detach_origin(&mut self, origin: SourceRecordId) -> bool {
        if let Some(index) = self.origins.iter().position(|v| *v == origin) {
            self.origins.remove(index);
            self.revision += 1;
            true
        } else {
            false
        }
    }
    pub fn merge(id: ArticleId, mut articles: Vec<Self>) -> Option<Self> {
        let first = articles.first()?.key.clone();
        if articles.iter().any(|a| a.key != first) {
            return None;
        }
        let state = ArticleState::merge(articles.iter().map(|a| a.state))?;
        let first_arrived_at = articles.iter().map(|a| a.first_arrived_at).min()?;
        let mut origins = vec![];
        for origin in articles.drain(..).flat_map(|a| a.origins) {
            if !origins.contains(&origin) {
                origins.push(origin);
            }
        }
        Some(Self {
            id,
            key: first,
            state,
            first_arrived_at,
            origins,
            revision: 0,
        })
    }
    pub fn split_with_keys(
        &self,
        parts: impl IntoIterator<Item = (ArticleId, DedupKey, Vec<SourceRecordId>)>,
    ) -> Vec<Self> {
        parts
            .into_iter()
            .map(|(id, key, origins)| Self {
                id,
                key,
                state: self.state,
                first_arrived_at: self.first_arrived_at,
                origins,
                revision: 0,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
