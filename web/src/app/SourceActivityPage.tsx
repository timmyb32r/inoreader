import { useEffect, useMemo, useState } from "preact/hooks";
import type { ApiClient } from "../api/client";
import type { SourceActivityView } from "../api/generated";
import { AutofillResistantField as Field } from "../ui/fields";
import { localDay, readPeriodForDay } from "./readPeriod";
import { sourceTreemap } from "./sourceTreemap";
import "./source-activity.css";

function validRange(from: string, until: string): boolean {
  try {
    readPeriodForDay(from);
    readPeriodForDay(until);
    return from <= until;
  } catch {
    return false;
  }
}
export function SourceActivityPage({
  client,
  workspaceId,
  navigate,
}: {
  client: ApiClient;
  workspaceId: string;
  navigate: (path: string) => void;
}) {
  const initial =
    new URLSearchParams(location.search).get("day") ?? localDay(new Date());
  const [from, setFrom] = useState(initial),
    [until, setUntil] = useState(initial);
  const [range, setRange] = useState({ from: initial, until: initial });
  const [data, setData] = useState<SourceActivityView | null>(null);
  const [pending, setPending] = useState(true),
    [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [hovered, setHovered] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setPending(true);
    setError("");
    if (!validRange(range.from, range.until)) {
      setError("Choose a valid date range");
      setPending(false);
      return;
    }
    client
      .sourceActivity(
        workspaceId,
        Intl.DateTimeFormat().resolvedOptions().timeZone,
        range.from,
        range.until,
      )
      .then((value) => {
        if (active) setData(value);
      })
      .catch((e) => {
        if (active)
          setError(
            e instanceof Error ? e.message : "Unable to load source activity",
          );
      })
      .finally(() => {
        if (active) setPending(false);
      });
    return () => {
      active = false;
    };
  }, [client, workspaceId, range, retry]);
  const sources = useMemo(() => {
    const values = new Map<
      string,
      {
        key: string;
        id: string | null;
        name: string;
        present: boolean;
        count: number;
      }
    >();
    for (const entry of data?.days ?? [])
      for (const source of entry.sources) {
        const key = source.subscriptionId ?? "unattributed";
        let row = values.get(key);
        if (!row) {
          row = {
            key,
            id: source.subscriptionId ?? null,
            name: source.name,
            present: source.present,
            count: 0,
          };
          values.set(key, row);
        }
        row.count += source.count;
      }
    return [...values.values()].sort(
      (a, b) => b.count - a.count || a.name.localeCompare(b.name),
    );
  }, [data]);
  const attributions = sources.reduce((sum, row) => sum + row.count, 0);
  const unique = (data?.days ?? []).reduce(
    (sum, value) => sum + value.total,
    0,
  );
  const rectangles = useMemo(
    () => sourceTreemap(sources, 1000, 500),
    [sources],
  );
  const byKey = new Map(sources.map((source) => [source.key, source]));
  const focused = hovered ? byKey.get(hovered) : undefined;
  const label = (source: (typeof sources)[number]) =>
    `${source.name}: ${source.count.toLocaleString()} articles · ${attributions ? ((100 * source.count) / attributions).toFixed(1) : "0"}%`;
  const apply = () => {
    if (pending || !validRange(from, until)) return;
    setPending(true);
    setHovered(null);
    setRange({ from, until });
  };
  return (
    <section
      class="source-activity"
      aria-label="Source contribution analysis"
      aria-busy={pending}
    >
      <header>
        <div>
          <p class="eyebrow">Subscription analytics</p>
          <h1>Where your articles come from</h1>
          <p>Rectangle area shows each source’s contribution.</p>
        </div>
        <button class="secondary-button" onClick={() => navigate("/")}>
          Back to Home
        </button>
      </header>
      <form
        class="source-activity-controls"
        onSubmit={(event) => {
          event.preventDefault();
          apply();
        }}
      >
        <label>
          From{" "}
          <Field
            type="date"
            aria-label="Analysis start date"
            value={from}
            onInput={(event) => setFrom(event.currentTarget.value)}
          />
        </label>
        <label>
          Through{" "}
          <Field
            type="date"
            aria-label="Analysis end date"
            value={until}
            onInput={(event) => setUntil(event.currentTarget.value)}
          />
        </label>
        <button
          class="primary-button"
          type="submit"
          disabled={pending || !validRange(from, until)}
          aria-busy={pending}
        >
          {pending ? "Loading…" : "Apply"}
        </button>
        <button
          class="secondary-button"
          type="button"
          disabled={pending}
          onClick={() => {
            const today = localDay(new Date());
            setFrom(today);
            setUntil(today);
            setRange({ from: today, until: today });
            setPending(true);
          }}
        >
          Today
        </button>
      </form>
      <div class="source-activity-status" role="status">
        {pending ? (
          "Loading activity…"
        ) : error ? (
          <>
            {error}{" "}
            <button
              disabled={pending}
              onClick={() => {
                setPending(true);
                setRetry((value) => value + 1);
              }}
            >
              Retry
            </button>
          </>
        ) : !validRange(from, until) ? (
          "Choose a valid date range: start must precede the end."
        ) : (
          `${range.from} — ${range.until} · ${unique.toLocaleString()} unique articles · ${attributions.toLocaleString()} source contributions`
        )}
      </div>
      <p class="source-activity-note">
        Dates mean arrival in your workspace. An article linked to multiple
        sources contributes once to each; percentages use source contributions.
        Removed sources remain visible in history.
      </p>
      <div
        class="source-treemap-frame"
        aria-label="Source contribution treemap"
      >
        <div
          class="source-treemap"
          inert={pending || !!error}
          style={{ opacity: pending ? 0.45 : 1 }}
        >
          {rectangles.map((rectangle) => {
            const source = byKey.get(rectangle.key)!;
            const style = {
              left: `${rectangle.x / 10}%`,
              top: `${rectangle.y / 5}%`,
              width: `${rectangle.width / 10}%`,
              height: `${rectangle.height / 5}%`,
            };
            return (
              <button
                key={rectangle.key}
                class="source-treemap-tile"
                style={style}
                aria-label={label(source)}
                aria-pressed={hovered === source.key}
                onMouseEnter={() => setHovered(source.key)}
                onMouseLeave={() => setHovered(null)}
                onFocus={() => setHovered(source.key)}
                onBlur={() => setHovered(null)}
                onClick={() =>
                  document.getElementById(`source-rank-${source.key}`)?.focus()
                }
              >
                {rectangle.width > 100 && rectangle.height > 48 && (
                  <span>
                    <strong>{source.name}</strong>
                    <small>
                      {source.count.toLocaleString()} ·{" "}
                      {((100 * source.count) / attributions).toFixed(1)}%
                    </small>
                  </span>
                )}
              </button>
            );
          })}
          {!pending && !error && !sources.length && (
            <p>No articles arrived in this period.</p>
          )}
        </div>
        <div class="source-treemap-detail" role="status">
          {focused
            ? label(focused)
            : "Hover or focus a rectangle for details · click to find it in the ranking"}
        </div>
      </div>
      <div class="source-activity-ranking" inert={pending || !!error}>
        <table>
          <caption>All sources, ranked by contribution</caption>
          <thead>
            <tr>
              <th scope="col">Source</th>
              <th scope="col">Articles</th>
              <th scope="col">Share</th>
            </tr>
          </thead>
          <tbody>
            {sources.map((source) => (
              <tr
                key={source.key}
                id={`source-rank-${source.key}`}
                tabIndex={-1}
              >
                <th scope="row">
                  {source.id && source.present ? (
                    <a
                      href={`/subscriptions/${source.id}`}
                      onClick={(event) => {
                        if (
                          event.button ||
                          event.metaKey ||
                          event.ctrlKey ||
                          event.shiftKey ||
                          event.altKey
                        )
                          return;
                        event.preventDefault();
                        navigate(`/subscriptions/${source.id}`);
                      }}
                    >
                      {source.name}
                    </a>
                  ) : (
                    <span>
                      {source.name}
                      {source.id ? " (removed)" : ""}
                    </span>
                  )}
                </th>
                <td>{source.count.toLocaleString()}</td>
                <td>
                  {attributions
                    ? ((100 * source.count) / attributions).toFixed(1)
                    : "0"}
                  %
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
