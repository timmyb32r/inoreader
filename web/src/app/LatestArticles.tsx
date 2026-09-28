import { useEffect, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import { ArticleDates } from "../ui/ArticleDates";
import type { Article } from "../api/viewModels";
import "./latest-articles.css";

export function LatestArticles({
  client,
  workspaceId,
  subscriptionId,
  onOpenArticles,
}: {
  client: ApiClient;
  workspaceId: string;
  subscriptionId: string;
  onOpenArticles: (subscriptionId: string, articleId?: string) => void;
}) {
  const [articles, setArticles] = useState<Article[]>([]);
  const [status, setStatus] = useState("Loading latest articles…");
  useEffect(() => {
    let active = true;
    setArticles([]);
    setStatus("Loading latest articles…");
    client
      .listArticles(workspaceId, "subscription", subscriptionId)
      .then((page) => {
        if (!active) return;
        setArticles(page.articles.slice(0, 5));
        setStatus(page.articles.length ? "" : "No articles collected yet.");
      })
      .catch((error) => {
        if (active) setStatus(error.message);
      });
    return () => {
      active = false;
    };
  }, [client, workspaceId, subscriptionId]);
  return (
    <section class="detail-card recent-card">
      <div class="card-heading">
        <h2>Latest articles</h2>
        <button
          class="text-button"
          onClick={() => onOpenArticles(subscriptionId)}
        >
          View all articles
        </button>
      </div>
      <div
        class="recent-articles-content"
        aria-busy={status === "Loading latest articles…"}
      >
        {articles.length ? (
          articles.map((article) => (
            <article key={article.id}>
              <button
                class="recent-article-link"
                onClick={() => onOpenArticles(subscriptionId, article.id)}
              >
                <strong>
                  {article.title || (
                    <span aria-label="Post has no title">Untitled post</span>
                  )}
                </strong>
                <span>
                  <ArticleDates article={article} />
                  <span> · {article.read ? "Read" : "Unread"}</span>
                </span>
              </button>
            </article>
          ))
        ) : (
          <p role="status">{status}</p>
        )}
      </div>
    </section>
  );
}
