# Mark-unread semantics correction

The owner clarified that marking a read article unread must remove it from the
read-period selection on refresh and return it to the ordinary Feed. Both read
history queries and the Home aggregation now require the article's current read
flag. The raw event journal is preserved. No schema or data migration.

Deployed image: `sha256:c23d384452e704436a53317f5ae1004ea6e1e245afdca78b7143240fd0ba2520`.
`just check-release`, `just check-affected` and the final real PostgreSQL acceptance
test passed. Regression coverage checks exclusion after unmarking, stale-cursor
exclusion, return to ordinary Feed, matching dashboard counts and retained events.

Live read-only API/browser checks: today's filtered list and dashboard both showed
zero read articles; ordinary Feed contained 611 unread articles. Date reload and
stable clearing remained correct. Temporary smoke sessions were removed; tests
did not change production article states.
