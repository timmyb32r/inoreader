export type View = "feed" | "subscription" | "later";
type StoredArticlePagePosition = {
  view: View;
  subscriptionId: string | null;
  cursor?: string;
  direction?: "older" | "newer";
  batch: number;
};
export function readArticlePagePosition(): StoredArticlePagePosition {
  const query = new URLSearchParams(window.location.search);
  const requestedView = query.get("view");
  const view: View = query.has("subscription")
    ? "subscription"
    : requestedView === "later"
      ? "later"
      : "feed";
  const direction = query.get("direction");
  return {
    view,
    subscriptionId: query.get("subscription"),
    cursor: query.get("cursor") ?? undefined,
    direction:
      direction === "newer" || direction === "older" ? direction : undefined,
    batch: Math.max(1, Number.parseInt(query.get("batch") ?? "1", 10) || 1),
  };
}
export function writeArticlePagePosition(
  view: View,
  subscriptionId: string | null,
  cursor?: string,
  direction?: "older" | "newer",
  batch = 1,
) {
  const query = new URLSearchParams();
  if (view !== "feed") query.set("view", view);
  if (subscriptionId) query.set("subscription", subscriptionId);
  if (cursor) query.set("cursor", cursor);
  if (direction) query.set("direction", direction);
  if (batch > 1) query.set("batch", String(batch));
  history.replaceState(
    history.state,
    "",
    `${window.location.pathname}${query.size ? `?${query}` : ""}`,
  );
}
