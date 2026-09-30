# Reading digest: one subscription at a time

The second design replaces feed-batch grouping. Desktop uses a source navigation
column and wide cards for the selected subscription. Mobile uses a fixed-height
horizontally scrolling source strip. Feedback spans the card width underneath
the saved summary preview. Full summaries and focused discussion remain available.

Drafts survive source switches and reload. Source selection and pagination are
in the URL; returning from focused discussion retains the source. Saving blocks
source changes and duplicate submission. Saved cards remain until explicit refresh.

ArticleScope::SubscriptionUnread requires a subscription ID. Both repository
implementations filter membership and unread status before pagination. SQL uses
EXISTS to avoid duplicate rows from multiple origins. Workspace isolation remains
unchanged. This change requires no schema migration or paid AI generation.

Navigation counts reflect the subscription snapshot; content status reports the
current query total. Verification logs: /tmp/subscription-affected.log,
/tmp/subscription-release.log and /tmp/subscription-production.log.
