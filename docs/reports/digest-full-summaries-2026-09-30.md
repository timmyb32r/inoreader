# Digest readability

The source list hides zero-unread subscriptions and sorts remaining sources by
descending unread count. The initial selection follows that same order.

Cards render complete saved summaries inline. The clipped preview, its status
wording, separate modal and per-card refresh were removed; the existing selection
refresh reloads saved text. Four GET workers load the bounded page before showing
interactive cards, preventing asynchronous text growth from moving rating targets.
Errors remain explicit per article; browsing never requests AI generation.

Regression tests cover reversed input ordering, hidden zero counts, complete text
height on desktop/mobile, delayed summary loading, stable navigation and rating
targets, drafts, and request deduplication. Logs: /tmp/digest-full-browser.log,
/tmp/digest-full-affected.log, /tmp/digest-full-release.log.
