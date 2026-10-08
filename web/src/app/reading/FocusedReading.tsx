import { SessionClockContext } from "./useSessionClock";
import { CommitGroup } from "./CommitGroup";
import { isCommitArticle } from "./commitArticle";
import { ReadingBackButton } from "./ReadingBackButton";
import { useContext, useEffect, useState } from "preact/hooks";
import type { ApiClient } from "../../api/client";
import type { AiProfile } from "../../api/ai";
import type { ArticleChatController } from "../../ai/useArticleChat";
import { ArticleChatContent } from "../../ai/ArticleChatContent";
import { ArticleReader } from "../ArticleReader";
import { GlossaryPanel } from "../../glossary/GlossaryPanel";
import type { useGlossary } from "../../glossary/useGlossary";
import { Icon } from "../../ui/Icon";
import { StatusRegion } from "../../ui/StatusRegion";
import { useFocusedReading } from "./useFocusedReading";
import { RatingReasonDialog } from "./RatingReasonDialog";
import "./focused-reading.css";

type Props = {
  client: ApiClient;
  owner: string;
  workspace: string;
  chat: ArticleChatController;
  profile: AiProfile | null;
  glossary: ReturnType<typeof useGlossary>;
  navigate: (path: string) => void;
  onExit: (article?: string) => void;
  announce: (message: string) => void;
};
export function FocusedReading({
  client,
  owner,
  workspace,
  chat,
  profile,
  glossary,
  navigate,
  onExit,
  announce,
}: Props) {
  const from = new URLSearchParams(location.search).get("from");
  const smart = from === "smart" || from === "session" || from === "random";
  const reading = useFocusedReading(client, owner, workspace);
  const { article, state, busy, score } = reading;
  const sessionClock = useContext(SessionClockContext);
  useEffect(() => {
    if (busy) return;
    window.dispatchEvent(
      new CustomEvent("reader-reading-ready", {
        detail: location.pathname + location.search,
      }),
    );
    if (!article) return;
    const saved = sessionClock?.active;
    if (
      saved?.url === location.pathname + location.search &&
      saved.workspace === workspace
    ) {
      const pane = document.querySelector<HTMLElement>(
        ".focused-reading__article .reader-body",
      );
      if (pane) pane.scrollTop = saved.scroll;
    }
  }, [busy, article?.id, owner, workspace]);
  const [commitIds, setCommitIds] = useState<string[] | null>(null);
  const [discussion, setDiscussion] = useState<{
    id: string;
    title: string;
  } | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  useEffect(() => {
    if (!article || (state?.read && !reading.uncertain && !busy))
      setConfirming(null);
  }, [article?.id, state?.read, reading.uncertain, busy]);
  const [tab, setTab] = useState<"summary" | "terms" | "both">("summary");
  const [mobile, setMobile] = useState<"article" | "assistant">("article");
  useEffect(() => {
    setDiscussion(null);
    setCommitIds(null);
    setTab("both");
    setMobile("article");
    glossary.close();
    if (article?.fullText === "ready")
      void glossary.open(article.id, article.title, false);
    if (article?.fullText === "ready")
      void chat.open(
        { workspaceId: workspace, articleId: article.id, title: article.title },
        false,
      );
    else chat.close();
  }, [article?.id, article?.fullText]);
  const sameChat =
    article?.fullText === "ready" &&
    chat.target?.articleId === (discussion?.id ?? article.id) &&
    chat.target.workspaceId === workspace;
  const showTerms =
    (tab === "terms" || tab === "both") &&
    glossary.target?.id === (discussion?.id ?? article?.id);
  const commit = !!article && isCommitArticle(article.url);
  const pending = new Set<string>(
    article && busy ? [`${article.id}:later`] : [],
  );
  const openTerms = () => {
    if (!article) return;
    setTab("terms");
    setMobile("assistant");
    const target = discussion ?? article;
    void glossary.open(target.id, target.title);
  };
  const summarize = () => {
    if (!article || article.fullText !== "ready") return;
    setTab("summary");
    setMobile("assistant");
    void chat.open({
      workspaceId: workspace,
      articleId: discussion?.id ?? article.id,
      title: discussion?.title ?? article.title,
    });
  };
  return (
    <section
      class="focused-reading"
      aria-label="Focused reading"
      aria-busy={!!busy}
    >
      <header class="focused-reading__header reading-navigation">
        <ReadingBackButton onClick={() => onExit(article?.id)} />
        {reading.session.error ? (
          <div class="focused-reading__mode">Сессия недоступна</div>
        ) : reading.session.enabled ? (
          <div class="focused-reading__mode">
            {reading.session.mode === "smart"
              ? "Умная лента"
              : "Случайная лента"}
          </div>
        ) : (
          <div class="focused-reading__mode">
            {from === "random"
              ? "Случайная лента"
              : smart
                ? "Умная лента"
                : "Reading mode"}
          </div>
        )}
        {smart && (
          <a
            class="text-button"
            href={`/smart?workspace=${encodeURIComponent(workspace)}`}
            onClick={(event) => {
              event.preventDefault();
              navigate(`/smart?workspace=${encodeURIComponent(workspace)}`);
            }}
          >
            Профиль интересов
          </a>
        )}
        <a
          class="text-button"
          href="/wiki"
          onClick={(e) => {
            e.preventDefault();
            navigate("/wiki");
          }}
        >
          Wiki
        </a>
        <div class="focused-reading__mobile-tabs" aria-label="Reading pane">
          <button
            class="text-button"
            aria-pressed={mobile === "article"}
            onClick={() => setMobile("article")}
          >
            Article
          </button>
          <button
            class="text-button"
            aria-pressed={mobile === "assistant"}
            onClick={() => setMobile("assistant")}
          >
            Assistant
          </button>
        </div>
      </header>
      <div class="focused-reading__panes" data-mobile={mobile}>
        <div
          class="focused-reading__article"
          data-departing={reading.departing}
        >
          {article && commit ? (
            <CommitGroup
              key={article.id}
              client={client}
              workspace={workspace}
              article={article}
              enabled={!!profile?.enabled}
              busy={!!busy}
              failures={reading.commitFailures}
              onSnapshot={setCommitIds}
              onDiscuss={(item) => {
                setDiscussion({ id: item.id, title: item.title });
                setTab("summary");
                setMobile("assistant");
                void chat.open({
                  workspaceId: workspace,
                  articleId: item.id,
                  title: item.title,
                });
              }}
            />
          ) : article ? (
            <ArticleReader
              key={article.id}
              focused
              article={article}
              className="focused-reading__original"
              pending={pending}
              onBack={() => onExit(article.id)}
              onOpenSubscription={(id) =>
                navigate(`/subscriptions/${encodeURIComponent(id)}`)
              }
              onUpdate={(patch) => {
                if (patch.later !== undefined)
                  void reading.updateLater(patch.later);
              }}
              onRefresh={reading.refresh}
              onNotice={announce}
              onSummarize={summarize}
              summaryPending={!!chat.busy}
              onDefinitions={openTerms}
              definitionsPending={glossary.actionBusy}
              aiClient={client.ai}
              wikiClient={client.wiki}
              onNavigate={navigate}
              workspaceId={workspace}
              translationEnabled={!!profile?.enabled}
            />
          ) : (
            <div class="focused-reading__empty">
              <Icon name={reading.done ? "unread" : "book"} size={30} />
              <h1>
                {reading.done
                  ? reading.session.progress.finished && reading.session.enabled
                    ? "Сессия завершена"
                    : "All caught up"
                  : "Reading mode"}
              </h1>
              <p>
                {reading.done
                  ? reading.session.progress.finished && reading.session.enabled
                    ? "Запланированное время чтения закончилось."
                    : "No more unread articles in this queue."
                  : busy
                    ? "Loading your article…"
                    : "The article could not be loaded."}
              </p>
              {reading.done ? (
                <button class="text-button" onClick={() => onExit()}>
                  Back to Feed
                </button>
              ) : (
                <button
                  class="text-button"
                  disabled={!!busy}
                  onClick={reading.retryLoad}
                >
                  Retry loading
                </button>
              )}
            </div>
          )}
        </div>
        <aside
          class={`focused-reading__assistant ${tab === "both" ? "focused-reading__assistant--both" : ""}`}
          aria-label="Article assistant"
        >
          <header>
            <Icon name="chat" size={17} />
            <strong>Article assistant</strong>
            <span>DeepSeek</span>
          </header>
          <div
            class="focused-reading__tabs"
            role="tablist"
            aria-label="Assistant tabs"
          >
            {(["both", "summary", "terms"] as const).map((value) => (
              <button
                key={value}
                role="tab"
                aria-selected={tab === value}
                disabled={!article}
                aria-busy={value === "terms" && glossary.busy}
                onClick={() =>
                  value === "terms" ? openTerms() : setTab(value)
                }
              >
                {value === "both"
                  ? "Summary + Terms"
                  : value === "summary"
                    ? "Summary / Chat"
                    : "Terms"}
              </button>
            ))}
          </div>
          <div
            class="ai-chat focused-reading__chat"
            hidden={tab === "terms" && showTerms}
          >
            {sameChat ? (
              <ArticleChatContent
                key={discussion?.id ?? article?.id}
                controller={chat}
                profile={profile}
              />
            ) : (
              <div class="focused-reading__unavailable">
                <p>
                  {!article
                    ? "Your assistant will appear with the next article."
                    : article.fullText !== "ready"
                      ? "The full article is not available. Read the excerpt or open the original."
                      : "Opening the saved conversation…"}
                </p>
              </div>
            )}
          </div>
          {showTerms && (
            <GlossaryPanel
              key={article?.id}
              embedded
              controller={{
                ...glossary,
                close: () => {
                  glossary.close();
                  setTab("summary");
                },
              }}
            />
          )}
        </aside>
      </div>
      <footer class="focused-reading__footer">
        <div class="focused-reading__rating-label">
          <strong>Useful to you?</strong>
          <span>
            {commit ? "Коммиты проекта · без оценки" : "1–10 / Не знаю"}
          </span>
        </div>
        <div
          class="focused-reading__rating"
          style={{ visibility: commit ? "hidden" : "visible" }}
        >
          <div role="group" aria-label="Personal value from 1 to 10">
            {Array.from({ length: 10 }, (_, i) => i + 1).map((value) => (
              <button
                key={value}
                aria-label={`Rate ${value} out of 10`}
                aria-pressed={score === value}
                disabled={!!busy || !state || state.read || reading.uncertain}
                onClick={() => reading.setScore(value)}
              >
                {value}
              </button>
            ))}
          </div>
          <div class="focused-reading__rating-ends">
            <button
              class="text-button"
              disabled={!!busy || !state || state.read || reading.uncertain}
              aria-pressed={reading.abstain}
              onClick={reading.setAbstain}
            >
              Не знаю
            </button>
            <span>Not useful</span>
            <span>Very useful</span>
          </div>
        </div>
        <button
          class="primary-button focused-reading__next"
          aria-busy={!!busy}
          disabled={
            !!busy ||
            !state ||
            (commit && commitIds === null) ||
            (!state.read && score === null && !reading.abstain && !commit)
          }
          onClick={() => {
            if (commit && commitIds) {
              if (commitIds.length) void reading.completeCommits(commitIds);
              else void reading.continue();
              return;
            }
            if (state?.read && !reading.uncertain) void reading.continue();
            else if (article) {
              if (commit && score === null) reading.setAbstain();
              setConfirming(article.id);
            }
          }}
        >
          <span>
            {commit
              ? commitIds?.length
                ? "Все прочитаны →"
                : "Далее →"
              : reading.uncertain
                ? "Retry saving"
                : state?.read
                  ? "Continue →"
                  : "Read & next →"}
          </span>
          {!!busy && (
            <span class="focused-reading__pending">
              <span class="spinner" />
            </span>
          )}
        </button>
        <StatusRegion class="focused-reading__status" busy={!!busy}>
          <span class={reading.error ? "ai-error" : ""}>
            {reading.error ||
              reading.session.storageError ||
              busy ||
              (commit
                ? commitIds
                  ? `${commitIds.length} коммитов · отметка по кнопке`
                  : "Загружаю подборку коммитов…"
                : "") ||
              (reading.done
                ? "Queue complete"
                : state?.read
                  ? "Already marked read. Continue without changing its rating."
                  : score === null
                    ? state?.rating != null
                      ? `Previous rating: ${state.rating}/10 · choose a rating to finish.`
                      : "Rate this article to finish reading."
                    : `Your rating: ${score}/10`)}
          </span>
        </StatusRegion>
        <div class="focused-reading__recovery">
          <button
            class="text-button"
            style={{
              visibility:
                reading.error && !reading.uncertain ? "visible" : "hidden",
            }}
            disabled={!!busy}
            onClick={reading.retryLoad}
          >
            Reload article
          </button>
          <button
            class="text-button"
            style={{ visibility: reading.undo ? "visible" : "hidden" }}
            disabled={!!busy || reading.uncertain}
            onClick={() => void reading.undoLast()}
          >
            Undo last read
          </button>
        </div>
      </footer>
      {article &&
        confirming === article.id &&
        (score !== null || reading.abstain) &&
        (!state?.read || !!busy || reading.uncertain) && (
          <RatingReasonDialog
            score={score}
            reason={reading.reason}
            onReason={reading.setReason}
            onScore={(value) =>
              value === null ? reading.setAbstain() : reading.setScore(value)
            }
            busy={!!busy}
            uncertain={reading.uncertain}
            error={reading.error || reading.draftError}
            onClose={() => setConfirming(null)}
            onSave={() => void reading.complete()}
          />
        )}
    </section>
  );
}
