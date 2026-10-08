import { createContext } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";
import { advanceSession, readSession } from "./readingSession";
import {
  restoreClock,
  sessionAt,
  sessionFromUrl,
  startClock,
  type ActiveSession,
} from "./sessionClock";
export function useSessionClock(
  owner: string,
  workspaces: readonly string[],
  route: string,
) {
  const key = `reading-active:${owner}`;
  const savedKey = (s: ActiveSession) =>
    `reading-session:${owner}:${s.workspace}:${s.session}`;
  const [error, setError] = useState("");
  const [active, setActive] = useState<ActiveSession | null>(null);
  const current = useRef(active);
  const currentOwner = useRef(owner);
  const [activeOwner, setActiveOwner] = useState(owner);
  const [now, setNow] = useState(Date.now());
  const shownNow = useRef(now);
  const failure = useRef("");
  const report = (message: string) => {
    failure.current = message;
    setError(message);
  };
  const persist = (next: ActiveSession | null) => {
    if (currentOwner.current !== owner) return;
    try {
      if (next) {
        sessionStorage.setItem(savedKey(next), JSON.stringify(next));
        sessionStorage.setItem(key, JSON.stringify(savedKey(next)));
      } else sessionStorage.removeItem(key);
      current.current = next;
      setActive(next);
      failure.current = "";
      shownNow.current = Date.now();
      setNow(shownNow.current);
      setError("");
    } catch {
      report(
        "Не удалось сохранить таймер. Состояние в хранилище сохранено; повторите действие.",
      );
    }
  };
  useEffect(() => {
    try {
      const pointer = JSON.parse(sessionStorage.getItem(key) ?? "null");
      if (
        pointer !== null &&
        (typeof pointer !== "string" ||
          !pointer.startsWith(`reading-session:${owner}:`))
      )
        throw Error("Сохранённый таймер повреждён; данные не перезаписаны.");
      const restored =
        pointer === null
          ? null
          : restoreClock(sessionStorage.getItem(pointer), workspaces);
      if (pointer !== null && (!restored || savedKey(restored) !== pointer))
        throw Error("Сохранённый таймер повреждён; данные не перезаписаны.");
      failure.current = "";
      setError("");
      currentOwner.current = owner;
      setActiveOwner(owner);
      current.current = restored;
      setActive(restored);
    } catch (cause) {
      currentOwner.current = owner;
      current.current = null;
      setActive(null);
      setActiveOwner(owner);
      report((cause as Error).message);
    }
  }, [key]);
  useEffect(() => {
    try {
      const incoming = sessionFromUrl(
        location.pathname + location.search,
        workspaces,
      );
      if (!incoming || failure.current || currentOwner.current !== owner)
        return;
      const existing = current.current;
      if (
        existing?.session === incoming.session &&
        existing.workspace === incoming.workspace
      ) {
        if (existing.url !== incoming.url)
          persist({
            ...existing,
            url: incoming.url,
            scroll:
              new URL(existing.url, location.origin).searchParams.get(
                "article",
              ) ===
              new URL(incoming.url, location.origin).searchParams.get("article")
                ? existing.scroll
                : 0,
          });
      } else {
        const progress = readSession(
          sessionStorage.getItem(savedKey(incoming)),
          incoming.plan,
        );
        if (!progress.finished)
          persist({
            ...incoming,
            progress: startClock(sessionAt(progress, Date.now()), Date.now()),
          });
      }
    } catch (cause) {
      report((cause as Error).message);
    }
  }, [route, location.search, key]);
  useEffect(() => {
    const tick = () => {
      const value = current.current?.progress;
      if (
        value &&
        !value.paused &&
        !value.finished &&
        value.deadline !== undefined &&
        shownNow.current < value.deadline
      ) {
        shownNow.current = Date.now();
        setNow(shownNow.current);
      }
    };
    const id = window.setInterval(tick, 250);
    window.addEventListener("focus", tick);
    document.addEventListener("visibilitychange", tick);
    const scroll = (event: Event) => {
      const el = event.target;
      const value = current.current;
      if (
        !(el instanceof HTMLElement) ||
        !el.closest(".focused-reading__article") ||
        !value ||
        location.pathname !== "/reading" ||
        location.pathname + location.search !== value.url
      )
        return;
      persist({ ...value, scroll: el.scrollTop });
    };
    document.addEventListener("scroll", scroll, true);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", tick);
      document.removeEventListener("visibilitychange", tick);
      document.removeEventListener("scroll", scroll, true);
    };
  }, [key]);
  const mutate = (change: (value: ActiveSession) => ActiveSession) => {
    const value = current.current;
    if (!value || failure.current || currentOwner.current !== owner)
      return null;
    const next = change({
      ...value,
      progress: sessionAt(value.progress, Date.now()),
    });
    persist(next);
    return next.progress;
  };
  return {
    active:
      active && activeOwner === owner
        ? { ...active, progress: sessionAt(active.progress, now) }
        : null,
    error,
    advance: () =>
      mutate((v) => ({
        ...v,
        progress: startClock(advanceSession(v.progress, v.plan), Date.now()),
      })),
    toggle: () =>
      mutate((v) => ({
        ...v,
        progress: startClock(
          { ...v.progress, paused: !v.progress.paused, deadline: undefined },
          Date.now(),
        ),
      })),
    stop: () => {
      const v = current.current;
      if (!v || failure.current || currentOwner.current !== owner) return;
      const stopped = {
        ...v,
        progress: {
          ...sessionAt(v.progress, Date.now()),
          remainingMs: 0,
          finished: true,
          stopped: true,
          deadline: undefined,
        },
      };
      persist(stopped);
      if (current.current === stopped) persist(null);
    },
  };
}
export type SessionClock = ReturnType<typeof useSessionClock>;
export const SessionClockContext = createContext<SessionClock | null>(null);
