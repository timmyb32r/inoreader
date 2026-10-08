export type ReadingMode = "smart" | "random";
export type ReadingPlan = { smart: number; random: number };
/** Minutes are explicit non-negative safe integers. At least one phase is positive;
 * conversion to milliseconds must retain exact integer precision. Local UI-only
 * execution state is never sent to the server or used for authorization/billing. */
export function readingPlan(smart: string, random: string): ReadingPlan {
  const values = [smart, random].map((value) => {
    if (!/^\d+$/.test(value))
      throw Error("Введите целое число минут, не меньше нуля.");
    const minutes = Number(value);
    if (!Number.isSafeInteger(minutes * 60000))
      throw Error("Длительность слишком велика.");
    return minutes;
  });
  if (values.every((value) => value === 0))
    throw Error("Выделите время хотя бы одному режиму.");
  return { smart: values[0], random: values[1] };
}
export function sessionUrl(workspace: string, plan: ReadingPlan): string {
  return `/reading?${new URLSearchParams({ workspace, from: "session", session: crypto.randomUUID(), seed: crypto.randomUUID(), smart: String(plan.smart), random: String(plan.random) })}`;
}
export type SessionProgress = {
  mode: ReadingMode;
  remainingMs: number;
  paused: boolean;
  finished: boolean;
  deadline?: number;
  stopped?: boolean;
};
export function initialSession(plan: ReadingPlan): SessionProgress {
  const mode = plan.smart > 0 ? "smart" : "random";
  return {
    mode,
    remainingMs: plan[mode] * 60000,
    paused: false,
    finished: false,
  };
}
export function readSession(
  raw: string | null,
  plan: ReadingPlan,
): SessionProgress {
  if (!raw) return initialSession(plan);
  const saved = JSON.parse(raw);
  const value = saved.progress;
  if (
    saved.plan?.smart !== plan.smart ||
    saved.plan?.random !== plan.random ||
    !value ||
    !["smart", "random"].includes(value.mode) ||
    !Number.isSafeInteger(value.remainingMs) ||
    value.remainingMs < 0 ||
    value.remainingMs > plan[value.mode as ReadingMode] * 60000 ||
    typeof value.paused !== "boolean" ||
    typeof value.finished !== "boolean" ||
    (value.finished && value.remainingMs !== 0) ||
    plan[value.mode as ReadingMode] === 0 ||
    (value.finished &&
      value.mode === "smart" &&
      plan.random > 0 &&
      !value.stopped) ||
    (value.stopped !== undefined && typeof value.stopped !== "boolean") ||
    (value.deadline !== undefined &&
      (!Number.isSafeInteger(value.deadline) || value.deadline <= 0))
  )
    throw Error("Сохранённая сессия повреждена; она не перезаписана.");
  return value;
}
/** Expiry is applied only at the explicit next-article boundary. */
export function advanceSession(
  value: SessionProgress,
  plan: ReadingPlan,
): SessionProgress {
  if (value.finished || value.remainingMs > 0) return value;
  if (value.mode === "smart" && plan.random > 0)
    return {
      ...value,
      mode: "random",
      remainingMs: plan.random * 60000,
      deadline: undefined,
    };
  return { ...value, finished: true };
}
