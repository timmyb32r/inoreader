import { formatArticleDate } from "../ui/formatArticleDate";
import type { Article } from "./data";
import "./latest-articles.css";

export function LatestArticles({
  articles,
  subscriptionId,
  onOpenArticles,
}: {
  articles: Article[];
  subscriptionId: string;
  onOpenArticles: (subscriptionId: string, articleId?: string) => void;
}) {
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
      {articles.length ? (
        articles.map((article) => (
          <article key={article.id}>
            <button
              class="recent-article-link"
              onClick={() => onOpenArticles(subscriptionId, article.id)}
            >
              <strong>{article.title}</strong>
              <span>
                {formatArticleDate(article.age)} ·{" "}
                {article.read ? "Read" : "Unread"}
              </span>
            </button>
          </article>
        ))
      ) : (
        <p>No articles collected yet.</p>
      )}
    </section>
  );
}
