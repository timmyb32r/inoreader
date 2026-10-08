import {
  initialSession,
  readingPlan,
  readSession,
  type ReadingPlan,
  type SessionProgress,
} from "./readingSession";
export type ActiveSession = {
  workspace: string;
  session: string;
  seed: string;
  plan: ReadingPlan;
  progress: SessionProgress;
  url: string;
  scroll: number;
};
const uuid = (v: string | null): v is string =>
  !!v &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
/** Account-owned browser state only. A deadline is absolute Unix milliseconds;
 * phase expiry waits for explicit article completion. No background API writes. */
export function sessionAt(
  value: SessionProgress,
  now: number,
): SessionProgress {
  return value.paused || value.finished || value.deadline === undefined
    ? value
    : { ...value, remainingMs: Math.max(0, value.deadline - now) };
}
export function startClock(
  value: SessionProgress,
  now: number,
): SessionProgress {
  if (value.paused || value.finished || value.remainingMs === 0)
    return { ...value, deadline: undefined };
  const deadline = now + value.remainingMs;
  if (!Number.isSafeInteger(deadline))
    throw Error("Длительность слишком велика для таймера.");
  return { ...value, deadline };
}
export function sessionFromUrl(
  url: string,
  workspaces: readonly string[],
): ActiveSession | null {
  const parsed = new URL(url, location.origin),
    q = parsed.searchParams;
  if (
    parsed.origin !== location.origin ||
    parsed.pathname !== "/reading" ||
    q.get("from") !== "session"
  )
    return null;
  const workspace = q.get("workspace"),
    session = q.get("session"),
    seed = q.get("seed");
  if (
    !workspace ||
    !workspaces.includes(workspace) ||
    !uuid(session) ||
    !uuid(seed)
  )
    throw Error("Некорректная сессия чтения.");
  const plan = readingPlan(q.get("smart") ?? "", q.get("random") ?? "");
  return {
    workspace,
    session,
    seed,
    plan,
    progress: initialSession(plan),
    url: parsed.pathname + parsed.search,
    scroll: 0,
  };
}
export function restoreClock(
  raw: string | null,
  workspaces: readonly string[],
): ActiveSession | null {
  if (!raw) return null;
  const saved = JSON.parse(raw);
  if (
    typeof saved?.url !== "string" ||
    typeof saved.scroll !== "number" ||
    !Number.isFinite(saved.scroll) ||
    saved.scroll < 0
  )
    throw Error("Сохранённый таймер повреждён; данные не перезаписаны.");
  const descriptor = sessionFromUrl(saved.url, workspaces);
  if (
    !descriptor ||
    descriptor.workspace !== saved.workspace ||
    descriptor.session !== saved.session ||
    descriptor.seed !== saved.seed ||
    saved.plan?.smart !== descriptor.plan.smart ||
    saved.plan?.random !== descriptor.plan.random
  )
    throw Error("Сохранённый таймер повреждён; данные не перезаписаны.");
  return {
    ...descriptor,
    scroll: saved.scroll,
    progress: readSession(JSON.stringify(saved), descriptor.plan),
  };
}
