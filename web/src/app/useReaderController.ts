import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { ApiClient, ArticlePage, Bootstrap } from "../api/client";
import type { Article, Subscription } from "../api/viewModels";
import {
  readArticlePagePosition,
  writeArticlePagePosition,
  type View,
} from "./readerLocation";

/** Account-owned reader data. Writes outlive page and workspace navigation.
 * A generation is replaced whenever a page/workspace changes.
 * Late reads, mutation failures and bulk rollbacks may only touch their original generation. */
export function useReaderController(
  client: ApiClient,
  bootstrap: Bootstrap,
  _subscriptions: Subscription[],
  announce: (message: string) => void,
  onOpenArticle: () => void,
) {
  const initialPagePosition = useRef(readArticlePagePosition()).current;
  const epoch = useRef(0),
    pageRequest = useRef(0),
    pageLock = useRef(false),
    bulkLock = useRef(false),
    pending = useRef(new Set<string>());
  useEffect(
    () => () => {
      epoch.current++;
      pageRequest.current++;
    },
    [],
  );
  const [pageGeneration, setPageGeneration] = useState(0);
  const [contentGeneration, setContentGeneration] = useState(0);
  const [view, setView] = useState<View>(initialPagePosition.view);
  const [articles, setArticles] = useState<Article[]>(
    bootstrap.articlePage.articles,
  );
  const [selectedId, setSelectedId] = useState(
    bootstrap.articlePage.articles[0]?.id ?? "",
  );
  const [selectedSubscriptionId, setSelectedSubscriptionId] = useState<
    string | null
  >(initialPagePosition.subscriptionId);
  const [workspaceId, setWorkspaceId] = useState(bootstrap.activeWorkspaceId);
  const [markingAll, setMarkingAll] = useState(false);
  const [pageTotal, setPageTotal] = useState(bootstrap.articlePage.total);
  const [unreadTotal, setUnreadTotal] = useState(
    bootstrap.articlePage.unreadTotal,
  );
  const [newerCursor, setNewerCursor] = useState<string | undefined>(
    bootstrap.articlePage.newerCursor ?? undefined,
  );
  const [olderCursor, setOlderCursor] = useState<string | undefined>(
    bootstrap.articlePage.olderCursor ?? undefined,
  );
  const [pageNumber, setPageNumber] = useState(initialPagePosition.batch);
  const [paging, setPaging] = useState(false);
  const [pendingArticleMutations, setPendingArticleMutations] = useState<
    Set<string>
  >(() => new Set());
  const articlesRef = useRef<Article[]>(bootstrap.articlePage.articles);
  const filteredRef = useRef<Article[]>([]);
  const selectedIdRef = useRef("");
  const selectedRef = useRef<Article>();
  const mutationGeneration = useRef(new Map<string, number>());
  const mutationQueue = useRef(new Map<string, Promise<void>>());
  // A page snapshot is publishable only after writes settle and no write began
  // during its request. Never discard an in-flight write to accept a stale GET.
  const writeVersion = useRef(0);
  const inFlightWrites = useRef(new Set<Promise<void>>());
  const waitForWrites = async () => {
    while (inFlightWrites.current.size)
      await Promise.all(inFlightWrites.current);
  };
  articlesRef.current = articles;
  // Keep the fetched batch in place as it is read/bookmarked. The next request
  // applies the view filter; removing rows under the pointer would shift targets.
  const filtered = useMemo(
    () =>
      articles.filter(
        (a) =>
          !selectedSubscriptionId ||
          a.subscriptionIds?.includes(selectedSubscriptionId),
      ),
    [articles, selectedSubscriptionId],
  );
  const selected = filtered.find((a) => a.id === selectedId) ?? filtered[0];
  filteredRef.current = filtered;
  selectedIdRef.current = selectedId;
  selectedRef.current = selected;
  useEffect(() => {
    if (!workspaceId || !selected) return;
    let active = true;
    const scope = epoch.current;
    let timer: number | undefined;
    let delay = selected.fullText === "pending" ? 1000 : 0;
    const poll = () => {
      const revision = mutationGeneration.current.get(selected.id);
      return client
        .getArticle(workspaceId, selected.id)
        .then((next) => {
          if (!active || scope !== epoch.current) return;
          if (
            revision !== mutationGeneration.current.get(selected.id) ||
            bulkLock.current ||
            [...pending.current].some((key) =>
              key.startsWith(`${selected.id}:`),
            )
          ) {
            timer = window.setTimeout(poll, 1000);
            return;
          }
          setArticles((current) =>
            current.map((article) => (article.id === next.id ? next : article)),
          );
          if (next.fullText !== "pending") return;
          delay = Math.min(Math.max(delay, 1000) * 2, 30000);
          timer = window.setTimeout(poll, delay);
        })
        .catch(() => {
          if (active && selected.fullText === "pending")
            timer = window.setTimeout(poll, 3000);
        });
    };
    timer = window.setTimeout(poll, delay);
    return () => {
      active = false;
      if (timer) window.clearTimeout(timer);
    };
  }, [client, workspaceId, selected?.id, pageGeneration, contentGeneration]);

  const refreshFullText = (id: string): Promise<void> => {
    const key = `${workspaceId}/${id}:fulltext`;
    const existing = mutationQueue.current.get(key);
    if (existing) return existing;
    const scope = epoch.current;
    writeVersion.current++;
    mutationGeneration.current.set(
      id,
      (mutationGeneration.current.get(id) ?? 0) + 1,
    );
    pending.current.add(`${id}:fulltext`);
    setPendingArticleMutations(new Set(pending.current));
    const request = client
      .refreshFullText(workspaceId, id)
      .then(() => {
        if (scope === epoch.current) setContentGeneration((value) => value + 1);
      })
      .finally(() => {
        mutationQueue.current.delete(key);
        if (scope === epoch.current) {
          pending.current.delete(`${id}:fulltext`);
          setPendingArticleMutations(new Set(pending.current));
        }
      });
    mutationQueue.current.set(key, request);
    const tracked = request.catch(() => {});
    inFlightWrites.current.add(tracked);
    void request
      .finally(() => inFlightWrites.current.delete(tracked))
      .catch(() => {});
    return request;
  };

  const update = (
    id: string,
    patch: Partial<Pick<Article, "read" | "later">>,
  ) => {
    const before = articlesRef.current.find((item) => item.id === id);
    if (!before || !workspaceId || bulkLock.current) return;
    const mutationKeys = Object.keys(patch).map((key) => `${id}:${key}`);
    if (mutationKeys.some((key) => pending.current.has(key))) return;
    writeVersion.current++;
    mutationKeys.forEach((key) => pending.current.add(key));
    setPendingArticleMutations(new Set(pending.current));
    const unreadDelta =
      patch.read === undefined || patch.read === before.read
        ? 0
        : patch.read
          ? -1
          : 1;
    setUnreadTotal((total) => total + unreadDelta);
    const requestWorkspace = workspaceId;
    const scope = epoch.current;
    const generation = (mutationGeneration.current.get(id) ?? 0) + 1;
    mutationGeneration.current.set(id, generation);
    setArticles((items) =>
      items.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    );
    const previous = mutationQueue.current.get(id) ?? Promise.resolve();
    const request = previous
      .catch(() => undefined)
      .then(() => client.updateArticle(requestWorkspace, id, patch))
      .then((saved) => {
        if (scope === epoch.current)
          setArticles((items) =>
            items.map((item) =>
              item.id === id
                ? {
                    ...item,
                    ...Object.fromEntries(
                      Object.keys(patch).map((key) => [
                        key,
                        saved[key as keyof typeof patch],
                      ]),
                    ),
                  }
                : item,
            ),
          );
      })
      .catch((error: Error) => {
        if (scope !== epoch.current) return;
        if (scope === epoch.current)
          setArticles((items) =>
            items.map((item) =>
              item.id === id
                ? {
                    ...item,
                    ...Object.fromEntries(
                      Object.keys(patch).map((key) => [
                        key,
                        before[key as keyof Article],
                      ]),
                    ),
                  }
                : item,
            ),
          );
        setUnreadTotal((total) => total - unreadDelta);
        announce(error.message);
      })
      .finally(() => {
        inFlightWrites.current.delete(request);
        if (scope !== epoch.current) return;
        mutationKeys.forEach((key) => pending.current.delete(key));
        setPendingArticleMutations((keys) => {
          const next = new Set(keys);
          mutationKeys.forEach((key) => next.delete(key));
          return next;
        });
        if (mutationQueue.current.get(id) === request)
          mutationQueue.current.delete(id);
      });
    mutationQueue.current.set(id, request);
    inFlightWrites.current.add(request);
  };
  const open = (id: string) => {
    setSelectedId(id);
    update(id, { read: true });
    onOpenArticle();
  };
  const applyPage = (page: ArticlePage, batch: number) => {
    epoch.current++;
    setPageGeneration((value) => value + 1);
    bulkLock.current = false;
    pending.current.clear();
    mutationQueue.current.clear();
    mutationGeneration.current.clear();
    setPendingArticleMutations(new Set());
    setMarkingAll(false);
    articlesRef.current = page.articles;
    setArticles(page.articles);
    setSelectedId(page.articles[0]?.id ?? "");
    setPageTotal(page.total);
    setUnreadTotal(page.unreadTotal);
    setNewerCursor(page.newerCursor ?? undefined);
    setOlderCursor(page.olderCursor ?? undefined);
    setPageNumber(batch);
  };
  const loadPage = async (
    nextView: View,
    subscriptionId: string | null,
    cursor?: string,
    direction?: "older" | "newer",
    batch = 1,
    articleId?: string,
  ) => {
    if (pageLock.current || !workspaceId) return;
    pageLock.current = true;
    setPaging(true);
    const scope = epoch.current,
      request = ++pageRequest.current;
    try {
      let page: ArticlePage;
      for (;;) {
        await waitForWrites();
        if (scope !== epoch.current) return;
        const version = writeVersion.current;
        page = await client.listArticles(
          workspaceId,
          nextView,
          subscriptionId ?? undefined,
          cursor,
          direction,
        );
        if (scope !== epoch.current) return;
        if (version === writeVersion.current) break;
      }
      if (scope !== epoch.current) return;
      applyPage(page, batch);
      setView(nextView);
      setSelectedSubscriptionId(subscriptionId);
      if (articleId && page.articles.some((a) => a.id === articleId)) {
        setSelectedId(articleId);
        onOpenArticle();
      }
      writeArticlePagePosition(
        nextView,
        subscriptionId,
        cursor,
        direction,
        batch,
      );
    } catch (e) {
      if (scope === epoch.current) announce((e as Error).message);
    } finally {
      if (request === pageRequest.current) {
        pageLock.current = false;
        setPaging(false);
      }
    }
  };
  const markAllRead = () => {
    if (bulkLock.current || pending.current.size || !workspaceId) return;
    const requestWorkspace = workspaceId;
    const scope = epoch.current;
    const affected = new Set(
      filtered.filter((article) => !article.read).map((article) => article.id),
    );
    if (affected.size === 0) return;
    writeVersion.current++;
    bulkLock.current = true;
    setPendingArticleMutations(
      new Set(articles.flatMap((a) => [`${a.id}:read`, `${a.id}:later`])),
    );
    setMarkingAll(true);
    const unreadBefore = unreadTotal;
    // Subscription counters can lag individual mutations; never subtract stale totals.
    if (view === "feed") setUnreadTotal(0);
    setArticles((items) =>
      items.map((article) =>
        affected.has(article.id) ? { ...article, read: true } : article,
      ),
    );
    const request = client
      .markAllRead(requestWorkspace, view, selectedSubscriptionId ?? undefined)
      .then(async () => {
        if (scope !== epoch.current) return;
        try {
          const refreshed = await client.listArticles(requestWorkspace, "feed");
          if (scope === epoch.current) setUnreadTotal(refreshed.unreadTotal);
        } catch (error) {
          if (scope === epoch.current)
            announce(
              `Articles marked read; count refresh failed: ${(error as Error).message}`,
            );
        }
      })
      .catch((error: Error) => {
        if (scope !== epoch.current) return;
        setUnreadTotal(unreadBefore);
        if (scope === epoch.current)
          setArticles((items) =>
            items.map((article) =>
              affected.has(article.id) ? { ...article, read: false } : article,
            ),
          );
        announce(error.message);
      })
      .finally(() => {
        inFlightWrites.current.delete(request);
        if (scope === epoch.current) {
          bulkLock.current = false;
          setPendingArticleMutations(new Set());
          setMarkingAll(false);
        }
      });
    inFlightWrites.current.add(request);
  };

  const replaceWorkspace = (id: string, page: ArticlePage) => {
    pageRequest.current++;
    pageLock.current = false;
    bulkLock.current = false;
    setPaging(false);
    setWorkspaceId(id);
    applyPage(page, 1);
    setView("feed");
    setSelectedSubscriptionId(null);
    writeArticlePagePosition("feed", null);
  };
  return {
    workspaceId,
    view,
    articles,
    selectedId,
    selectedSubscriptionId,
    filtered,
    selected,
    filteredRef,
    selectedRef,
    selectedIdRef,
    pageTotal,
    unreadTotal,
    newerCursor,
    olderCursor,
    pageNumber,
    paging,
    markingAll,
    pendingArticleMutations,
    open,
    update,
    loadPage,
    markAllRead,
    replaceWorkspace,
    waitForWrites,
    refreshFullText,
  };
}
