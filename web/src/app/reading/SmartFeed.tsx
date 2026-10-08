import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../../api/client";
import type { SmartFeed as Feed } from "../../api/generated";
import { AutofillResistantTextarea } from "../../ui/fields";
import { ReadingBackButton } from "./ReadingBackButton";
import "./smart-feed.css";

/** Results only reorder on explicit navigation/refresh. Background scoring never
 * inserts a new hit target under a reader's pointer or moves an active control. */
export function SmartFeed({
  client,
  workspace,
  navigate,
  onExit,
}: {
  client: ApiClient;
  workspace: string;
  navigate: (path: string) => void;
  onExit: () => void;
}) {
  const query = new URLSearchParams(location.search);
  const random = query.get("order") === "random";
  const [generatedSeed] = useState(() => crypto.randomUUID());
  const seed = query.get("seed") ?? generatedSeed;
  const [feed, setFeed] = useState<Feed | null>(null);
  const [cursor, setCursor] = useState<string | undefined>();
  const [showHidden, setShowHidden] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState(false);
  const [prompt, setPrompt] = useState("");
  const lock = useRef(false);
  useEffect(() => {
    let active = true;
    setBusy(true);
    setError("");
    lock.current = true;
    (random
      ? client.interests.random(workspace, seed, cursor)
      : client.interests.feed(workspace, cursor, showHidden)
    )
      .then((value) => {
        if (active) {
          setFeed(value);
          setPrompt(value.profile?.prompt ?? "");
        }
      })
      .catch((cause: Error) => {
        if (active) setError(cause.message);
      })
      .finally(() => {
        if (active) {
          setBusy(false);
          lock.current = false;
        }
      });
    return () => {
      active = false;
    };
  }, [client, workspace, cursor, refresh, showHidden, random, seed]);
  const load = (position?: string) => {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    setCursor(position);
    setRefresh((n) => n + 1);
  };
  const save = async () => {
    if (lock.current || !feed?.profile) return;
    lock.current = true;
    setSaving(true);
    setError("");
    try {
      const profile = await client.interests.save({ ...feed.profile, prompt });
      setFeed({ ...feed, profile });
      setEditing(false);
      setCursor(undefined);
      setRefresh((n) => n + 1);
    } catch (cause) {
      setError((cause as Error).message);
    } finally {
      setSaving(false);
      lock.current = false;
    }
  };
  return (
    <section
      class="smart-feed"
      aria-label={random ? "Random feed" : "Smart feed"}
      aria-busy={busy}
    >
      <header class="smart-feed__header">
        <ReadingBackButton onClick={onExit} disabled={saving} />
        <h1>{random ? "Случайная лента" : "Умная лента"}</h1>
        <a
          class="text-button"
          href={
            random
              ? `/smart?workspace=${workspace}`
              : `/smart?${new URLSearchParams({ workspace, order: "random", seed: crypto.randomUUID() })}`
          }
          onClick={(event) => {
            event.preventDefault();
            navigate(event.currentTarget.href.replace(location.origin, ""));
          }}
        >
          {random ? "Умный порядок" : "Случайный порядок"}
        </a>
        <button
          class="secondary-button"
          aria-label="Обновить"
          disabled={busy || saving}
          onClick={() => load()}
          aria-busy={busy}
        >
          {busy ? "Загрузка…" : "Обновить"}
        </button>
        <button
          class="secondary-button"
          disabled={busy || saving || !feed?.profile}
          onClick={() => setEditing(true)}
        >
          Профиль интересов
        </button>
      </header>
      <div class="smart-feed__visibility">
        <button
          class="text-button"
          disabled={busy || saving}
          style={{ visibility: random ? "hidden" : "visible" }}
          aria-pressed={showHidden}
          onClick={() => {
            setCursor(undefined);
            setShowHidden(!showHidden);
          }}
        >
          {" "}
          {showHidden ? "Скрыть прогноз 1/10" : "Показать скрытые 1/10"}
        </button>
      </div>
      <div class="smart-feed__status" role="status" aria-live="polite">
        {error ||
          (feed
            ? `Оценено ${feed.scored} из ${feed.total} непрочитанных · ошибок: ${feed.failed} · профиль: ${feed.profile?.trainingCount ?? 0} примеров`
            : "Загрузка ленты…")}
      </div>
      <div class="smart-feed__content">
        {editing ? (
          <div class="smart-feed__editor">
            <h2>Что мне интересно</h2>
            <p>
              Этот профиль выведен из текстов, оценок и пояснений. Прогноз —
              ожидаемая личная ценность от 1 до 10. Сохранение обновит прогнозы;
              сами статьи и оценки сохраняются.
            </p>
            <AutofillResistantTextarea
              aria-label="Профиль интересов"
              value={prompt}
              onInput={(event) => setPrompt(event.currentTarget.value)}
              disabled={saving}
            />
            <div class="smart-feed__actions">
              <button
                class="primary-button"
                disabled={saving || !prompt.trim()}
                aria-busy={saving}
                onClick={() => void save()}
              >
                {saving ? "Сохранение…" : "Сохранить профиль"}
              </button>
              <button
                class="secondary-button"
                disabled={saving}
                onClick={() => setEditing(false)}
              >
                Закрыть
              </button>
            </div>
          </div>
        ) : (
          <>
            <p>
              Сначала — наиболее интересные по твоей разметке. Остальные статьи
              остаются доступны. Классификация идёт в фоне в пределах общего
              AI-бюджета; обновление списка — по кнопке.
            </p>
            {feed && !feed.profile && <p>Профиль интересов ещё не создан.</p>}
            {feed?.articles.map((article) => (
              <article class="smart-feed__article" key={article.id}>
                <div class="smart-feed__score">
                  {article.prediction
                    ? `${article.prediction.score}/10`
                    : "Ещё не оценено"}
                </div>
                <a
                  href={`/reading?${new URLSearchParams({ workspace, article: article.id, from: random ? "random" : "smart", ...(random ? { seed } : {}) })}`}
                  onClick={(event) => {
                    event.preventDefault();
                    navigate(
                      `/reading?${new URLSearchParams({ workspace, article: article.id, from: random ? "random" : "smart", ...(random ? { seed } : {}) })}`,
                    );
                  }}
                >
                  <h2>{article.title || "Пост без заголовка"}</h2>
                </a>
                <p class="smart-feed__excerpt">{article.excerpt}</p>
                <p>
                  {article.prediction?.reason ??
                    article.error ??
                    "Прогноз появится после классификации."}
                </p>
                {article.prediction && (
                  <small>
                    Уверенность:{" "}
                    {
                      (
                        {
                          high: "высокая",
                          medium: "средняя",
                          low: "низкая",
                        } as const
                      )[article.prediction.confidence]
                    }
                  </small>
                )}
              </article>
            ))}
            {feed && feed.total === 0 && <p>Непрочитанных статей нет.</p>}
          </>
        )}
      </div>
      <footer class="smart-feed__footer">
        <button
          class="secondary-button"
          disabled={busy || saving || editing || !cursor}
          onClick={() => load()}
        >
          В начало
        </button>
        <button
          class="secondary-button"
          disabled={busy || saving || editing || !feed?.nextCursor}
          onClick={() => load(feed?.nextCursor ?? undefined)}
        >
          Следующие
        </button>
      </footer>
    </section>
  );
}
