import { useReadingSession } from "./useReadingSession";
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
  const [reason, setReasonValue] = useState("");
  const [draftError, setDraftError] = useState("");
  const reasonDrafts = useRef(new Map<string, string>());
  const reasonKey = (id: string) =>
    `reading-reason:${owner}:${workspace}:${id}`;
  const [score, setScore] = useState<number | null>(null);
  const [abstain, setAbstain] = useState(false);
  const [departing, setDeparting] = useState(false);
  const [busy, setBusy] = useState("Loading article");
  const [error, setError] = useState("");
  const [commitFailures, setCommitFailures] = useState<string[]>([]);
  const [done, setDone] = useState(false);
  const [undo, setUndo] = useState<ReadingCompletion | null>(null);
  const [uncertain, setUncertain] = useState(false);
  const [url, setUrl] = useState(currentUrl);
  const [reload, setReload] = useState(0);
  const session = useReadingSession(owner, workspace);
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
    const query = new URLSearchParams(location.search);
    query.set("workspace", workspace);
    query.delete("article");
    query.delete("done");
    if (new URLSearchParams(location.search).get("from") === "smart")
      query.set("from", "smart");
    if (new URLSearchParams(location.search).get("from") === "digest") {
      query.set("from", "digest");
      const subscription = new URLSearchParams(location.search).get(
        "digestSubscription",
      );
      if (subscription) query.set("digestSubscription", subscription);
    }
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
    setCommitFailures([]);
    setArticle(null);
    setState(null);
    setScore(null);
    setAbstain(false);
    setDeparting(false);
    setReasonValue("");
    setDraftError("");
    setDone(false);
    setUncertain(false);
    let disposed = false;
    void (async () => {
      try {
        if (session.error) throw Error(session.error);
        const snapshot = saved();
        pending.current = snapshot.pending;
        setUndo(snapshot.undo ?? null);
        const q = new URL(url, location.origin).searchParams;
        if (
          q.get("done") === "1" ||
          (session.enabled && session.progress.finished)
        ) {
          setDone(true);
          setBusy("");
          return;
        }
        let id = q.get("article");
        if (!id) {
          const page =
            session.random || (session.enabled && session.mode === "random")
              ? await client.interests.random(workspace, session.seed)
              : q.get("from") === "smart" || session.enabled
                ? await client.interests.feed(workspace)
                : await client.listArticles(workspace, "feed");
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
        const key = reasonKey(id);
        try {
          setReasonValue(
            reasonDrafts.current.get(key) ?? sessionStorage.getItem(key) ?? "",
          );
        } catch {
          setReasonValue(reasonDrafts.current.get(key) ?? "");
          setDraftError(
            "This browser cannot retain your explanation after reload. Save it before leaving.",
          );
        }
        // Each visit requires a deliberate selection; a previous rating is shown
        // separately so a late load cannot enable completion under a repeat click.
        setScore(null);
        if (pending.current?.article === id) {
          setUncertain(true);
          setScore(pending.current.command.rating ?? null);
          setAbstain(pending.current.command.rating === null);
          setReasonValue(pending.current.command.reason ?? "");
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
    if (
      new URLSearchParams(location.search).get("from") === "smart" ||
      session.random ||
      session.enabled
    ) {
      const progress = session.enabled ? session.advance() : null;
      if (progress?.finished) {
        selectUrl(null);
        return;
      }
      const page =
        session.random || progress?.mode === "random"
          ? await client.interests.random(workspace, session.seed)
          : await client.interests.feed(workspace);
      if (token === epoch.current) selectUrl(page.articles[0]?.id ?? null);
      return;
    }
    if (!remaining.current.length) {
      const page = await client.reading.next(workspace, id);
      if (token !== epoch.current) return;
      remaining.current = page.articles;
    }
    const nextArticle = remaining.current.shift();
    if (token === epoch.current) selectUrl(nextArticle?.id ?? null);
  };
  const setReason = (value: string) => {
    if (!article || lock.current || uncertain) return;
    setReasonValue(value);
    const key = reasonKey(article.id);
    reasonDrafts.current.set(key, value);
    try {
      sessionStorage.setItem(key, value);
      setDraftError("");
    } catch {
      setDraftError(
        "This browser cannot retain your explanation after reload. Save it before leaving.",
      );
    }
  };
  const complete = () =>
    run("Saving rating", async (token) => {
      if (!article || !state || (score === null && !abstain)) return;
      if (reason.includes("\0"))
        throw Error("Explanation cannot contain U+0000.");
      const request =
        pending.current?.article === article.id
          ? pending.current
          : {
              article: article.id,
              command: {
                operationId: crypto.randomUUID(),
                expectedRevision: state.revision,
                rating: score,
                reason: reason === "" ? null : reason,
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
      const key = reasonKey(article.id);
      reasonDrafts.current.delete(key);
      try {
        sessionStorage.removeItem(key);
      } catch {
        /* The committed server copy is authoritative. */
      }
      setReasonValue("");
      if (result.undone) {
        setScore(result.state.rating ?? null);
        return;
      }
      setArticle({ ...article, read: true });
      setDeparting(true);
      // The original leaves inside a fixed viewport; controls never move. The
      // read receipt is committed first, so a failed save never animates away.
      if (!matchMedia("(prefers-reduced-motion: reduce)").matches)
        await new Promise<void>((resolve) => setTimeout(resolve, 240));
      try {
        if (token === epoch.current) await next(article.id, token);
      } finally {
        if (token === epoch.current) setDeparting(false);
      }
    });
  const completeCommits = (ids: string[]) =>
    run("Помечаю коммиты прочитанными", async (token) => {
      if (!article || !ids.length) return;
      setCommitFailures([]);
      const failures: string[] = [];
      let index = 0;
      await Promise.all(
        Array.from({ length: Math.min(4, ids.length) }, async () => {
          while (token === epoch.current && index < ids.length) {
            const id = ids[index++];
            try {
              await client.updateArticle(workspace, id, { read: true });
            } catch {
              failures.push(id);
            }
          }
        }),
      );
      if (token !== epoch.current) return;
      if (failures.length) {
        setCommitFailures(failures);
        throw Error(
          `Не удалось отметить ${failures.length} коммитов. Сохранённые отметки не потеряны; повторите действие.`,
        );
      }
      remaining.current = [];
      setState(await client.reading.state(workspace, article.id));
      setDeparting(true);
      if (!matchMedia("(prefers-reduced-motion: reduce)").matches)
        await new Promise<void>((resolve) => setTimeout(resolve, 240));
      try {
        if (token === epoch.current) await next(article.id, token);
      } finally {
        if (token === epoch.current) setDeparting(false);
      }
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
    session,
    article,
    state,
    score,
    setScore: (value: number) => {
      setAbstain(false);
      setScore(value);
    },
    abstain,
    setAbstain: () => {
      setScore(null);
      setAbstain(true);
    },
    departing,
    reason,
    setReason,
    draftError,
    busy,
    error,
    done,
    undo,
    uncertain,
    complete,
    completeCommits,
    commitFailures,
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
