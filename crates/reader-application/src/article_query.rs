use crate::{ArticlePageCursor, ArticlePageDirection};
use chrono::{DateTime, Utc};
use reader_core::SubscriptionId;

/// Explicit half-open read-event interval [start, end), in UTC.
/// Constructed from the selected local calendar day, including DST boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadPeriod {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}
impl ReadPeriod {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, &'static str> {
        if !start.timestamp_subsec_nanos().is_multiple_of(1000)
            || !end.timestamp_subsec_nanos().is_multiple_of(1000)
        {
            return Err("read period supports microsecond precision");
        }
        if start >= end {
            return Err("read period must end after its start");
        }
        Ok(Self { start, end })
    }
    pub fn start(self) -> DateTime<Utc> {
        self.start
    }
    pub fn end(self) -> DateTime<Utc> {
        self.end
    }
}

/// Closed selection: a subscription id is required only for subscription history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArticleScope {
    Feed,
    Later,
    Subscription(SubscriptionId),
}
impl ArticleScope {
    pub fn from_wire(
        view: &str,
        subscription: Option<SubscriptionId>,
    ) -> Result<Self, &'static str> {
        match (view, subscription) {
            ("feed", None) => Ok(Self::Feed),
            ("later", None) => Ok(Self::Later),
            ("subscription", Some(id)) => Ok(Self::Subscription(id)),
            _ => Err("invalid article scope: subscription history requires a subscription; Feed and Read later are workspace views"),
        }
    }
    pub fn subscription(self) -> Option<SubscriptionId> {
        match self {
            Self::Subscription(id) => Some(id),
            _ => None,
        }
    }
}

/// Positive selection size with room for one lookahead row in PostgreSQL LIMIT.
/// This is a representation boundary; the caller supplies the product limit.
#[derive(Clone, Copy, Debug)]
pub struct SelectionLimit(usize);
impl SelectionLimit {
    pub fn new(value: usize) -> Result<Self, &'static str> {
        if value == 0
            || value
                .checked_add(1)
                .and_then(|v| i64::try_from(v).ok())
                .is_none()
        {
            return Err("selection limit must be positive and fit PostgreSQL LIMIT with lookahead");
        }
        Ok(Self(value))
    }
    pub fn get(self) -> usize {
        self.0
    }
    pub fn lookahead(self) -> i64 {
        (self.0 + 1) as i64
    }
}

/// Valid execution request. Newer traversal always has a cursor; no setters or
/// unchecked deserialization path can bypass construction. Cloning preserves it.
#[derive(Clone, Debug)]
pub struct ArticlePageRequest {
    scope: ArticleScope,
    cursor: Option<ArticlePageCursor>,
    direction: ArticlePageDirection,
    limit: SelectionLimit,
    read_period: Option<ReadPeriod>,
}
impl ArticlePageRequest {
    pub fn new(
        scope: ArticleScope,
        cursor: Option<ArticlePageCursor>,
        direction: ArticlePageDirection,
        limit: SelectionLimit,
    ) -> Result<Self, &'static str> {
        if direction == ArticlePageDirection::Newer && cursor.is_none() {
            return Err("newer direction requires a cursor");
        }
        Ok(Self {
            scope,
            cursor,
            direction,
            limit,
            read_period: None,
        })
    }
    pub fn with_read_period(mut self, period: Option<ReadPeriod>) -> Result<Self, &'static str> {
        if period.is_some() && self.scope != ArticleScope::Feed {
            return Err("read period requires Feed without a subscription");
        }
        self.read_period = period;
        Ok(self)
    }
    pub fn read_period(&self) -> Option<ReadPeriod> {
        self.read_period
    }
    pub fn scope(&self) -> ArticleScope {
        self.scope
    }
    pub fn cursor(&self) -> Option<&ArticlePageCursor> {
        self.cursor.as_ref()
    }
    pub fn direction(&self) -> ArticlePageDirection {
        self.direction
    }
    pub fn limit(&self) -> SelectionLimit {
        self.limit
    }
}

#[cfg(test)]
#[path = "tests/article_query.rs"]
mod tests;
