import { useEffect, useMemo, useState } from "preact/hooks";

export type DailyActivity = Record<string, number>;

const FLUSH_INTERVAL_MS = 15_000;

function dateKey(date: Date): string {
  return date.toISOString().slice(0, 10);
}

export function activityStorageKey(accountId: string): string {
  return `reader.activity.${accountId}`;
}

function browserStorage(): Storage | undefined {
  return typeof window === "undefined" ? undefined : window.localStorage ?? undefined;
}

export function readActivity(accountId: string): DailyActivity {
  try {
    const value = JSON.parse(browserStorage()?.getItem(activityStorageKey(accountId)) ?? "{}");
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(Object.entries(value).filter((entry): entry is [string, number] => /^\d{4}-\d{2}-\d{2}$/.test(entry[0]) && typeof entry[1] === "number" && entry[1] >= 0));
  } catch {
    return {};
  }
}

export function recordActivity(accountId: string, elapsedMs: number, now = new Date()): DailyActivity {
  const activity = readActivity(accountId);
  const day = dateKey(now);
  activity[day] = (activity[day] ?? 0) + Math.max(0, elapsedMs);
  browserStorage()?.setItem(activityStorageKey(accountId), JSON.stringify(activity));
  return activity;
}

export function useActivityTracker(accountId: string): DailyActivity {
  const [activity, setActivity] = useState<DailyActivity>(() => accountId ? readActivity(accountId) : {});
  useEffect(() => {
    if (!accountId) return;
    setActivity(readActivity(accountId));
    let startedAt = document.visibilityState === "visible" && document.hasFocus() ? Date.now() : undefined;
    const flush = () => {
      if (startedAt === undefined) return;
      const now = Date.now();
      setActivity(recordActivity(accountId, now - startedAt, new Date(now)));
      startedAt = now;
    };
    const sync = () => {
      const active = document.visibilityState === "visible" && document.hasFocus();
      if (active && startedAt === undefined) startedAt = Date.now();
      if (!active && startedAt !== undefined) { flush(); startedAt = undefined; }
    };
    const interval = window.setInterval(flush, FLUSH_INTERVAL_MS);
    document.addEventListener("visibilitychange", sync);
    window.addEventListener("focus", sync);
    window.addEventListener("blur", sync);
    window.addEventListener("pagehide", flush);
    return () => {
      flush();
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", sync);
      window.removeEventListener("focus", sync);
      window.removeEventListener("blur", sync);
      window.removeEventListener("pagehide", flush);
    };
  }, [accountId]);
  return activity;
}

type Day = { key: string; label: string; minutes: number; level: number };

function activityDays(activity: DailyActivity, today = new Date()): Day[] {
  const end = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()));
  const start = new Date(end);
  start.setUTCDate(start.getUTCDate() - 363);
  const values: Day[] = [];
  for (let cursor = new Date(start); cursor <= end; cursor.setUTCDate(cursor.getUTCDate() + 1)) {
    const key = dateKey(cursor);
    const minutes = Math.floor((activity[key] ?? 0) / 60_000);
    const level = minutes === 0 ? 0 : minutes < 15 ? 1 : minutes < 45 ? 2 : minutes < 90 ? 3 : 4;
    values.push({ key, minutes, level, label: cursor.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric", timeZone: "UTC" }) });
  }
  return values;
}

export function ActivityDashboard({ activity, workspaceName, onOpenLibrary }: { activity: DailyActivity; workspaceName: string; onOpenLibrary: () => void }) {
  const days = useMemo(() => activityDays(activity), [activity]);
  const totalMinutes = days.reduce((sum, day) => sum + day.minutes, 0);
  const activeDays = days.filter((day) => day.minutes > 0).length;
  return <section class="home-dashboard" aria-labelledby="home-title">
    <header class="home-dashboard__header"><div><p class="eyebrow">{workspaceName}</p><h1 id="home-title">Your reading activity</h1><p>Time spent reading and maintaining your library on this device.</p></div><button class="primary-button" onClick={onOpenLibrary}>Open library</button></header>
    <div class="activity-summary"><article><strong>{totalMinutes}</strong><span>minutes this year</span></article><article><strong>{activeDays}</strong><span>active days</span></article></div>
    <section class="activity-card" aria-label="Reading activity for the last year"><div class="activity-card__heading"><div><h2>Time on Reader</h2><p>Each tile is one day. Darker tiles mean more minutes.</p></div><div class="activity-legend" aria-label="Activity intensity"><span>Less</span>{[0,1,2,3,4].map(level=><i class={`activity-tile activity-tile--${level}`}/>)}<span>More</span></div></div><div class="activity-calendar" role="grid" aria-label="Daily minutes"><div class="activity-calendar__days" aria-hidden="true"><span>Mon</span><span>Wed</span><span>Fri</span></div><div class="activity-calendar__grid">{days.map(day=><span role="gridcell" class={`activity-tile activity-tile--${day.level}`} title={`${day.label}: ${day.minutes} minutes`} aria-label={`${day.label}: ${day.minutes} minutes`}/>)}</div></div></section>
  </section>;
}
