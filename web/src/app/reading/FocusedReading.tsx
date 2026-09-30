import { ReadingBackButton } from "./ReadingBackButton";
import { useEffect, useState } from "preact/hooks";
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
  const reading = useFocusedReading(client, owner, workspace);
  const { article, state, busy, score } = reading;
  const [confirming, setConfirming] = useState<string | null>(null);
  useEffect(() => {
    if (!article || (state?.read && !reading.uncertain && !busy))
      setConfirming(null);
  }, [article?.id, state?.read, reading.uncertain, busy]);
  const [tab, setTab] = useState<"summary" | "terms">("summary");
  const [mobile, setMobile] = useState<"article" | "assistant">("article");
  useEffect(() => {
    setTab("summary");
    setMobile("article");
    glossary.close();
    if (article?.fullText === "ready")
      void chat.open(
        { workspaceId: workspace, articleId: article.id, title: article.title },
        false,
      );
    else chat.close();
  }, [article?.id, article?.fullText]);
  const sameChat =
    article?.fullText === "ready" &&
    chat.target?.articleId === article.id &&
    chat.target.workspaceId === workspace;
  const showTerms = tab === "terms" && glossary.target?.id === article?.id;
  const pending = new Set<string>(
    article && busy ? [`${article.id}:later`] : [],
  );
  const openTerms = () => {
    if (!article) return;
    setTab("terms");
    setMobile("assistant");
    if (glossary.target?.id !== article.id)
      void glossary.open(article.id, article.title);
  };
  const summarize = () => {
    if (!article || article.fullText !== "ready") return;
    setTab("summary");
    setMobile("assistant");
    void chat.open({
      workspaceId: workspace,
      articleId: article.id,
      title: article.title,
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
        <span>Reading mode</span>
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
        <div class="focused-reading__article">
          {article ? (
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
              definitionsPending={glossary.busy}
              aiClient={client.ai}
              wikiClient={client.wiki}
              onNavigate={navigate}
              workspaceId={workspace}
              translationEnabled={!!profile?.enabled}
            />
          ) : (
            <div class="focused-reading__empty">
              <Icon name={reading.done ? "unread" : "book"} size={30} />
              <h1>{reading.done ? "All caught up" : "Reading mode"}</h1>
              <p>
                {reading.done
                  ? "No more unread articles in this queue."
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
          class="focused-reading__assistant"
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
            {(["summary", "terms"] as const).map((value) => (
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
                {value === "summary" ? "Summary" : "Terms"}
              </button>
            ))}
          </div>
          <div class="ai-chat focused-reading__chat" hidden={showTerms}>
            {sameChat ? (
              <ArticleChatContent
                key={article?.id}
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
          <span>Required · 1–10</span>
        </div>
        <div class="focused-reading__rating">
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
            <span>Not useful</span>
            <span>Very useful</span>
          </div>
        </div>
        <button
          class="primary-button focused-reading__next"
          aria-busy={!!busy}
          disabled={!!busy || !state || (!state.read && score === null)}
          onClick={() => {
            if (state?.read && !reading.uncertain) void reading.continue();
            else if (article) setConfirming(article.id);
          }}
        >
          <span>
            {reading.uncertain
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
              busy ||
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
        score !== null &&
        (!state?.read || !!busy || reading.uncertain) && (
          <RatingReasonDialog
            score={score}
            reason={reading.reason}
            onReason={reading.setReason}
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
