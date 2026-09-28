import { useEffect, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type { SearchTarget, WikiPage } from "../api/generated";
import type { Article } from "../api/viewModels";
import { WikiMarkdown } from "../wiki/WikiMarkdown";
import { ParagraphReader } from "../translation/ParagraphReader";
import { StatusRegion } from "../ui/StatusRegion";
import { ArticleDates } from "../ui/ArticleDates";
export function SearchPreview({
  client,
  target,
  navigate,
}: {
  client: ApiClient;
  target: SearchTarget | null;
  navigate: (path: string) => boolean;
}) {
  const key = JSON.stringify(target);
  const [result, setResult] = useState<{
    key: string;
    article?: Article;
    page?: WikiPage;
    error?: string;
  } | null>(null);
  useEffect(() => {
    let active = true;
    if (!target) return;
    const request =
      target.kind === "news"
        ? client
            .getArticle(target.workspace, target.article)
            .then((article) => ({ key, article }))
        : client.wiki
            .page(target.namespace, target.page)
            .then((page) => ({ key, page }));
    request
      .then((value) => {
        if (active) setResult(value);
      })
      .catch((e: unknown) => {
        if (active)
          setResult({
            key,
            error: e instanceof Error ? e.message : "Preview unavailable",
          });
      });
    return () => {
      active = false;
    };
  }, [key, client]);
  const current = result?.key === key ? result : null;
  const busy = !!target && !current;
  return (
    <section
      class="search-preview"
      aria-label="Search preview"
      aria-busy={busy}
    >
      <header>
        <span>Preview</span>
        {current?.page && (
          <button
            onClick={() =>
              navigate(
                `/wiki/${current.page!.namespace}/page/${current.page!.id}`,
              )
            }
          >
            Open in Wiki ↗
          </button>
        )}
        {current?.article && (
          <a
            href={current.article.url}
            target="_blank"
            rel="noopener noreferrer"
          >
            Open original ↗
          </a>
        )}
      </header>
      <StatusRegion class="search-status" busy={busy}>
        {busy ? (
          <>
            <span class="spinner" /> Loading preview…
          </>
        ) : (
          (current?.error ?? "")
        )}
      </StatusRegion>
      <div class="search-preview__body" key={key}>
        {!target && (
          <div class="search-empty">
            <h2>Select a result</h2>
            <p>Read news and wiki pages here without leaving your search.</p>
          </div>
        )}
        {current?.page && (
          <>
            <small>Wiki</small>
            <h1>{current.page.name}</h1>
            <WikiMarkdown
              text={current.page.markdown}
              onLink={async (name) => {
                try {
                  const page = await client.wiki.resolve(
                    current.page!.namespace,
                    name,
                  );
                  navigate(`/wiki/${page.namespace}/page/${page.id}`);
                } catch (error) {
                  setResult((previous) =>
                    previous?.key === key
                      ? {
                          ...previous,
                          error:
                            error instanceof Error
                              ? error.message
                              : "Wiki page unavailable",
                        }
                      : previous,
                  );
                }
              }}
            />
          </>
        )}
        {current?.article && target?.kind === "news" && (
          <>
            <small>{current.article.source}</small>
            <ArticleDates article={current.article} />
            <ParagraphReader
              client={client.ai}
              workspaceId={target.workspace}
              articleId={target.article}
              title={current.article.title}
              excerpt={current.article.excerpt}
              html={current.article.bodyHtml ?? ""}
              body={current.article.body}
              enabled={false}
            />
          </>
        )}
      </div>
    </section>
  );
}
