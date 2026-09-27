import type { Article } from "../api/viewModels";
import { formatArticleDate } from "../ui/formatArticleDate";
import "./article-dates.css";

export function ArticleDates({ article }: { article: Article }) {
  const missing =
    article.publicationStatus === "conflicting"
      ? "Conflicting dates"
      : article.publicationStatus === "invalid"
        ? "Invalid source date"
        : "Unknown";
  const source = article.publicationSources?.join(", ");
  return (
    <span class="article-dates">
      <span
        title={
          article.publishedAt
            ? `Publication date · ${source} · ${article.publishedAt}`
            : "The source has not provided an unambiguous publication date"
        }
      >
        <span class="article-dates__label">Published</span>{" "}
        {article.publishedAt ? (
          <time dateTime={article.publishedAt}>
            {formatArticleDate(article.publishedAt)}
          </time>
        ) : (
          <span>{missing}</span>
        )}
      </span>
      <span title={`First saved in this workspace · ${article.savedAt}`}>
        <span class="article-dates__label">Saved</span>{" "}
        <time dateTime={article.savedAt}>
          {formatArticleDate(article.savedAt)}
        </time>
      </span>
    </span>
  );
}
