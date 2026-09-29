import { ArticleDates } from "../ui/ArticleDates";
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
        <ArticleDates article={article} />
        <h2>
          {article.title || (
            <span aria-label="Post has no title">Untitled post</span>
          )}
        </h2>
        <p>{article.excerpt}</p>
        {article.markedReadAt && (
          <span class="article-read-at">
            ✓ Marked read{" "}
            {new Date(article.markedReadAt).toLocaleString("en-US")}
          </span>
        )}
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
export function EmptyState({
  readingHistory = false,
}: {
  readingHistory?: boolean;
}) {
  return (
    <div class="empty-state">
      <span>
        <Icon name="check" size={24} />
      </span>
      <h2>
        {readingHistory
          ? "No articles marked read in this period"
          : "Nothing here right now"}
      </h2>
      <p>
        {readingHistory
          ? "Choose another date to view its reading history."
          : "You are caught up in this view."}
      </p>
    </div>
  );
}
