import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import type { DefinitionsView, GlossaryClient } from "../api/glossary";

export function useGlossary(
  client: GlossaryClient,
  scope: string,
  workspace: string,
) {
  const [target, setTarget] = useState<{ id: string; title: string } | null>(
    null,
  );
  const [view, setView] = useState<DefinitionsView | null>(null);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const [pollTick, setPollTick] = useState(0);
  // An uncertain POST remains the same paid intent across close/reopen/retry.
  const intents = useRef(
    new Map<string, { operation: string; regenerate: boolean }>(),
  );
  const revision = useRef(0),
    locked = useRef(false),
    origin = useRef<HTMLElement | null>(null);
  const close = () => {
    revision.current++;
    locked.current = false;
    setBusy(false);
    setTarget(null);
    setView(null);
    setError("");
    const element = origin.current;
    requestAnimationFrame(() => {
      if (element?.isConnected) element.focus();
    });
  };
  useLayoutEffect(() => {
    close();
  }, [scope, workspace]);
  const load = async (
    id: string,
    title: string,
    retry = false,
    requestGeneration = true,
  ) => {
    if (locked.current) return;
    if (!retry && target?.id === id && view?.job?.status === "completed") {
      setTarget({ id, title });
      return;
    }
    const intentKey = JSON.stringify([scope, workspace, id]);
    const token = ++revision.current;
    locked.current = true;
    origin.current = document.activeElement as HTMLElement;
    setTarget({ id, title });
    setBusy(true);
    setError("");
    if (!retry) setView(null);
    try {
      let value = await client.get(workspace, id);
      if (token !== revision.current) return;
      setView(value);
      if (
        requestGeneration &&
        (!value.job ||
          value.job.status === "queued" ||
          retry ||
          intents.current.has(intentKey)) &&
        value.channel.indexReady &&
        value.channel.generationAllowed
      ) {
        const intent = intents.current.get(intentKey) ?? {
          operation: crypto.randomUUID(),
          regenerate: retry,
        };
        intents.current.set(intentKey, intent);
        value = await client.generate(
          workspace,
          id,
          intent.operation,
          intent.regenerate,
        );
        intents.current.delete(intentKey);
        if (token !== revision.current) return;
        setView(value);
      }
    } catch (e) {
      if (token === revision.current) setError((e as Error).message);
    } finally {
      if (token === revision.current) {
        locked.current = false;
        setBusy(false);
      }
    }
  };
  const pending =
    busy ||
    view?.job?.status === "queued" ||
    view?.job?.status === "generating";
  useEffect(() => {
    if (!target || busy || !pending) return;
    const token = revision.current;
    const timer = window.setTimeout(() => {
      client
        .get(workspace, target.id)
        .then((value) => {
          if (token === revision.current) {
            setView(value);
            setError("");
          }
        })
        .catch((e) => {
          if (token === revision.current) setError(e.message);
        })
        .finally(() => {
          if (token === revision.current) setPollTick((n) => n + 1);
        });
    }, 1000);
    return () => clearTimeout(timer);
  }, [view, target, busy, error, workspace, pollTick]);
  return {
    target,
    view,
    busy: !!pending,
    actionBusy: busy || view?.job?.status === "generating",
    error,
    close,
    open: (id: string, title: string, requestGeneration = true) =>
      load(id, title, false, requestGeneration),
    retry: () => target && load(target.id, target.title, true),
  };
}
