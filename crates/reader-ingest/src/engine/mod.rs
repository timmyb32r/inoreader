use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use reader_core::{ArticleId, SourceRecordId};
use reader_web_runtime::SafeRenderedContent;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    BrowserCapability, BrowserCollector, ContentChunk, ContentRevision, DeliveryCommit,
    FeedFetcher, FetchError, FullTextExtractor, IngestLimits, IngestStore, LeasedWork, PollAction,
    PollCommit, PolledRecord, RecordRevisionEffect, SourceKind, SourceRecord, StoreError, WorkItem,
};

#[derive(Debug, Error)]
pub enum IngestError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("feed parse failed: {0}")]
    Parse(String),
    #[error("browser-backed collection is temporarily unavailable")]
    BrowserDegraded,
    #[error("lease duration must be positive")]
    InvalidLeaseDuration,
    #[error("retry policy durations must be representable, positive delay and nonnegative jitter")]
    InvalidRetryPolicy,
}

pub struct IngestWorker<S, F, X, B> {
    store: Arc<S>,
    feeds: Arc<F>,
    fulltext: Arc<X>,
    browser: Arc<B>,
    limits: IngestLimits,
    lease_duration: Duration,
    renew_interval: Duration,
    retry_attempts: u32,
    rate_limit_delay: Duration,
    retry_jitter: Duration,
}

impl<S, F, X, B> IngestWorker<S, F, X, B>
where
    S: IngestStore,
    F: FeedFetcher,
    X: FullTextExtractor,
    B: BrowserCollector,
{
    pub fn new(
        store: Arc<S>,
        feeds: Arc<F>,
        fulltext: Arc<X>,
        browser: Arc<B>,
        limits: IngestLimits,
        lease_duration: Duration,
    ) -> Result<Self, IngestError> {
        if lease_duration <= Duration::zero() {
            return Err(IngestError::InvalidLeaseDuration);
        }
        Ok(Self {
            store,
            feeds,
            fulltext,
            browser,
            limits,
            lease_duration,
            renew_interval: lease_duration / 3,
            retry_attempts: u32::MAX,
            rate_limit_delay: Duration::zero(),
            retry_jitter: Duration::zero(),
        })
    }
    pub fn with_runtime_policy(
        mut self,
        renew_interval: Duration,
        retry_attempts: u32,
        rate_limit_delay: Duration,
        retry_jitter: Duration,
    ) -> Result<Self, IngestError> {
        if renew_interval <= Duration::zero()
            || renew_interval >= self.lease_duration
            || retry_attempts == 0
        {
            return Err(IngestError::InvalidLeaseDuration);
        }
        if rate_limit_delay <= Duration::zero()
            || retry_jitter < Duration::zero()
            || rate_limit_delay
                .max(Duration::seconds(1024))
                .checked_add(&retry_jitter)
                .and_then(|delay| Utc::now().checked_add_signed(delay))
                .is_none()
        {
            return Err(IngestError::InvalidRetryPolicy);
        }
        self.renew_interval = renew_interval;
        self.retry_attempts = retry_attempts;
        self.rate_limit_delay = rate_limit_delay;
        self.retry_jitter = retry_jitter;
        Ok(self)
    }

    /// Claims at most one durable job. Callers control concurrency and shutdown;
    /// there is deliberately no unbounded in-memory queue.
    pub async fn run_one(&self, worker: &str, now: DateTime<Utc>) -> Result<bool, IngestError> {
        let Some(lease) = self
            .store
            .claim(worker, now, now + self.lease_duration)
            .await?
        else {
            return Ok(false);
        };
        let result = self.handle_with_heartbeat(&lease).await;
        match result {
            Ok(()) => {
                self.store.complete(lease.job_id, lease.token).await?;
                Ok(true)
            }
            Err(error) => {
                let diagnostic = error.diagnostic();
                let not_before = match &error {
                    IngestError::Fetch(FetchError::RetryAfter { not_before, .. }) => {
                        Some(*not_before)
                    }
                    _ => None,
                };
                let rate_limited = matches!(
                    &error,
                    IngestError::Fetch(
                        FetchError::Http(429) | FetchError::RetryAfter { status: 429, .. }
                    )
                );
                let delay_seconds = 1_i64.checked_shl(lease.attempt.min(10)).unwrap_or(1024);
                let delay = Duration::seconds(delay_seconds).max(if rate_limited {
                    self.rate_limit_delay
                } else {
                    Duration::zero()
                });
                // UUIDs are already random. Use their exact low bits for a stable
                // per-job offset; no hash or source identity transformation.
                let spread = self.retry_jitter.num_milliseconds();
                let jitter = if spread > 0 {
                    Duration::milliseconds(
                        (lease.job_id.as_uuid().as_u128() % spread as u128) as i64,
                    )
                } else {
                    Duration::zero()
                };
                let retry_at = Utc::now()
                    .max(now)
                    .checked_add_signed(delay)
                    .and_then(|at| at.checked_add_signed(jitter))
                    .ok_or(IngestError::InvalidLeaseDuration)?
                    .max(not_before.unwrap_or(now));
                if lease.attempt.saturating_add(1) >= self.retry_attempts {
                    self.store
                        .fail(
                            lease.job_id,
                            lease.token,
                            &diagnostic,
                            if rate_limited {
                                Some(retry_at)
                            } else {
                                not_before
                            },
                        )
                        .await?;
                    return Err(error);
                }
                self.store
                    .retry(
                        lease.job_id,
                        lease.token,
                        // Backoff starts after the failed request, not its claim.
                        retry_at,
                        &diagnostic,
                    )
                    .await?;
                Err(error)
            }
        }
    }

    async fn handle_with_heartbeat(&self, lease: &LeasedWork) -> Result<(), IngestError> {
        let renew_every = self
            .renew_interval
            .to_std()
            .map_err(|_| IngestError::InvalidLeaseDuration)?;
        let start = tokio::time::Instant::now() + renew_every;
        let mut heartbeat = tokio::time::interval_at(start, renew_every);
        let work = self.handle(lease, Utc::now());
        // Commit operations hold the leased job row while publishing records.
        // Renewal can wait on that same row; keep polling the commit future
        // concurrently, otherwise renewal and commit deadlock each other.
        let renewals = async {
            loop {
                heartbeat.tick().await;
                if let Err(error) = self
                    .store
                    .renew(lease.job_id, lease.token, Utc::now() + self.lease_duration)
                    .await
                {
                    return Err::<(), IngestError>(error.into());
                }
            }
        };
        tokio::select! {
            result = work => result,
            result = renewals => result,
        }
    }

    pub async fn handle(&self, lease: &LeasedWork, now: DateTime<Utc>) -> Result<(), IngestError> {
        match &lease.item {
            WorkItem::PollSource { source_id } => self.poll(lease, *source_id, now).await,
            WorkItem::RefreshSource { source_id } => {
                let source = self.store.source(*source_id).await?;
                if matches!(
                    source.kind(),
                    SourceKind::WebPage(_) | SourceKind::BuiltIn(_)
                ) {
                    self.collect_web_feed(lease, *source_id, now).await
                } else {
                    self.poll(lease, *source_id, now).await
                }
            }
            WorkItem::FanOut {
                source_id,
                record_id,
                after_subscription,
            } => {
                self.fanout(lease, *source_id, *record_id, *after_subscription, now)
                    .await
            }
            WorkItem::ExtractFullText {
                record_id,
                source_revision,
                url,
                ..
            } => {
                self.fulltext(lease, *record_id, *source_revision, url, now)
                    .await
            }
            WorkItem::CollectWebFeed { source_id } => {
                self.collect_web_feed(lease, *source_id, now).await
            }
            WorkItem::CleanupContent {
                record_id,
                keep_refresh_id,
            } => self
                .store
                .cleanup_content(lease, *record_id, *keep_refresh_id)
                .await
                .map_err(Into::into),
            WorkItem::EvaluateArticleRules {
                workspace_id,
                article_id,
                evaluations,
            } => self
                .store
                .evaluate_article_rules(lease, *workspace_id, *article_id, evaluations)
                .await
                .map_err(Into::into),
            WorkItem::ApplyRule {
                workspace_id,
                rule_id,
                rule_version,
                after_article,
                through_article,
            } => {
                self.store
                    .apply_rule_batch(
                        lease,
                        *workspace_id,
                        *rule_id,
                        *rule_version,
                        *after_article,
                        *through_article,
                        self.limits.fanout_batch(),
                    )
                    .await?;
                Ok(())
            }
        }
    }

    async fn poll(
        &self,
        lease: &LeasedWork,
        source_id: reader_core::SourceId,
        now: DateTime<Utc>,
    ) -> Result<(), IngestError> {
        let started = std::time::Instant::now();
        // No active delivery means no new poll. Already received fulltext jobs are
        // independent and still finish.
        if self.store.active_delivery_count(source_id).await? == 0 {
            return Ok(());
        }
        if let Some(pending) = self
            .store
            .pending_poll(source_id, self.limits.fanout_batch())
            .await?
        {
            self.store.commit_poll(lease, pending).await?;
            return Ok(());
        }
        let source = self.store.source(source_id).await?;
        let validators = self.store.source_validators(source_id).await?;
        let page = self.feeds.fetch(source.url(), &validators).await?;
        if page.body.len() > self.limits.max_input_bytes() {
            return Err(IngestError::Parse(
                "feed exceeds configured max_input_bytes".into(),
            ));
        }
        if page.not_modified {
            self.store
                .record_source_success(
                    lease,
                    source_id,
                    now,
                    false,
                    started.elapsed().as_millis().try_into().map_err(|_| {
                        IngestError::Parse("poll duration exceeds u64 milliseconds".into())
                    })?,
                )
                .await?;
            return Ok(());
        }
        let parsed = match source.kind() {
            SourceKind::RssCoalesceIdentical => {
                reader_collectors::parse_rss_coalescing_identical(&page.body, &page.final_url).map(
                    |(records, repeated)| {
                        eprintln!(
                            "feed_identical_repeats source_id={} coalesced={repeated}",
                            source_id.as_uuid()
                        );
                        records
                    },
                )
            }
            SourceKind::XmlFeed => reader_collectors::parse_xml(&page.body, &page.final_url),
            SourceKind::JsonFeed => reader_collectors::parse_json(&page.body, &page.final_url),
            SourceKind::Auto
                if page
                    .content_type
                    .as_deref()
                    .is_some_and(|v| v.to_ascii_lowercase().contains("json"))
                    || page.body.iter().copied().find(|v| !v.is_ascii_whitespace())
                        == Some(b'{') =>
            {
                reader_collectors::parse_json(&page.body, &page.final_url)
            }
            SourceKind::Auto => reader_collectors::parse_xml(&page.body, &page.final_url),
            SourceKind::WebPage(_) | SourceKind::BuiltIn(_) => {
                return Err(IngestError::Parse(
                    "Web feed source must use CollectWebFeed".into(),
                ))
            }
        }
        .map_err(|error| IngestError::Parse(error.to_string()))?;
        let is_initial = !self.store.has_committed_poll(source_id).await?;
        let incomplete = is_initial && parsed.len() > self.limits.initial_feed_items();
        let mut records = Vec::new();
        let mut upstream_ids = std::collections::HashSet::new();
        for parsed in parsed {
            if !upstream_ids.insert(parsed.upstream_id.clone()) {
                return Err(IngestError::Parse(
                    "duplicate upstream identity in feed response".into(),
                ));
            }
            let value = match self
                .store
                .record_by_upstream(source_id, &parsed.upstream_id)
                .await?
            {
                Some(existing) => {
                    let revision = existing
                        .revise(parsed)
                        .map_err(|e| IngestError::Parse(e.to_string()))?;
                    match revision.effect {
                        RecordRevisionEffect::Unchanged => continue,
                        RecordRevisionEffect::RefreshContent => PolledRecord {
                            record: revision.record,
                            action: PollAction::RefreshContent,
                        },
                        RecordRevisionEffect::RegroupAndRefresh => PolledRecord {
                            record: revision.record,
                            action: PollAction::Regroup,
                        },
                    }
                }
                None => PolledRecord {
                    record: SourceRecord::from_parsed(SourceRecordId::new(), source_id, parsed)
                        .map_err(|e| IngestError::Parse(e.to_string()))?,
                    action: PollAction::Deliver,
                },
            };
            records.push(value);
        }
        let remainder = if is_initial && records.len() > self.limits.initial_feed_items() {
            records.split_off(self.limits.initial_feed_items())
        } else {
            Vec::new()
        };
        self.store
            .commit_poll(
                lease,
                PollCommit {
                    source_id,
                    source_revision: source.revision(),
                    records,
                    remainder,
                    fetched_at: now,
                    final_url: page.final_url,
                    validators: page.validators,
                    incomplete,
                    duration_ms: started.elapsed().as_millis().try_into().map_err(|_| {
                        IngestError::Parse("poll duration exceeds u64 milliseconds".into())
                    })?,
                },
            )
            .await?;
        Ok(())
    }

    async fn fanout(
        &self,
        lease: &LeasedWork,
        source_id: reader_core::SourceId,
        record_id: SourceRecordId,
        after: Option<reader_core::SubscriptionId>,
        now: DateTime<Utc>,
    ) -> Result<(), IngestError> {
        let record = self.store.record(record_id).await?;
        let targets = self
            .store
            .delivery_targets(source_id, after, self.limits.fanout_batch())
            .await?;
        let mut last = None;
        for target in targets {
            let subscription = target.subscription_id;
            self.store
                .deliver(
                    lease,
                    DeliveryCommit {
                        target,
                        record: record.clone(),
                        proposed_article_id: ArticleId::new(),
                        delivered_at: now,
                    },
                )
                .await?;
            last = Some(subscription);
        }
        // Cursor advancement is separately fenced and idempotent. A crash
        // between delivery and advance repeats each atomic delivery safely.
        if let Some(subscription) = last {
            self.store.advance_fanout(lease, subscription).await?;
        }
        Ok(())
    }

    async fn fulltext(
        &self,
        lease: &LeasedWork,
        record_id: SourceRecordId,
        source_revision: u64,
        url: &url::Url,
        now: DateTime<Utc>,
    ) -> Result<(), IngestError> {
        let result = async {
            let record = self.store.record(record_id).await?;
            let definition = self.store.source(record.source_id()).await?;
            let telegram = matches!(
                definition.kind(),
                SourceKind::BuiltIn(crate::BuiltInAdapter::Telegram { .. })
            );
            let page = if telegram {
                if record.revision() != source_revision {
                    return Err(IngestError::Parse(
                        "telegram_content_revision_changed".into(),
                    ));
                }
                let html = record
                    .feed_content_html()
                    .ok_or_else(|| IngestError::Parse("telegram_stored_body_missing".into()))?;
                crate::FetchedPage {
                    final_url: url.clone(),
                    content_type: Some("text/html; charset=utf-8".into()),
                    body: html.as_bytes().to_vec(),
                    validators: crate::CacheValidators::default(),
                    not_modified: false,
                }
            } else {
                self.fulltext.extract(url).await?
            };
            if page.body.len() > self.limits.max_input_bytes() {
                return Err(IngestError::Parse(
                    "fulltext response exceeds configured max_input_bytes".into(),
                ));
            }
            let raw = chunks(&page.body, self.limits.content_chunk_bytes());
            // Character-set decoding belongs at the fetch boundary. Until it
            // can prove a lossless decoding, invalid input fails visibly
            // instead of inserting replacement characters into user data.
            let source = crate::web_feed::decode_html(&page.body, page.content_type.as_deref())
                .map_err(|error| IngestError::Parse(error.to_string()))?;
            let publication = crate::extract_publication(&source, &page.final_url);
            let readable = if telegram {
                source
            } else {
                crate::web_feed::readable_fragment(&source)
            };
            if readable.len() > self.limits.max_extracted_bytes() {
                return Err(IngestError::Parse(
                    "extracted content exceeds configured max_extracted_bytes".into(),
                ));
            }
            let safe = SafeRenderedContent::from_untrusted_html(&readable);
            let safe = chunks(safe.html().as_bytes(), self.limits.content_chunk_bytes());
            self.store
                .publish_content(
                    lease,
                    ContentRevision {
                        publication,
                        record_id,
                        source_revision,
                        refresh_id: Uuid::new_v4(),
                        raw_chunks: raw,
                        safe_html_chunks: safe,
                        fetched_at: now,
                        final_url: page.final_url,
                    },
                )
                .await?;
            Ok(())
        }
        .await;
        if let Err(error) = &result {
            // Every failure after claiming the refresh is visible, including
            // decoding and configured-size failures. Recording it never clears
            // the previous complete manifest.
            let diagnostic = error.diagnostic();
            self.store
                .record_refresh_failure(lease, record_id, &diagnostic)
                .await?;
        }
        result
    }

    async fn collect_web_feed(
        &self,
        lease: &LeasedWork,
        source_id: reader_core::SourceId,
        now: DateTime<Utc>,
    ) -> Result<(), IngestError> {
        let started = std::time::Instant::now();
        if self.store.active_delivery_count(source_id).await? == 0 {
            return Ok(());
        }
        if let Some(pending) = self
            .store
            .pending_poll(source_id, self.limits.fanout_batch())
            .await?
        {
            self.store.commit_poll(lease, pending).await?;
            return Ok(());
        }
        if self.browser.capability() == BrowserCapability::Degraded {
            return Err(IngestError::BrowserDegraded);
        }
        let source = self.store.source(source_id).await?;
        let is_initial = !self.store.has_committed_poll(source_id).await?;
        let mut records = Vec::new();
        let collected = self
            .browser
            .collect(&source)
            .await
            .map_err(|error| match error {
                FetchError::Rejected(ref value) if value == "browser_degraded" => {
                    IngestError::BrowserDegraded
                }
                other => IngestError::Fetch(other),
            })?;
        if !is_initial && collected.is_empty() {
            return Err(IngestError::Parse("web_feed_empty_after_success".into()));
        }
        let incomplete = is_initial && collected.len() > self.limits.initial_feed_items();
        let mut upstream_ids = std::collections::HashSet::new();
        for proposed in collected {
            if !upstream_ids.insert(proposed.upstream_id().to_owned()) {
                return Err(IngestError::Parse(
                    "duplicate upstream identity in collection".into(),
                ));
            }
            let value = match self
                .store
                .record_by_upstream(source_id, proposed.upstream_id())
                .await?
            {
                Some(existing) => {
                    let revision = match source.kind() {
                        SourceKind::BuiltIn(crate::BuiltInAdapter::Dropbox) => {
                            existing.revise_from_dropbox_listing(proposed)
                        }
                        _ => existing.revise_from_record(proposed),
                    }
                    .map_err(|error| IngestError::Parse(error.to_string()))?;
                    match revision.effect {
                        RecordRevisionEffect::Unchanged => continue,
                        RecordRevisionEffect::RefreshContent => PolledRecord {
                            record: revision.record,
                            action: PollAction::RefreshContent,
                        },
                        RecordRevisionEffect::RegroupAndRefresh => PolledRecord {
                            record: revision.record,
                            action: PollAction::Regroup,
                        },
                    }
                }
                None => PolledRecord {
                    record: proposed,
                    action: PollAction::Deliver,
                },
            };
            records.push(value);
        }
        let remainder = if is_initial && records.len() > self.limits.initial_feed_items() {
            records.split_off(self.limits.initial_feed_items())
        } else {
            Vec::new()
        };
        self.store
            .commit_poll(
                lease,
                PollCommit {
                    source_id,
                    source_revision: source.revision(),
                    records,
                    remainder,
                    fetched_at: now,
                    final_url: source.url().clone(),
                    validators: Default::default(),
                    incomplete,
                    duration_ms: started.elapsed().as_millis().try_into().map_err(|_| {
                        IngestError::Parse("collection duration exceeds u64 milliseconds".into())
                    })?,
                },
            )
            .await?;
        Ok(())
    }
}

impl IngestError {
    fn diagnostic(&self) -> String {
        match self {
            Self::Store(error) => format!("storage: {error}"),
            Self::Fetch(FetchError::RetryAfter { status, not_before }) => {
                format!("remote_http_status: {status}; retry_after: {not_before}")
            }
            Self::Fetch(FetchError::Http(status)) => format!("remote_http_status: {status}"),
            Self::Fetch(FetchError::Rejected(reason)) => {
                format!("outbound_rejected: {reason}")
            }
            Self::Parse(value) if value == "web_feed_empty_after_success" => value.clone(),
            Self::Parse(reason) => format!("feed_parse: {reason}"),
            Self::BrowserDegraded => "browser_degraded".into(),
            Self::InvalidLeaseDuration => "invalid_lease_duration".into(),
            Self::InvalidRetryPolicy => "invalid_retry_policy".into(),
        }
    }
}

fn chunks(bytes: &[u8], size: usize) -> Vec<ContentChunk> {
    bytes
        .chunks(size)
        .enumerate()
        .map(|(ordinal, bytes)| ContentChunk {
            ordinal: ordinal as u32,
            bytes: bytes.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
