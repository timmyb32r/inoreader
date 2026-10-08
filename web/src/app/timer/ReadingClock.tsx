import { useReturnToNews } from "./useReturnToNews";
import { Icon } from "../../ui/Icon";
import type { SessionClock } from "../reading/useSessionClock";
const time = (ms: number) => {
  const seconds = Math.ceil(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};
export function ReadingClock({
  clock,
  onReturn,
}: {
  clock: SessionClock;
  onReturn: () => string | null;
}) {
  const returning = useReturnToNews(onReturn);
  const active = clock.active!;
  const p = active.progress;
  const smart = p.mode === "smart" ? p.remainingMs : 0;
  const random = p.finished
    ? 0
    : p.mode === "random"
      ? p.remainingMs
      : active.plan.random * 60000;
  const spent =
    active.plan.smart * 60000 + active.plan.random * 60000 - smart - random;
  const total = active.plan.smart * 60000 + active.plan.random * 60000;
  return (
    <div
      class={`reading-clock ${p.paused ? "paused" : ""}`}
      aria-label="Этап чтения"
    >
      <div class="reading-clock__phase">
        <strong>
          {p.finished
            ? "Сессия завершена"
            : p.mode === "smart"
              ? "Умная лента"
              : "Случайная лента"}
        </strong>
        <span class="reading-clock__digits">{time(p.remainingMs)}</span>
        <span class="reading-clock__status">
          {clock.error
            ? "Ошибка сохранения"
            : p.finished
              ? "завершено"
              : p.paused
                ? "пауза"
                : p.remainingMs === 0
                  ? "смена после статьи"
                  : "осталось"}
        </span>
      </div>
      <div class="reading-clock__other">
        <span>
          {p.mode === "smart" ? "Далее · случайная" : "Умная · завершена"}
        </span>
        <strong>{time(p.mode === "smart" ? random : smart)}</strong>
      </div>
      <button
        class="reading-clock__control"
        disabled={p.finished || !!clock.error}
        aria-pressed={p.paused}
        aria-label={p.paused ? "Продолжить таймер" : "Пауза таймера"}
        title={p.paused ? "Продолжить таймер" : "Пауза таймера"}
        onClick={clock.toggle}
      >
        {p.paused ? <span aria-hidden="true">▶</span> : <Icon name="pause" />}
      </button>
      <button
        class="reading-clock__control"
        disabled={!!clock.error}
        aria-label="Остановить сессию"
        title="Остановить сессию"
        onClick={clock.stop}
      >
        <Icon name="stop" />
      </button>
      <button
        class="reading-clock__return"
        disabled={
          returning.pending ||
          !new URL(active.url, location.origin).searchParams.has("article")
        }
        aria-busy={returning.pending}
        onClick={returning.open}
      >
        {returning.pending ? (
          <span class="spinner" />
        ) : (
          <Icon name="external" />
        )}
        <span>К текущей новости</span>
      </button>
      <div class="reading-clock__track" aria-hidden="true">
        <i style={{ width: `${(100 * spent) / total}%` }} />
      </div>
      <span class="sr-only" role="status">
        {clock.error}
      </span>
    </div>
  );
}
