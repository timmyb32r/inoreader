import { ReadingBackButton } from "./ReadingBackButton";
import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient, ArticlePage } from "../../api/client";
import type { Subscription } from "../../api/viewModels";
import { loadDigestSummaries, type DigestSummary } from "./digestSummaries";
import { DigestArticle } from "./DigestArticle";
import "./reading-digest.css";

type Position = {
  subscription?: string;
  cursor?: string;
  direction?: "older" | "newer";
};
/** One subscription per selection; pagination is scoped to its unread articles. */
export function ReadingDigest({
  client,
  owner,
  workspace,
  subscriptions,
  navigate,
  onExit,
}: {
  client: ApiClient;
  owner: string;
  workspace: string;
  subscriptions: Subscription[];
  navigate: (path: string) => void;
  onExit: () => void;
}) {
  const sources = subscriptions
    .filter((s) => (s.unreadCount ?? 0) > 0)
    .sort((a, b) => b.unreadCount! - a.unreadCount!);
  const positionFromUrl = (): Position => {
    const q = new URLSearchParams(location.search);
    return {
      subscription: q.get("subscription") ?? sources[0]?.id,
      cursor: q.get("cursor") ?? undefined,
      direction: q.get("direction") === "newer" ? "newer" : "older",
    };
  };
  const [position, setPosition] = useState(positionFromUrl);
  const [page, setPage] = useState<ArticlePage | null>(null);
  const [summaries, setSummaries] = useState<Record<string, DigestSummary>>({});
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  const [saving, setSaving] = useState(new Set<string>());
  const pageLock = useRef(false),
    savingRef = useRef(new Set<string>());
  const scroll = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const restore = () => setPosition(positionFromUrl());
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);
  useEffect(() => {
    let active = true;
    setBusy(true);
    setError("");
    pageLock.current = true;
    if (!position.subscription) {
      setPage(null);
      setBusy(false);
      pageLock.current = false;
      return;
    }
    const canonical = new URL(location.href);
    if (!canonical.searchParams.get("subscription")) {
      canonical.searchParams.set("subscription", position.subscription);
      history.replaceState(history.state, "", canonical);
    }
    void client
      .listArticles(
        workspace,
        "subscription-unread",
        position.subscription,
        position.cursor,
        position.direction,
      )
      .then(async (result) => {
        const loaded = await loadDigestSummaries(
          client,
          workspace,
          result.articles,
          () => active,
        );
        if (active) {
          setSummaries(loaded);
          setPage(result);
          scroll.current?.scrollTo(0, 0);
        }
      })
      .catch((cause: Error) => {
        if (active) setError(cause.message);
      })
      .finally(() => {
        if (active) {
          setBusy(false);
          pageLock.current = false;
        }
      });
    return () => {
      active = false;
    };
  }, [client, workspace, position, reload]);
  const source = subscriptions.find((s) => s.id === position.subscription);
  const turn = (next: Position) => {
    if (pageLock.current || savingRef.current.size) return;
    pageLock.current = true;
    setBusy(true);
    setError("");
    const q = new URLSearchParams({ workspace });
    next = {
      ...next,
      subscription: next.subscription ?? position.subscription,
    };
    if (next.subscription) q.set("subscription", next.subscription);
    if (next.cursor) {
      q.set("cursor", next.cursor);
      q.set("direction", next.direction ?? "older");
    }
    history.pushState(history.state, "", `/digest?${q}`);
    window.dispatchEvent(new Event("reader-location-written"));
    setPosition(next);
  };
  const onBusy = (id: string, value: boolean) => {
    if (value) savingRef.current.add(id);
    else savingRef.current.delete(id);
    setSaving(new Set(savingRef.current));
  };
  return (
    <section
      class="reading-digest"
      aria-label="Subscription digest"
      aria-busy={busy}
    >
      <header class="reading-digest__header reading-navigation">
        <ReadingBackButton onClick={onExit} disabled={saving.size > 0} />
        <div>
          <p class="eyebrow">Reading mode</p>
          <h1>Подписка за подпиской</h1>
        </div>
      </header>
      <div class="reading-digest__status" role="status">
        {busy
          ? "Загрузка подборки…"
          : error ||
            `${source?.name ?? "Выберите подписку"} · ${page?.total ?? 0} непрочитанных · на странице ${page?.articles.length ?? 0}`}
      </div>
      <div class="reading-digest__body">
        <nav class="reading-digest__sources" aria-label="Подписки для чтения">
          {sources.map((subscription) => (
            <button
              key={subscription.id}
              class="reading-digest__source"
              aria-current={
                position.subscription === subscription.id ? "page" : undefined
              }
              disabled={busy || saving.size > 0}
              onClick={() => {
                if (position.subscription !== subscription.id)
                  turn({ subscription: subscription.id });
              }}
            >
              <span>{subscription.name}</span>
              <span>{subscription.unreadCount ?? "—"}</span>
            </button>
          ))}
        </nav>
        <div class="reading-digest__scroll" ref={scroll}>
          <div class="reading-digest__groups" inert={busy} aria-busy={busy}>
            {!busy && !error && (
              <section class="digest-source" aria-label={source?.name}>
                <header>
                  <h2>{source?.name ?? "Нет подписок"}</h2>
                </header>
                {page?.articles.map((article) => (
                  <DigestArticle
                    key={`${article.id}:${reload}`}
                    client={client}
                    owner={owner}
                    workspace={workspace}
                    article={article}
                    summary={summaries[article.id]}
                    navigate={navigate}
                    onBusy={onBusy}
                  />
                ))}
                {!page?.articles.length && (
                  <p class="reading-digest__empty">
                    В этой подписке нет непрочитанных статей.
                  </p>
                )}
              </section>
            )}
          </div>
        </div>
      </div>
      <footer class="reading-digest__footer">
        <button
          class="secondary-button"
          disabled={busy || saving.size > 0 || !page?.newerCursor}
          onClick={() =>
            turn({ cursor: page?.newerCursor ?? undefined, direction: "newer" })
          }
        >
          ← Новее
        </button>
        <button
          class="text-button"
          disabled={busy || saving.size > 0}
          onClick={() => {
            pageLock.current = true;
            setBusy(true);
            setReload((n) => n + 1);
          }}
        >
          Обновить подборку
        </button>
        <button
          class="secondary-button"
          disabled={busy || saving.size > 0 || !page?.olderCursor}
          onClick={() =>
            turn({ cursor: page?.olderCursor ?? undefined, direction: "older" })
          }
        >
          Старее →
        </button>
      </footer>
    </section>
  );
}
