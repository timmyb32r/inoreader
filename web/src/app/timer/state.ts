/** Browser-local, account-scoped session timer. Milliseconds; running state stores
 * an absolute deadline, paused state stores a remainder. No daily accumulation. */
export type TimerState =
  | { mode: "idle" | "finished"; duration: number }
  | { mode: "running"; duration: number; deadline: number }
  | { mode: "paused"; duration: number; remaining: number };
export const initialTimer: TimerState = { mode: "idle", duration: 3600000 };
export function remaining(state: TimerState, now: number): number {
  return state.mode === "running"
    ? Math.max(0, state.deadline - now)
    : state.mode === "paused"
      ? state.remaining
      : state.mode === "idle"
        ? state.duration
        : 0;
}
export function formatDuration(ms: number): string {
  const seconds = Math.ceil(ms / 1000);
  return [
    Math.floor(seconds / 3600),
    Math.floor(seconds / 60) % 60,
    seconds % 60,
  ]
    .map((n) => String(n).padStart(2, "0"))
    .join(":");
}
/** Explicit editor range: 00:00:01–99:59:59, whole seconds. */
export function parseDuration(value: string): number {
  if (!/^\d{2}:[0-5]\d:[0-5]\d$/.test(value))
    throw Error("Use HH:MM:SS (00:00:01–99:59:59)");
  const [h, m, s] = value.split(":").map(Number);
  const duration = (h * 3600 + m * 60 + s) * 1000;
  if (!duration) throw Error("Duration must be greater than zero");
  return duration;
}
export function readTimer(raw: string | null): TimerState {
  if (raw === null) return initialTimer;
  const v = JSON.parse(raw);
  const positive = (n: unknown): n is number =>
    Number.isSafeInteger(n) && Number(n) > 0;
  if (
    !v ||
    !positive(v.duration) ||
    v.duration > 359999000 ||
    v.duration % 1000 !== 0
  )
    throw Error("Invalid saved timer");
  if (v.mode === "idle" || v.mode === "finished")
    return { mode: v.mode, duration: v.duration };
  if (v.mode === "running" && positive(v.deadline))
    return { mode: v.mode, duration: v.duration, deadline: v.deadline };
  if (v.mode === "paused" && positive(v.remaining) && v.remaining <= v.duration)
    return { mode: v.mode, duration: v.duration, remaining: v.remaining };
  throw Error("Invalid saved timer");
}
