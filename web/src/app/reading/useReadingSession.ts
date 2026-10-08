import { useContext } from "preact/hooks";
import { initialSession, readSession } from "./readingSession";
import { sessionFromUrl, sessionAt } from "./sessionClock";
import { SessionClockContext } from "./useSessionClock";
export function useReadingSession(owner: string, workspace: string) {
  const clock = useContext(SessionClockContext);
  const q = new URLSearchParams(location.search),
    enabled = q.get("from") === "session",
    random = q.get("from") === "random";
  let error = "",
    plan = { smart: 0, random: 1 };
  let progress = initialSession(plan);
  try {
    if (enabled) {
      const descriptor = sessionFromUrl(location.pathname + location.search, [
        workspace,
      ]);
      if (!descriptor) throw Error("Некорректная сессия чтения.");
      plan = descriptor.plan;
      progress =
        clock?.active?.session === q.get("session") &&
        clock.active.workspace === workspace
          ? clock.active.progress
          : sessionAt(
              readSession(
                sessionStorage.getItem(
                  `reading-session:${owner}:${workspace}:${q.get("session")}`,
                ),
                plan,
              ),
              Date.now(),
            );
      if (!clock) throw Error("Таймер сессии недоступен.");
    }
    if (
      (enabled || random) &&
      !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        q.get("seed") ?? "",
      )
    )
      throw Error("Сессии нужен корректный seed.");
  } catch (cause) {
    error = (cause as Error).message;
  }
  return {
    enabled,
    random,
    seed: q.get("seed") ?? "",
    progress,
    error: error || (enabled ? (clock?.error ?? "") : ""),
    storageError: clock?.error ?? "",
    mode: enabled
      ? progress.mode
      : random
        ? ("random" as const)
        : ("smart" as const),
    advance: () => clock?.advance() ?? progress,
    toggle: () => clock?.toggle(),
  };
}
