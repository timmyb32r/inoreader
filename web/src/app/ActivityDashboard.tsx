import { useEffect, useMemo, useState } from "preact/hooks";

import { ReadingFlowChart } from "./ReadingFlowChart";

export type DailyActivity = Record<string, { count: number; arrived: number }>;

import type { ApiClient } from "../api/client";

function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

type Day = {
  key: string;
  label: string;
  count: number;
  arrived: number;
  level: number;
  weekday: number;
};

function activityDays(activity: DailyActivity, today = new Date()): Day[] {
  const end = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate(),
    12,
  );
  const start = new Date(end);
  start.setDate(start.getDate() - 363);
  const values: Day[] = [];
  for (
    let cursor = new Date(start);
    cursor <= end;
    cursor.setDate(cursor.getDate() + 1)
  ) {
    const key = dateKey(cursor);
    const count = activity[key]?.count ?? 0;
    const level =
      count === 0 ? 0 : count < 5 ? 1 : count < 15 ? 2 : count < 30 ? 3 : 4;
    values.push({
      key,
      count,
      arrived: activity[key]?.arrived ?? 0,
      level,
      weekday: ((cursor.getDay() + 6) % 7) + 1,
      label: cursor.toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric",
      }),
    });
  }
  return values;
}

export function ActivityDashboard({
  client,
  workspaceId,
  workspaceName,
  onOpenLibrary,
  onReadDay,
  onArrivedDay,
  opening = false,
}: {
  client: ApiClient;
  workspaceId: string;
  workspaceName: string;
  onOpenLibrary: () => void;
  onReadDay?: (day: string) => void;
  onArrivedDay?: (day: string) => void;
  opening?: boolean;
}) {
  const [activity, setActivity] = useState<DailyActivity | null>(null);
  const [error, setError] = useState(false);
  const [today, setToday] = useState(() => new Date());
  useEffect(() => {
    let disposed = false;
    let pending = false;
    setActivity(null);
    setError(false);
    const load = async () => {
      if (pending) return;
      pending = true;
      try {
        const result = await client.readingActivity(
          workspaceId,
          Intl.DateTimeFormat().resolvedOptions().timeZone,
        );
        if (!disposed) {
          setActivity(
            Object.fromEntries(
              result.days.map((day) => [
                day.day,
                { count: day.count, arrived: day.arrived },
              ]),
            ),
          );
          setToday(new Date());
          setError(false);
        }
      } catch {
        if (!disposed) setError(true);
      } finally {
        pending = false;
      }
    };
    void load();
    const refresh = () => {
      if (document.visibilityState === "visible") void load();
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    const interval = window.setInterval(refresh, 60_000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [client, workspaceId]);
  const days = useMemo(
    () => activityDays(activity ?? {}, today),
    [activity, today],
  );
  const countToday = activity?.[dateKey(today)]?.count ?? 0;
  const arrivedToday = activity?.[dateKey(today)]?.arrived ?? 0;
  return (
    <section class="home-dashboard" aria-labelledby="home-title">
      <header class="home-dashboard__header">
        <div>
          <p class="eyebrow">{workspaceName}</p>
          <h1 id="home-title">Your reading activity</h1>
          <p>New arrivals and articles you marked as read, day by day.</p>
        </div>
        <button class="primary-button" onClick={onOpenLibrary}>
          Начать чтение
        </button>
      </header>
      <div class="activity-summary">
        <button
          class="activity-summary__read"
          disabled={!activity || opening || !onReadDay}
          aria-busy={opening}
          onClick={() => onReadDay?.(dateKey(today))}
        >
          <strong>{activity ? countToday : "—"}</strong>
          <span>articles marked read today</span>
          <span class="activity-summary__link">
            {opening ? "Opening…" : "View articles →"}
          </span>
        </button>
        <button
          class="activity-summary__read"
          disabled={!activity || !onArrivedDay}
          onClick={() => onArrivedDay?.(dateKey(today))}
        >
          <strong>{activity ? arrivedToday : "—"}</strong>
          <span>articles arrived today</span>
          <span class="activity-summary__link">Explore sources →</span>
        </button>
      </div>
      <ReadingFlowChart
        days={days}
        onReadDay={onReadDay}
        opening={opening}
        status={activity ? "ready" : error ? "error" : "loading"}
      />
      <section
        class="activity-card"
        aria-label="Reading activity for the last year"
      >
        <div class="activity-card__heading">
          <div>
            <h2>Articles marked read</h2>
            <p>Each tile is one day. Darker tiles mean more articles.</p>
          </div>
          <div class="activity-legend" aria-label="Activity intensity">
            <span>Less</span>
            {[0, 1, 2, 3, 4].map((level) => (
              <i class={`activity-tile activity-tile--${level}`} />
            ))}
            <span>More</span>
          </div>
        </div>
        <div
          class="activity-calendar"
          role="grid"
          aria-label="Daily articles marked read"
          aria-busy={activity === null && !error}
        >
          <div class="activity-calendar__days" aria-hidden="true">
            <span>Mon</span>
            <span>Wed</span>
            <span>Fri</span>
          </div>
          <div class="activity-calendar__grid">
            {days.map((day, index) => (
              <button
                disabled={!activity || opening || !onReadDay}
                aria-busy={opening}
                onClick={() => onReadDay?.(day.key)}
                role="gridcell"
                style={index === 0 ? { gridRowStart: day.weekday } : undefined}
                class={`activity-tile activity-tile--${day.level}`}
                title={`${day.label}: ${activity ? `${day.count} articles` : "not loaded"}`}
                aria-label={`${day.label}: ${activity ? `${day.count} articles` : "not loaded"}`}
              />
            ))}
          </div>
        </div>
        <p class="activity-status" role="status">
          {error
            ? "Could not load reading activity. Return to this page to retry."
            : "Recorded since tracking was enabled. Days use your local timezone."}
        </p>
      </section>
    </section>
  );
}
