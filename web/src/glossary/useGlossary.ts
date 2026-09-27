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
  const load = async (id: string, title: string, retry = false) => {
    if (locked.current) return;
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
        (!value.job || retry) &&
        value.channel.indexReady &&
        value.channel.generationAllowed
      ) {
        value = await client.generate(
          workspace,
          id,
          crypto.randomUUID(),
          retry,
        );
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
    error,
    close,
    open: load,
    retry: () => target && load(target.id, target.title, true),
  };
}
