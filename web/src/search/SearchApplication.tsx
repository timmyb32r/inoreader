import { MatchText } from "./MatchText";
import { useEffect, useRef, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type {
  SearchLimitsView,
  SearchPage,
  SearchTarget,
} from "../api/generated";
import { SearchField } from "../ui/SearchField";
import { SearchPreview } from "./SearchPreview";
import { StatusRegion } from "../ui/StatusRegion";
import "./search.css";
const kinds = ["all", "news", "wiki"] as const;
type Kind = (typeof kinds)[number];
function selection(params: URLSearchParams): SearchTarget | null {
  const kind = params.get("selectedKind"),
    container = params.get("container"),
    id = params.get("selected");
  if (!container || !id) return null;
  return kind === "news"
    ? { kind, workspace: container, article: id }
    : kind === "wiki"
      ? { kind, namespace: container, page: id }
      : null;
}
export function SearchApplication({
  client,
  url,
  navigate,
}: {
  client: ApiClient;
  url: string;
  navigate: (path: string) => boolean;
}) {
  const params = new URLSearchParams(url.split("?")[1] ?? "");
  const q = params.get("q") ?? "",
    rawKind = params.get("kind") ?? "all";
  const kind = rawKind as Kind;
  const request = new URLSearchParams({
    q,
    kind,
    offset: params.get("offset") ?? "0",
  });
  for (const name of ["namespace", "subscription"])
    if (params.has(name)) request.set(name, params.get(name)!);
  const key = request.toString(),
    target = selection(params);
  const [draft, setDraft] = useState(q),
    [limits, setLimits] = useState<SearchLimitsView | null>(null),
    [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [result, setResult] = useState<{
    key: string;
    page?: SearchPage;
    error?: string;
  } | null>(null);
  const locked = useRef(false);
  const busy = !!q && result?.key !== key;
  useEffect(() => {
    setDraft(q);
  }, [q]);
  useEffect(() => {
    let active = true;
    client.search
      .limits()
      .then((l) => {
        if (active) setLimits(l);
      })
      .catch((e) => {
        if (active) setError(e.message);
      });
    return () => {
      active = false;
    };
  }, [client]);
  useEffect(() => {
    let active = true;
    locked.current = false;
    if (!q) return;
    client.search
      .query(request)
      .then((page) => {
        if (active) setResult({ key, page });
      })
      .catch((e) => {
        if (active) setResult({ key, error: e.message });
      });
    return () => {
      active = false;
    };
  }, [key, client, retry]);
  const change = (
    value: string,
    nextKind: Kind = kind,
    offset = 0,
    clearScope = false,
  ) => {
    if (locked.current) return;
    if (!limits) {
      setError("Search settings are still loading");
      return;
    }
    if (
      new TextEncoder().encode(value).length > limits.query_bytes ||
      value.includes("\0")
    ) {
      setError("Search exceeds the configured query limit");
      return;
    }
    const next = new URLSearchParams({
      q: value,
      kind: nextKind,
      offset: String(offset),
    });
    if (!clearScope && nextKind === kind)
      for (const name of ["namespace", "subscription"])
        if (params.has(name)) next.set(name, params.get(name)!);
    if (next.toString() === key) {
      if (result?.error) {
        locked.current = true;
        setResult(null);
        setError("");
        setRetry((v) => v + 1);
      }
      return;
    }
    locked.current = true;
    setError("");
    if (!navigate(`/search?${next}`)) locked.current = false;
  };
  const page = result?.key === key ? result.page : undefined;
  const choose = (t: SearchTarget) => {
    const next = new URLSearchParams(params);
    next.set("selectedKind", t.kind);
    next.set("container", t.kind === "news" ? t.workspace : t.namespace);
    next.set("selected", t.kind === "news" ? t.article : t.page);
    navigate(`/search?${next}`);
  };
  const offset = Number(request.get("offset"));
  return (
    <section class="search-shell" aria-label="Search">
      <div class="search-heading">
        <div>
          <small>YOUR LIBRARY</small>
          <h1>Search</h1>
        </div>
        <SearchField
          value={draft}
          onInput={setDraft}
          onSubmit={() => change(draft)}
          busy={busy || !limits}
        />
      </div>
      <div class="search-filters" aria-label="Search scope">
        {kinds.map((k) => (
          <button
            key={k}
            aria-pressed={k === kind}
            onClick={() => change(q, k, 0, k !== kind)}
          >
            {k === "all" ? "All" : k === "news" ? "News" : "Wiki"}
          </button>
        ))}
        <span class="search-scope">
          {params.has("namespace")
            ? "This wiki namespace"
            : params.has("subscription")
              ? "This subscription"
              : "All accessible content"}
        </span>
        <button
          class={
            !params.has("namespace") && !params.has("subscription")
              ? "search-clear--hidden"
              : ""
          }
          onClick={() => change(q, kind, 0, true)}
        >
          Clear scope
        </button>
      </div>
      <StatusRegion class="search-status" busy={busy}>
        {error ||
          (result?.key === key && result.error) ||
          (busy ? (
            <>
              <span class="spinner" /> Searching…
            </>
          ) : q ? (
            page?.items.length ? (
              `${offset + 1}–${offset + page.items.length}${page.has_more ? " · more results available" : ""}`
            ) : (
              "No results"
            )
          ) : (
            "Search article titles, full text and wiki pages"
          ))}
      </StatusRegion>
      <div class="search-columns">
        <section
          class="search-results"
          aria-label="Search results"
          aria-busy={busy}
        >
          <div class="search-results__list">
            {page?.items.map((hit) => (
              <button
                key={JSON.stringify(hit.target)}
                class="search-hit"
                aria-pressed={
                  JSON.stringify(hit.target) === JSON.stringify(target)
                }
                onClick={() => choose(hit.target)}
              >
                <small>
                  {hit.target.kind === "news" ? "NEWS" : "WIKI"} · {hit.context}
                </small>
                <strong>
                  <MatchText text={hit.title || "Untitled post"} query={q} />
                </strong>
                <span>
                  <MatchText text={hit.excerpt} query={q} />
                </span>
              </button>
            ))}
            {!q && (
              <p class="search-empty">One search for your news and wiki.</p>
            )}
          </div>
          <footer>
            <button
              disabled={busy || !limits || !offset}
              onClick={() =>
                change(q, kind, Math.max(0, offset - limits!.page_size))
              }
            >
              ← Previous
            </button>
            <button
              disabled={busy || !limits || !page?.has_more}
              onClick={() => change(q, kind, offset + limits!.page_size)}
            >
              Next →
            </button>
          </footer>
        </section>
        <SearchPreview client={client} target={target} navigate={navigate} />
      </div>
    </section>
  );
}
