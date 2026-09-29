// Complete synthetic wire fixtures with the fields emitted by the real server.
// The application decoder itself never supplies defaults or rewrites payloads.
export function wireFixture(value) {
  if (Array.isArray(value)) return value.map(wireFixture);
  if (!value || typeof value !== "object") return value;
  const result = Object.fromEntries(
    Object.entries(value).map(([k, v]) => [k, wireFixture(v)]),
  );
  if ("fullText" in result && "url" in result)
    return {
      sources: [],
      subscriptionIds: [],
      publishedAt: null,
      publicationStatus: "unknown",
      publicationSources: [],
      ...result,
    };
  if ("status" in result && "count" in result && "name" in result)
    return {
      sourceTitle: result.name,
      sourceUrl: "https://example.test/feed",
      sourceType: "feed",
      unreadCount: 0,
      consecutiveFailures: 0,
      needsAttention: false,
      incomplete: false,
      editableWebFeed: false,
      ...result,
    };
  return result;
}
