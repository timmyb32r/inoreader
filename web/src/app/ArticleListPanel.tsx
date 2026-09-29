import { AutofillResistantField } from "../ui/fields";
import { localDay, readPeriodForDay } from "./readPeriod";
import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type { Subscription } from "../api/viewModels";
import type { useReaderController } from "./useReaderController";
import { ArticleRow, EmptyState } from "./ArticleRow";
import { Icon } from "../ui/Icon";

type Props = {
  reader: ReturnType<typeof useReaderController>;
  client: ApiClient;
  workspace: string;
  selectedSubscription: Subscription | null;
  archived: boolean;
  archiveReason?: string | null;
  mobilePanel: "nav" | "list" | "article";
  newCount: number;
  navigate: (path: string) => void;
  announce: (message: string) => void;
};
/** Owns list rendering and source refresh interaction. Reader data and writes
 * remain owned by the account controller; mounting this pane never resets them. */
export function ArticleListPanel({
  reader,
  client,
  workspace,
  selectedSubscription,
  archived,
  archiveReason,
  mobilePanel,
  newCount,
  navigate,
  announce,
}: Props) {
  const {
    view,
    readPeriod,
    unreadTotal,
    paging,
    loadPage,
    selectedSubscriptionId,
    markingAll,
    pendingArticleMutations,
    filtered,
    markAllRead,
    pageTotal,
    pageNumber,
    selected,
    open,
    update,
    newerCursor,
    olderCursor,
  } = reader;
  const [refreshingSubscription, setRefreshingSubscription] = useState(false);
  const refreshLock = useRef(false);
  const [showReadFilter, setShowReadFilter] = useState(!!readPeriod);
  useEffect(() => {
    if (readPeriod) setShowReadFilter(true);
  }, [readPeriod]);
  return (
    <section
      class={`article-list ${view === "feed" && (readPeriod || showReadFilter) ? "article-list--history" : ""} panel-mobile-${mobilePanel === "list" ? "show" : "hide"}`}
      aria-label="Article list"
    >
      <header class="list-header">
        <div class="list-header__title">
          {selectedSubscription ? (
            <a
              class="subscription-heading"
              href={`/subscriptions/${encodeURIComponent(selectedSubscription.id)}`}
              aria-label={`Open settings for ${selectedSubscription.name}`}
              onClick={(event) => {
                event.preventDefault();
                navigate(
                  `/subscriptions/${encodeURIComponent(selectedSubscription.id)}`,
                );
              }}
            >
              <p class="eyebrow">Subscription</p>
              <h1>{selectedSubscription.name}</h1>
            </a>
          ) : (
            <>
              <p class="eyebrow">{workspace}</p>
              <h1>
                {view === "feed"
                  ? readPeriod
                    ? "Feed"
                    : `Feed (${unreadTotal})`
                  : view === "subscription"
                    ? "Retained articles"
                    : "Read later"}
              </h1>
            </>
          )}
        </div>
        <div class="list-header__tools">
          {view === "feed" && !readPeriod && (
            <button
              class="text-button"
              disabled={paging || markingAll || unreadTotal === 0}
              onClick={() =>
                navigate(
                  `/reading?${new URLSearchParams({ workspace: reader.workspaceId, ...(selected && !selected.read ? { article: selected.id } : {}) })}`,
                )
              }
            >
              Reading mode
            </button>
          )}
          <button
            class="icon-button"
            aria-label="Refresh subscription"
            aria-busy={refreshingSubscription}
            disabled={!selectedSubscription || refreshingSubscription}
            title={
              selectedSubscription
                ? "Refresh this subscription"
                : "Choose a subscription to refresh"
            }
            onClick={() => {
              if (!selectedSubscription || refreshLock.current) return;
              refreshLock.current = true;
              setRefreshingSubscription(true);
              client
                .refreshSubscription(selectedSubscription.id)
                .then(() => announce("Refresh queued"))
                .catch((error: Error) => announce(error.message))
                .finally(() => {
                  refreshLock.current = false;
                  setRefreshingSubscription(false);
                });
            }}
          >
            {refreshingSubscription ? (
              <span class="spinner" />
            ) : (
              <Icon name="refresh" />
            )}
          </button>
        </div>
      </header>
      {view === "feed" && (readPeriod || showReadFilter) && (
        <div class="read-history-filter" aria-busy={paging}>
          <button class="text-button" onClick={() => navigate("/")}>
            ← Back to Home
          </button>
          <div class="read-history-filter__controls">
            <span>Marked read</span>
            <AutofillResistantField
              type="date"
              aria-label="Marked read on"
              value={readPeriod ? localDay(new Date(readPeriod.from)) : ""}
              disabled={paging}
              onChange={(event) => {
                const day = event.currentTarget.value;
                if (!day) return;
                void loadPage(
                  "feed",
                  null,
                  undefined,
                  undefined,
                  1,
                  undefined,
                  readPeriodForDay(day),
                );
              }}
            />
            <button
              class="icon-button"
              aria-label="Clear reading date filter"
              disabled={paging || !readPeriod}
              onClick={() => loadPage("feed", null)}
            >
              ×
            </button>
          </div>
          <span class="read-history-filter__status" role="status">
            {paging
              ? "Loading articles…"
              : readPeriod
                ? `${pageTotal} articles · ${Intl.DateTimeFormat().resolvedOptions().timeZone}`
                : "All unread articles"}
          </span>
        </div>
      )}
      {archived && (
        <div class="archive-strip">
          <Icon name="archive" />
          <span>
            This workspace is archived.{" "}
            {archiveReason ? `Reason: ${archiveReason}. ` : ""}
            Your library remains readable.
          </span>
        </div>
      )}
      {selectedSubscription?.status === "paused" && (
        <div class="archive-strip">
          <Icon name="pause" />
          <span>
            Paused
            {selectedSubscription.reason
              ? `: ${selectedSubscription.reason}`
              : ""}
            {selectedSubscription.reasonAt
              ? ` · ${new Date(selectedSubscription.reasonAt).toLocaleString("en-US")}`
              : ""}
          </span>
        </div>
      )}
      {newCount > 0 && (
        <button
          class="new-items"
          disabled={paging}
          aria-busy={paging}
          onClick={() =>
            loadPage(
              view,
              selectedSubscriptionId,
              undefined,
              undefined,
              1,
              undefined,
              readPeriod,
            )
          }
        >
          <span>
            {paging ? (
              <>
                <span class="spinner" />
                Loading…
              </>
            ) : (
              `${newCount} new articles`
            )}
          </span>
          <span>Show now</span>
        </button>
      )}
      <div class="list-controls">
        <button
          disabled={
            !!readPeriod ||
            markingAll ||
            pendingArticleMutations.size > 0 ||
            !filtered.some((article) => !article.read)
          }
          aria-busy={markingAll}
          onClick={markAllRead}
        >
          {markingAll ? (
            <span class="spinner" />
          ) : (
            <Icon name="check" size={15} />
          )}{" "}
          {markingAll ? "Marking…" : "Mark all read"}
        </button>
        <span>
          {pageTotal
            ? `${(pageNumber - 1) * 50 + 1}–${Math.min(pageNumber * 50, pageTotal)} of ${pageTotal}`
            : "0 articles"}
        </span>
        <span
          title={
            readPeriod
              ? "Sorted by the latest read event in this period"
              : "Sorted by the date first saved in this workspace"
          }
        >
          {readPeriod ? "Recently marked read" : "Newest saved first"}
        </span>
      </div>
      <div class="article-scroll">
        {filtered.length ? (
          filtered.map((article) => (
            <ArticleRow
              article={article}
              selected={article.id === selected?.id}
              laterPending={pendingArticleMutations.has(`${article.id}:later`)}
              onOpen={() => open(article.id)}
              onLater={() => update(article.id, { later: !article.later })}
            />
          ))
        ) : (
          <EmptyState readingHistory={!!readPeriod} />
        )}
      </div>
      <nav class="article-pager" aria-label="Article pages">
        <button
          disabled={!newerCursor || paging}
          aria-busy={paging}
          onClick={() =>
            newerCursor &&
            loadPage(
              view,
              selectedSubscriptionId,
              newerCursor,
              "newer",
              Math.max(1, pageNumber - 1),
              undefined,
              readPeriod,
            )
          }
        >
          ← Newer
        </button>
        <span>50 articles per request</span>
        <button
          disabled={!olderCursor || paging}
          aria-busy={paging}
          onClick={() =>
            olderCursor &&
            loadPage(
              view,
              selectedSubscriptionId,
              olderCursor,
              "older",
              pageNumber + 1,
              undefined,
              readPeriod,
            )
          }
        >
          Older →
        </button>
      </nav>
    </section>
  );
}
