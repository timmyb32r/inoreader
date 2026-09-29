import { useEffect, useRef, useState } from "preact/hooks";
import { ApiError, type ApiClient } from "../../api/client";
import type { Article } from "../../api/viewModels";
import type {
  CompleteReading,
  ReadingCompletion,
  ReadingState,
} from "../../api/focusedReading";
import { assertWire } from "../../api/decode";

type Pending = { article: string; command: CompleteReading };
type Saved = {
  owner: string;
  workspace: string;
  pending?: Pending;
  undo?: ReadingCompletion;
};
const currentUrl = () => location.pathname + location.search;
/** One queue, separate from ordinary reader's auto-read selection. Completion IDs
 * survive lost responses/reload in account-scoped history entries. No background writes. */
export function useFocusedReading(
  client: ApiClient,
  owner: string,
  workspace: string,
) {
  const [article, setArticle] = useState<Article | null>(null);
  const [state, setState] = useState<ReadingState | null>(null);
  const [score, setScore] = useState<number | null>(null);
  const [busy, setBusy] = useState("Loading article");
  const [error, setError] = useState("");
  const [done, setDone] = useState(false);
  const [undo, setUndo] = useState<ReadingCompletion | null>(null);
  const [uncertain, setUncertain] = useState(false);
  const [url, setUrl] = useState(currentUrl);
  const [reload, setReload] = useState(0);
  const epoch = useRef(0),
    lock = useRef(false),
    pending = useRef<Pending>();
  const remaining = useRef<Article[]>([]);
  const saved = (): Saved => {
    const value = history.state?.focusedReading;
    if (!value || value.owner !== owner || value.workspace !== workspace)
      return { owner, workspace };
    if (value.pending) {
      assertWire("CompleteReading", value.pending.command);
      if (typeof value.pending.article !== "string")
        throw Error("Invalid saved reading operation");
    }
    if (value.undo) assertWire("ReadingCompletion", value.undo);
    return value;
  };
  const remember = (change: Partial<Saved>) =>
    history.replaceState(
      { ...history.state, focusedReading: { ...saved(), ...change } },
      "",
      currentUrl(),
    );
  const selectUrl = (id: string | null, replace = false) => {
    const query = new URLSearchParams({ workspace });
    if (id) query.set("article", id);
    else query.set("done", "1");
    const next = `/reading?${query}`;
    history[replace ? "replaceState" : "pushState"](history.state, "", next);
    window.dispatchEvent(new Event("reader-location-written"));
    setUrl(next);
  };
  useEffect(() => {
    const restore = () => {
      if (location.pathname === "/reading") {
        remaining.current = [];
        setUrl(currentUrl());
      }
    };
    window.addEventListener("popstate", restore);
    return () => {
      epoch.current++;
      window.removeEventListener("popstate", restore);
    };
  }, []);
  useEffect(() => {
    const token = ++epoch.current;
    lock.current = false;
    setBusy("Loading article");
    setError("");
    setArticle(null);
    setState(null);
    setScore(null);
    setDone(false);
    setUncertain(false);
    let disposed = false;
    void (async () => {
      try {
        const snapshot = saved();
        pending.current = snapshot.pending;
        setUndo(snapshot.undo ?? null);
        const q = new URL(url, location.origin).searchParams;
        if (q.get("done") === "1") {
          setDone(true);
          return;
        }
        let id = q.get("article");
        if (!id) {
          const page = await client.listArticles(workspace, "feed");
          if (disposed) return;
          id = page.articles[0]?.id ?? null;
          selectUrl(id, true);
          return;
        }
        const [item, read] = await Promise.all([
          client.getArticle(workspace, id),
          client.reading.state(workspace, id),
        ]);
        if (disposed || token !== epoch.current) return;
        setArticle(item);
        setState(read);
        // Each visit requires a deliberate selection; a previous rating is shown
        // separately so a late load cannot enable completion under a repeat click.
        setScore(null);
        if (pending.current?.article === id) {
          setUncertain(true);
          setScore(pending.current.command.rating);
          setError(
            "The previous save needs confirmation. Retry uses the same operation.",
          );
        }
      } catch (cause) {
        if (!disposed) setError((cause as Error).message);
      } finally {
        if (!disposed && token === epoch.current) setBusy("");
      }
    })();
    return () => {
      disposed = true;
      epoch.current++;
    };
  }, [client, owner, workspace, url, reload]);
  const run = async (
    label: string,
    action: (token: number) => Promise<void>,
  ) => {
    if (lock.current) return;
    lock.current = true;
    setBusy(label);
    setError("");
    const token = epoch.current;
    try {
      await action(token);
    } catch (cause) {
      if (token === epoch.current) setError((cause as Error).message);
    } finally {
      if (token === epoch.current) {
        lock.current = false;
        setBusy("");
      }
    }
  };
  const next = async (id: string, token: number) => {
    if (!remaining.current.length) {
      const page = await client.reading.next(workspace, id);
      if (token !== epoch.current) return;
      remaining.current = page.articles;
    }
    const nextArticle = remaining.current.shift();
    if (token === epoch.current) selectUrl(nextArticle?.id ?? null);
  };
  const complete = () =>
    run("Saving rating", async (token) => {
      if (!article || !state || score === null) return;
      const request =
        pending.current?.article === article.id
          ? pending.current
          : {
              article: article.id,
              command: {
                operationId: crypto.randomUUID(),
                expectedRevision: state.revision,
                rating: score,
              },
            };
      pending.current = request;
      remember({ pending: request });
      let result: ReadingCompletion;
      try {
        result = await client.reading.complete(
          workspace,
          article.id,
          request.command,
        );
      } catch (cause) {
        if (token !== epoch.current) throw cause;
        if (
          cause instanceof ApiError &&
          cause.status >= 400 &&
          cause.status < 500
        ) {
          pending.current = undefined;
          remember({ pending: undefined });
          setUncertain(false);
        } else setUncertain(true);
        throw cause;
      }
      if (token !== epoch.current) return;
      pending.current = undefined;
      setUncertain(false);
      remember({
        pending: undefined,
        undo: result.undone ? undefined : result,
      });
      setUndo(result.undone ? null : result);
      setState(result.state);
      if (result.undone) {
        setScore(result.state.rating ?? null);
        return;
      }
      setArticle({ ...article, read: true });
      await next(article.id, token);
    });
  const undoLast = () =>
    run("Undoing completion", async (token) => {
      if (!undo) return;
      const result = await client.reading.undo(
        workspace,
        undo.articleId,
        undo.operationId,
      );
      if (token !== epoch.current) return;
      remaining.current = [];
      remember({ undo: undefined, pending: undefined });
      pending.current = undefined;
      setUndo(null);
      if (article?.id === result.articleId) {
        setReload((v) => v + 1);
      } else selectUrl(result.articleId);
    });
  const updateLater = (later: boolean) =>
    run("Saving bookmark", async (token) => {
      if (!article || uncertain) return;
      const updated = await client.updateArticle(workspace, article.id, {
        later,
      });
      const read = await client.reading.state(workspace, article.id);
      if (token === epoch.current) {
        setArticle(updated);
        setState(read);
      }
    });
  const refresh = async () => {
    if (!article || lock.current) return;
    await run("Fetching full text", async (token) => {
      await client.refreshFullText(workspace, article.id);
      if (token === epoch.current) setReload((v) => v + 1);
    });
  };
  return {
    article,
    state,
    score,
    setScore,
    busy,
    error,
    done,
    undo,
    uncertain,
    complete,
    undoLast,
    updateLater,
    refresh,
    retryLoad: () => setReload((v) => v + 1),
    continue: () =>
      run("Loading next article", async (token) => {
        if (article) await next(article.id, token);
      }),
  };
}
