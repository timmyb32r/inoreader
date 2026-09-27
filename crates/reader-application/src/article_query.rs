use crate::{ArticlePageCursor, ArticlePageDirection};
use reader_core::SubscriptionId;

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
        })
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
