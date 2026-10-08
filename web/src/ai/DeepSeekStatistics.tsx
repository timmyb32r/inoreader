import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import type { AiClient } from "../api/ai";
import type { AiRequestStatistics } from "../api/generated";
import { AutofillResistantField as Field } from "../ui/fields";
import {
  isoDate,
  labels,
  modes,
  moscowToday,
  periodDates,
  sumUsd,
  validDates,
  type Bucket,
} from "./requestStatistics";
import "./statistics.css";

type Range = { from: string; until: string; bucket: Bucket };
const bucketLabels: Record<Bucket, string> = {
  day: "По дням",
  month: "По месяцам",
  year: "По годам",
};
function preset(bucket: Bucket): Range {
  const until = moscowToday(),
    date = new Date(`${until}T00:00:00Z`);
  if (bucket === "day") date.setUTCDate(date.getUTCDate() - 29);
  else if (bucket === "month") {
    date.setUTCDate(1);
    date.setUTCMonth(date.getUTCMonth() - 11);
  } else {
    date.setUTCMonth(0, 1);
    date.setUTCFullYear(date.getUTCFullYear() - 4);
  }
  return { from: isoDate(date), until, bucket };
}
function initialRange(): Range {
  const q = new URLSearchParams(location.search),
    bucket = q.get("bucket");
  const selected: Bucket =
    bucket === "month" || bucket === "year" ? bucket : "day";
  const defaults = preset(selected);
  return {
    from: q.get("from") ?? defaults.from,
    until: q.get("until") ?? defaults.until,
    bucket: selected,
  };
}
export function DeepSeekStatistics({
  client,
  allowed,
}: {
  client: AiClient;
  allowed: boolean;
}) {
  const [draft, setDraft] = useState<Range>(initialRange),
    [range, setRange] = useState<Range>(initialRange);
  const [data, setData] = useState<AiRequestStatistics | null>(null),
    [pending, setPending] = useState(true),
    [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const lock = useRef(true);
  useEffect(() => {
    if (!allowed) {
      lock.current = false;
      setPending(false);
      return;
    }
    let active = true;
    lock.current = true;
    setPending(true);
    setError("");
    if (!validDates(range.from, range.until)) {
      setError("Укажите корректный диапазон дат");
      setPending(false);
      lock.current = false;
      return;
    }
    client
      .statistics(range)
      .then((value) => {
        if (active) setData(value);
      })
      .catch((e) => {
        if (active) setError(e.message);
      })
      .finally(() => {
        if (active) {
          lock.current = false;
          setPending(false);
        }
      });
    return () => {
      active = false;
    };
  }, [client, allowed, range, revision]);
  const apply = (value: Range) => {
    if (lock.current) return;
    if (!validDates(value.from, value.until)) {
      setError("Укажите корректный диапазон дат");
      return;
    }
    lock.current = true;
    setPending(true);
    setError("");
    setData(null);
    setDraft(value);
    setRange(value);
    setRevision((n) => n + 1);
    history.replaceState(
      history.state,
      "",
      `/ai-statistics?${new URLSearchParams(value)}`,
    );
  };
  const periods = useMemo(
    () =>
      validDates(range.from, range.until)
        ? periodDates(range.from, range.until, range.bucket)
        : [],
    [range],
  );
  const grouped = useMemo(() => {
    const map = new Map<string, AiRequestStatistics["rows"]>();
    for (const row of data?.rows ?? [])
      map.set(row.period, [...(map.get(row.period) ?? []), row]);
    return map;
  }, [data]);
  const total = (data?.rows ?? []).reduce((sum, row) => sum + row.requests, 0);
  const unknown = (data?.rows ?? []).reduce(
    (sum, row) => sum + row.unconfirmed,
    0,
  );
  const maximum = Math.max(
    1,
    ...[...grouped.values()].map((rows) =>
      rows.reduce((sum, row) => sum + row.requests, 0),
    ),
  );
  const byMode = modes.map((mode) => ({
    mode,
    count: (data?.rows ?? [])
      .filter((row) => row.mode === mode)
      .reduce((sum, row) => sum + row.requests, 0),
  }));
  const drill = (period: string) => {
    if (range.bucket === "day") return;
    const end = new Date(`${period}T00:00:00Z`);
    if (range.bucket === "year") end.setUTCFullYear(end.getUTCFullYear() + 1);
    else end.setUTCMonth(end.getUTCMonth() + 1);
    end.setUTCDate(end.getUTCDate() - 1);
    apply({
      from: period < range.from ? range.from : period,
      until: isoDate(end) > range.until ? range.until : isoDate(end),
      bucket: range.bucket === "year" ? "month" : "day",
    });
  };
  if (!allowed)
    return (
      <section class="deepseek-statistics">
        <h1>Нет доступа</h1>
        <p>Статистика DeepSeek доступна только timmyb32r.</p>
      </section>
    );
  return (
    <section
      class="deepseek-statistics"
      aria-label="Статистика DeepSeek"
      aria-busy={pending}
    >
      <header class="ds-heading">
        <div>
          <p class="eyebrow">PRIVATE ADMIN · TIMMYB32R</p>
          <h1>
            DeepSeek <span> / статистика</span>
          </h1>
          <p>Запросы, типы задач и расходы — в одном месте.</p>
        </div>
        <span class="ds-timezone">Europe/Moscow · UTC+3</span>
      </header>
      <div class="ds-controls">
        <div class="ds-segments" aria-label="Группировка">
          {(["day", "month", "year"] as Bucket[]).map((bucket) => (
            <button
              disabled={pending}
              aria-pressed={range.bucket === bucket}
              onClick={() => apply(preset(bucket))}
            >
              {bucketLabels[bucket]}
            </button>
          ))}
        </div>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            apply(draft);
          }}
        >
          <label>
            С
            <Field
              type="date"
              aria-label="Начало периода"
              value={draft.from}
              disabled={pending}
              onInput={(event) =>
                setDraft({ ...draft, from: event.currentTarget.value })
              }
            />
          </label>
          <label>
            По
            <Field
              type="date"
              aria-label="Конец периода"
              value={draft.until}
              disabled={pending}
              onInput={(event) =>
                setDraft({ ...draft, until: event.currentTarget.value })
              }
            />
          </label>
          <button
            class="primary-button"
            disabled={pending}
            aria-busy={pending}
            type="submit"
            aria-label="Показать"
          >
            {pending ? "Загрузка…" : "Показать"}
          </button>
        </form>
        <button
          class="secondary-button"
          disabled={pending}
          onClick={() =>
            apply({ from: moscowToday(), until: moscowToday(), bucket: "day" })
          }
        >
          Сегодня
        </button>
      </div>
      <div class="ds-status" role="status">
        {error ||
          (pending
            ? "Загружаем статистику…"
            : `${range.from} — ${range.until} · ${bucketLabels[range.bucket].toLowerCase()}`)}
      </div>
      <div class="ds-kpis">
        <article>
          <span>Учтённые API-попытки</span>
          <strong>{data ? total.toLocaleString("ru") : "—"}</strong>
          <small>Каждый вызов, включая повторные</small>
        </article>
        <article>
          <span>Расход с известным биллингом</span>
          <strong>
            {data ? `$${sumUsd(data.rows.map((row) => row.spentUsd))}` : "—"}
          </strong>
          <small>Точные суммы из журнала</small>
        </article>
        <article>
          <span>Без подтверждения биллинга</span>
          <strong>{data ? unknown.toLocaleString("ru") : "—"}</strong>
          <small>
            {data
              ? `$${sumUsd(data.rows.map((row) => row.reservedUsd))} зарезервировано`
              : "—"}
          </small>
        </article>
      </div>
      <article class="ds-card">
        <div class="ds-card-heading">
          <h2>Динамика запросов</h2>
          <span>
            {range.bucket === "day"
              ? "Наведение / фокус — точные значения"
              : "Нажмите период, чтобы раскрыть"}
          </span>
        </div>
        <div class="ds-chart-scroll">
          <div
            class="ds-chart"
            style={{ minWidth: `${Math.max(560, periods.length * 30)}px` }}
          >
            {periods.map((period) => {
              const rows = grouped.get(period) ?? [],
                count = rows.reduce((sum, row) => sum + row.requests, 0);
              return (
                <button
                  class="ds-column"
                  disabled={pending}
                  onClick={() => drill(period)}
                  aria-label={`${period}: ${data ? count : "нет данных"} запросов`}
                >
                  <div
                    class="ds-stack"
                    style={{
                      height: `${data ? (count / maximum) * 200 : 0}px`,
                    }}
                  >
                    {modes.map((mode, i) => (
                      <div
                        class={`ds-mode-${i}`}
                        style={{
                          height: `${count ? ((rows.find((row) => row.mode === mode)?.requests ?? 0) / count) * 100 : 0}%`,
                        }}
                      />
                    ))}
                  </div>
                  <span class="ds-period">
                    {range.bucket === "day"
                      ? period.slice(5)
                      : range.bucket === "month"
                        ? period.slice(0, 7)
                        : period.slice(0, 4)}
                  </span>
                  {data && (
                    <span class="ds-tooltip">
                      <b>
                        {period} · {count} запросов
                      </b>
                      {modes.map((mode) => (
                        <span>
                          {labels[mode]}:{" "}
                          {rows.find((row) => row.mode === mode)?.requests ?? 0}
                        </span>
                      ))}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
        <div class="ds-legend">
          {modes.map((mode, i) => (
            <span>
              <i class={`ds-mode-${i}`} />
              {labels[mode]}
            </span>
          ))}
        </div>
      </article>
      <div class="ds-bottom">
        <article class="ds-card">
          <h2>Распределение по задачам</h2>
          <div class="ds-breakdown">
            {byMode.map(({ mode, count }, i) => (
              <div>
                <span>{labels[mode]}</span>
                <b>{data ? count.toLocaleString("ru") : "—"}</b>
                <div class="ds-track">
                  <i
                    class={`ds-mode-${i}`}
                    style={{ width: `${total ? (count / total) * 100 : 0}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </article>
        <article class="ds-card ds-detail">
          <h2>Журнал по периодам</h2>
          <div class="ds-table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Период / задача</th>
                  <th>Запросы</th>
                  <th>Без биллинга</th>
                  <th>Расход, USD</th>
                  <th>Резерв, USD</th>
                </tr>
              </thead>
              <tbody>
                {(data?.rows ?? []).map((row) => (
                  <tr>
                    <td>
                      {row.period}
                      <small>{labels[row.mode]}</small>
                    </td>
                    <td>{row.requests}</td>
                    <td>{row.unconfirmed}</td>
                    <td>{row.spentUsd}</td>
                    <td>{row.reservedUsd}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {data && !data.rows.length && (
              <p class="ds-empty">В этом периоде запросов нет.</p>
            )}
          </div>
        </article>
      </div>
      <p class="ds-note">
        Одна статья может вызвать несколько запросов. Здесь учитываются
        отдельные платные попытки из журнала приложения, включая ошибки и
        повторные вызовы. Попытка без биллинга не подтверждает получение запроса
        провайдером; её резерв не считается известным расходом. Периоды —
        включительно, по московской дате допуска к вызову.
      </p>
    </section>
  );
}
