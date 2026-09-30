import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../../api/client";
import type { Article } from "../../api/viewModels";
import { ChatMarkdown } from "../../ai/ChatMarkdown";
import { Icon } from "../../ui/Icon";
import { ArticleDates } from "../../ui/ArticleDates";
import type { DigestSummary } from "./digestSummaries";
import { ArticleFeedback } from "./ArticleFeedback";

/** Summaries are loaded before the page is shown so rating controls never move. */
export function DigestArticle({
  client,
  owner,
  workspace,
  article,
  summary,
  navigate,
  onBusy,
}: {
  client: ApiClient;
  owner: string;
  workspace: string;
  article: Article;
  summary: DigestSummary;
  navigate: (path: string) => void;
  onBusy: (id: string, value: boolean) => void;
}) {
  const root = useRef<HTMLElement>(null);
  const [visible, setVisible] = useState(false);
  const [done, setDone] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "160px" },
    );
    if (root.current) observer.observe(root.current);
    return () => observer.disconnect();
  }, []);
  const read = () =>
    navigate(
      `/reading?${new URLSearchParams({ workspace, article: article.id, from: "digest", digestSubscription: new URLSearchParams(location.search).get("subscription") ?? article.subscriptionIds?.[0] ?? "" })}`,
    );
  return (
    <article
      ref={root}
      class="digest-article"
      aria-label={article.title || "Untitled post"}
      data-read={done}
    >
      <div class="digest-article__content">
        <div class="digest-article__topline">
          <div class="digest-article__dates">
            <ArticleDates article={article} />
          </div>
          <a
            class="reader-toolbar-button reader-toolbar-button--icon toolbar-tooltip toolbar-tooltip--right"
            href={article.url}
            target="_blank"
            rel="noopener noreferrer"
            aria-label="Open original"
            data-tooltip="Open original"
          >
            <Icon name="external" size={16} />
          </a>
        </div>
        <h3>
          <button
            class="digest-article__title"
            disabled={saving}
            onClick={read}
          >
            {article.title || "Untitled post"}
          </button>
        </h3>
        <div class="digest-article__summary">
          {summary.text ? (
            <ChatMarkdown text={summary.text} />
          ) : (
            <p class={summary.failed ? "digest-error" : ""}>{summary.status}</p>
          )}
        </div>
        <div class="digest-article__summary-status" role="status">
          {summary.text ? summary.status : ""}
        </div>
        <div class="digest-article__links">
          <button class="text-button" disabled={saving} onClick={read}>
            Читать и обсудить →
          </button>
        </div>
      </div>
      <div class="digest-article__feedback">
        {visible && (
          <ArticleFeedback
            inline
            client={client}
            owner={owner}
            workspace={workspace}
            article={article.id}
            onSave={async (command) => {
              const receipt = await client.reading.complete(
                workspace,
                article.id,
                command,
              );
              if (receipt.undone)
                throw Error(
                  "Эта операция уже отменена. Обновите подборку, чтобы оценить статью заново.",
                );
            }}
            onSaved={() => setDone(true)}
            onBusy={(value) => {
              setSaving(value);
              onBusy(article.id, value);
            }}
          />
        )}
      </div>
    </article>
  );
}
