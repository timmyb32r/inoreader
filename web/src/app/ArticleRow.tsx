import { formatArticleDate } from "../ui/formatArticleDate";
import { Icon } from "../ui/Icon";
import type { Article } from "../api/viewModels";
export function ArticleRow({
  article,
  selected,
  laterPending,
  onOpen,
  onLater,
}: {
  article: Article;
  selected: boolean;
  laterPending: boolean;
  onOpen: () => void;
  onLater: () => void;
}) {
  return (
    <article
      class={`article-row ${selected ? "selected" : ""} ${article.read ? "read" : ""}`}
    >
      <button class="article-row__main" onClick={onOpen}>
        <span class="unread-dot" />
        <span class="article-row__source">
          {article.sources?.join(" · ") ?? article.source}
        </span>
        <time>{formatArticleDate(article.age)}</time>
        <h2>{article.title}</h2>
        <p>{article.excerpt}</p>
        <span class={`fulltext fulltext--${article.fullText}`}>
          {article.fullText === "pending" && (
            <span class="spinner" aria-hidden="true" />
          )}
          {article.fullText === "ready"
            ? "Full text"
            : article.fullText === "pending"
              ? "Fetching full text"
              : "Excerpt only"}
        </span>
      </button>
      <button
        class={`row-later ${article.later ? "active" : ""}`}
        disabled={laterPending}
        aria-busy={laterPending}
        aria-label={
          article.later
            ? "Remove article from Read later"
            : "Read article later"
        }
        onClick={onLater}
      >
        {laterPending ? (
          <span class="spinner" />
        ) : (
          <Icon name="later" size={17} />
        )}
      </button>
    </article>
  );
}
export function EmptyState() {
  return (
    <div class="empty-state">
      <span>
        <Icon name="check" size={24} />
      </span>
      <h2>Nothing here right now</h2>
      <p>You are caught up in this view.</p>
    </div>
  );
}
