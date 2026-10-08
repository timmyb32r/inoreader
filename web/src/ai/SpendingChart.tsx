import type { AiProfile } from "../api/ai";
const modes = [
  "summary",
  "verification",
  "chat",
  "translation",
  "terms",
  "ranking",
] as const;
const labels = [
  "Summary",
  "Fact-check",
  "Chat",
  "Translation",
  "Terms",
  "Ranking",
];
export function SpendingChart({
  spending,
}: {
  spending: AiProfile["spending"] | undefined;
}) {
  const today =
    spending?.today ??
    new Intl.DateTimeFormat("sv-SE", { timeZone: "Europe/Moscow" }).format(
      new Date(),
    );
  const days = Array.from({ length: 30 }, (_, i) => {
    const date = new Date(`${today}T12:00:00Z`);
    date.setUTCDate(date.getUTCDate() - 29 + i);
    const day = date.toISOString().slice(0, 10);
    const values = modes.map((mode) =>
      spending?.days.find((row) => row.day === day && row.mode === mode),
    );
    return {
      day,
      values,
      total: values.reduce((sum, row) => sum + Number(row?.spentUsd ?? 0), 0),
    };
  });
  const max = Math.max(
    Number(spending?.dailyLimitUsd ?? 3),
    ...days.map((day) => day.total),
  );
  return (
    <section
      class="ai-spending"
      aria-label="Daily DeepSeek spending"
      aria-busy={!spending}
    >
      <h4>Estimated spending · Moscow</h4>
      <div class="ai-spending__summary" role="status">
        {spending ? (
          <>
            <span>Today ${spending.spentUsd}</span>
            <span>Reserved ${spending.reservedUsd}</span>
            <span>
              Available ${spending.remainingUsd} of ${spending.dailyLimitUsd}
            </span>
          </>
        ) : (
          "Loading spending…"
        )}
      </div>
      <div
        class="ai-spending__plot"
        aria-label="Spending over the last 30 days"
      >
        <span class="ai-spending__axis">
          ${max.toFixed(2)}
          <br />
          USD
        </span>
        {days.map(({ day, values }) => (
          <div
            class="ai-spending__day"
            key={day}
            tabIndex={0}
            role="img"
            aria-label={
              !spending
                ? `${day}: spending unavailable`
                : `${day}: ${modes.map((mode, i) => `${labels[i]} $${values[i]?.spentUsd ?? "0"}, reserved $${values[i]?.reservedUsd ?? "0"}`).join("; ")}`
            }
          >
            <div class="ai-spending__bar">
              {values.map((value, i) => (
                <span
                  key={modes[i]}
                  class={`ai-spending__segment ai-spending__mode-${i}`}
                  style={{
                    height: `${(Number(value?.spentUsd ?? 0) / max) * 100}%`,
                  }}
                />
              ))}
            </div>
            <span class="ai-spending__tooltip">
              {day}
              {values.map((value, i) => (
                <span key={modes[i]}>
                  {labels[i]}: ${value?.spentUsd ?? "0"}
                  {Number(value?.reservedUsd ?? 0) > 0
                    ? ` (+$${value?.reservedUsd} reserved)`
                    : ""}
                </span>
              ))}
            </span>
          </div>
        ))}
      </div>
      <div class="ai-spending__range">
        {days[0].day} — {today}
      </div>
      <div class="ai-spending__legend">
        {labels.map((label, i) => (
          <span key={label}>
            <i class={`ai-spending__mode-${i}`} />
            {label}
          </span>
        ))}
      </div>
      <small>
        Peak-rate estimates recorded from activation. Uncertain charges remain
        reserved. Limit resets at midnight in Moscow. Spending outside Reader is
        not included.
      </small>
    </section>
  );
}
