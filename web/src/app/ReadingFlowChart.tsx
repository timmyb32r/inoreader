import { useState } from "preact/hooks";
import "./reading-flow.css";

export type ReadingFlowDay = {
  key: string;
  label: string;
  count: number;
  arrived: number;
};

// Explicit owner-requested chart-only exclusions. Keep calendar slots and source data.
const hiddenDays = new Set(["2026-09-26", "2026-09-27", "2026-09-28"]);

/** Two independent counts share a linear scale; bars are adjacent, never stacked. */
export function ReadingFlowChart({
  days,
  status,
  onReadDay,
  opening = false,
}: {
  days: ReadingFlowDay[];
  status: "loading" | "ready" | "error";
  onReadDay?: (day: string) => void;
  opening?: boolean;
}) {
  const loaded = status === "ready";
  const [active, setActive] = useState<number | null>(null);
  const recent = days.slice(-30);
  const step = Math.max(
    1,
    Math.ceil(
      Math.max(
        ...recent
          .filter((day) => !hiddenDays.has(day.key))
          .flatMap((day) => [day.count, day.arrived]),
        0,
      ) / 4,
    ),
  );
  const ceiling = step * 4;
  const selected = active === null ? undefined : recent[active];
  return (
    <section
      class="activity-card reading-flow"
      aria-label="Articles received and read over the last 30 days"
      aria-busy={status === "loading"}
    >
      <div class="activity-card__heading">
        <div>
          <h2>Arrived &amp; read</h2>
          <p role="status">
            {status === "error"
              ? "Could not load activity. Return to this page to retry."
              : "Daily articles · last 30 days"}
          </p>
        </div>
        <div class="reading-flow__legend">
          <span>
            <i class="reading-flow__arrived" />
            Arrived
          </span>
          <span>
            <i class="reading-flow__read" />
            Read
          </span>
        </div>
      </div>
      <div class="reading-flow__scroll">
        <div class="reading-flow__canvas">
          <div class="reading-flow__plot">
            {[0, 1, 2, 3, 4].map((tick) => (
              <div
                class="reading-flow__tick"
                style={{ bottom: `${tick * 25}%` }}
                aria-hidden="true"
              >
                <span>{tick * step}</span>
              </div>
            ))}
            <div
              class="reading-flow__bars"
              onMouseLeave={() => setActive(null)}
            >
              {recent.map((day, index) =>
                hiddenDays.has(day.key) ? (
                  <div
                    key={day.key}
                    class="reading-flow__day"
                    aria-hidden="true"
                    onMouseEnter={() => setActive(null)}
                  />
                ) : (
                  <div
                    key={day.key}
                    class="reading-flow__day"
                    tabIndex={loaded ? 0 : -1}
                    role="group"
                    aria-label={`${day.label}: ${loaded ? `${day.arrived} arrived, ${day.count} read` : "not loaded"}`}
                    onMouseEnter={() => setActive(index)}
                    onFocus={() => setActive(index)}
                    onBlur={() => setActive(null)}
                  >
                    <i
                      class="reading-flow__arrived"
                      style={{ height: `${(day.arrived / ceiling) * 100}%` }}
                      aria-hidden="true"
                    />
                    <button
                      class="reading-flow__read-target"
                      disabled={!loaded || opening || !onReadDay}
                      aria-busy={opening}
                      aria-label={`Show articles marked read on ${day.label}`}
                      onClick={() => onReadDay?.(day.key)}
                    >
                      <i
                        class="reading-flow__read"
                        style={{ height: `${(day.count / ceiling) * 100}%` }}
                        aria-hidden="true"
                      />
                    </button>
                  </div>
                ),
              )}
            </div>
            {(recent.length
              ? [
                  ...new Set([
                    0,
                    Math.floor((recent.length - 1) / 2),
                    recent.length - 1,
                  ]),
                ]
              : []
            ).map((index, position) => (
              <span
                class={`reading-flow__date reading-flow__date--${position}`}
                aria-hidden="true"
              >
                {recent[index].label}
              </span>
            ))}
            {selected && loaded && (
              <div
                class="reading-flow__tooltip"
                role="tooltip"
                style={{
                  left: `${Math.max(18, Math.min(82, (((active ?? 0) + 0.5) / recent.length) * 100))}%`,
                }}
              >
                <strong>{selected.label}</strong>
                <span>
                  Arrived: {selected.arrived} · Read: {selected.count}
                </span>
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
