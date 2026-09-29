import type { ReadPeriod } from "../api/client";
export type View = "feed" | "subscription" | "later";
type StoredArticlePagePosition = {
  view: View;
  articleId?: string;
  workspaceId?: string;
  readPeriod?: ReadPeriod;
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
    articleId: query.get("article") ?? undefined,
    workspaceId: query.get("workspace") ?? undefined,
    readPeriod:
      query.has("read_from") || query.has("read_until")
        ? {
            from: query.get("read_from") ?? "",
            until: query.get("read_until") ?? "",
          }
        : undefined,
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
  readPeriod?: ReadPeriod,
  articleId?: string,
  workspaceId?: string,
) {
  const query = new URLSearchParams();
  if (workspaceId) query.set("workspace", workspaceId);
  if (articleId) query.set("article", articleId);
  if (view !== "feed") query.set("view", view);
  if (subscriptionId) query.set("subscription", subscriptionId);
  if (readPeriod) {
    query.set("read_from", readPeriod.from);
    query.set("read_until", readPeriod.until);
  }
  if (cursor) query.set("cursor", cursor);
  if (direction) query.set("direction", direction);
  if (batch > 1) query.set("batch", String(batch));
  history.replaceState(
    history.state,
    "",
    `${window.location.pathname}${query.size ? `?${query}` : ""}`,
  );
  window.dispatchEvent(new Event("reader-location-written"));
}

/** Selecting another article adds one history entry; repeated activation does not. */
export function writeSelectedArticle(
  id: string,
  workspaceId: string,
  replace = false,
) {
  const url = new URL(location.href);
  if (
    url.searchParams.get("article") === id &&
    url.searchParams.get("workspace") === workspaceId
  )
    return;
  url.searchParams.set("workspace", workspaceId);
  url.searchParams.set("article", id);
  history[replace ? "replaceState" : "pushState"](
    history.state,
    "",
    url.pathname + url.search + url.hash,
  );
  window.dispatchEvent(new Event("reader-location-written"));
}
