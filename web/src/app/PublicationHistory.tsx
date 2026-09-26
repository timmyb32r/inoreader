import { useEffect, useMemo, useState } from "preact/hooks";
import type { ApiClient, PublicationHistory as History } from "../api/client";
import { AutofillResistantSelect } from "../ui/fields";
import "./publication-history.css";

type Scale = "days" | "months" | "years";
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const pad = (value: number) => String(value).padStart(2, "0");
const dateKey = (year: number, month: number, day: number) => `${year}-${pad(month + 1)}-${pad(day)}`;
const monthLength = (year: number, month: number) => new Date(Date.UTC(year, month + 1, 0)).getUTCDate();

export function PublicationHistory({ client, subscriptionId }: { client: ApiClient; subscriptionId: string }) {
  const [history, setHistory] = useState<History | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [scale, setScale] = useState<Scale>("months");
  const [year, setYear] = useState(new Date().getUTCFullYear());
  const [month, setMonth] = useState(new Date().getUTCMonth());
  const [inspected, setInspected] = useState("");
  useEffect(() => {
    let active = true;
    setHistory(null); setError("");
    client.publicationHistory(subscriptionId).then(data => {
      if (!active) return;
      setHistory(data);
      const last = data.days.at(-1)?.date;
      if (last) { setYear(Number(last.slice(0, 4))); setMonth(Number(last.slice(5, 7)) - 1); }
    }).catch((reason: Error) => { if (active) setError(reason.message); });
    return () => { active = false; };
  }, [client, subscriptionId, attempt]);
  const counts = useMemo(() => new Map(history?.days.map(day => [day.date, day.count])), [history]);
  const years = useMemo(() => [...new Set([year, ...(history?.days.map(day => Number(day.date.slice(0, 4))) ?? [])])].sort((a, b) => a - b), [history, year]);
  const monthCounts = useMemo(() => months.map((_, index) => (history?.days ?? []).filter(day => day.date.startsWith(`${year}-${pad(index + 1)}-`)).reduce((sum, day) => sum + day.count, 0)), [history, year]);
  const bars = useMemo(() => {
    if (scale === "days") return Array.from({ length: monthLength(year, month) }, (_, index) => ({ label: String(index + 1), title: dateKey(year, month, index + 1), count: counts.get(dateKey(year, month, index + 1)) ?? 0, value: index + 1 }));
    if (scale === "months") return months.map((label, index) => ({ label, title: `${label} ${year}`, count: monthCounts[index], value: index }));
    const first = years[0], last = years.at(-1)!;
    const totals = new Map<number, number>();
    history?.days.forEach(day => { const y = Number(day.date.slice(0, 4)); totals.set(y, (totals.get(y) ?? 0) + day.count); });
    return Array.from({ length: last - first + 1 }, (_, index) => ({ label: String(first + index), title: String(first + index), count: totals.get(first + index) ?? 0, value: first + index }));
  }, [scale, history, year, month, counts, years, monthCounts]);
  const max = Math.max(1, ...bars.map(bar => bar.count));
  const maxDay = Math.max(1, ...(history?.days.filter(day => day.date.startsWith(`${year}-`)).map(day => day.count) ?? []));
  const total = history?.days.reduce((sum, day) => sum + day.count, 0) ?? 0;
  const caption = (title: string, count: number) => `${title} · ${count} ${count === 1 ? "article" : "articles"}`;
  const pending = !history && !error;
  return <section class="detail-card publication-history" aria-label="Publication history" aria-busy={pending}>
    <header class="publication-heading"><div><p class="eyebrow">Publishing rhythm</p><h2>Publication history</h2></div><div class="publication-scale" role="group" aria-label="Chart interval">{(["days", "months", "years"] as const).map(value => <button disabled={!history} aria-pressed={scale === value} onClick={() => { setScale(value); setInspected(""); }}>{value[0].toUpperCase() + value.slice(1)}</button>)}</div></header>
    <p class="publication-summary">{history ? `${total.toLocaleString("en-US")} dated articles${history.days.length ? ` · ${history.days[0].date} — ${history.days.at(-1)!.date}` : ""}` : "Publication dates from collected articles"}</p>
    <div class="publication-controls"><label>Year <AutofillResistantSelect aria-label="Publication year" disabled={!history} value={String(year)} onChange={event => { setYear(Number(event.currentTarget.value)); setInspected(""); }}>{years.map(value => <option value={value}>{value}</option>)}</AutofillResistantSelect></label><label class={scale === "days" ? "" : "publication-month-hidden"}>Month <AutofillResistantSelect aria-label="Publication month" disabled={!history || scale !== "days"} value={String(month)} onChange={event => { setMonth(Number(event.currentTarget.value)); setInspected(""); }}>{months.map((value, index) => <option value={index}>{value}</option>)}</AutofillResistantSelect></label><span>Articles / {scale === "days" ? "day" : scale === "months" ? "month" : "year"}</span></div>
    <div class="publication-content">
      <div class="publication-chart" role="group" aria-label={`Articles by ${scale}`}>
        {bars.map(bar => <button disabled={!history} class="publication-bar" title={caption(bar.title, bar.count)} aria-label={caption(bar.title, bar.count)} aria-pressed={scale === "years" ? bar.value === year : scale === "months" ? bar.value === month : inspected === caption(bar.title, bar.count)} onMouseEnter={() => setInspected(caption(bar.title, bar.count))} onFocus={() => setInspected(caption(bar.title, bar.count))} onClick={() => { setInspected(caption(bar.title, bar.count)); if (scale === "years") setYear(bar.value); if (scale === "months") setMonth(bar.value); }}><span class="publication-bar-track"><span style={{ height: `${bar.count / max * 100}%` }}/></span><span class="publication-bar-label">{bar.label}</span></button>)}
      </div>
      <div class="publication-inspected" aria-live="polite">{inspected || (history && !total ? "No dated articles collected yet." : "Hover or focus a bar or date to see the article count.")}</div>
      <div class="publication-calendar-heading"><h3>{year} at a glance</h3><span>Fewer <i class="publication-dot publication-dot--small"/><i class="publication-dot"/> More</span></div>
      <div class="publication-calendar">{months.map((name, index) => {
        const offset = (new Date(Date.UTC(year, index, 1)).getUTCDay() + 6) % 7;
        return <section class="publication-month" aria-label={`${name} ${year}`}><header><button disabled={!history} onClick={() => { setMonth(index); setScale("days"); setInspected(""); }}>{name}</button><span>{monthCounts[index]}</span></header><div class="publication-weekdays" aria-hidden="true">{["M", "T", "W", "T", "F", "S", "S"].map(day => <span>{day}</span>)}</div><div class="publication-dates">{Array.from({ length: offset }, () => <span/>)}{Array.from({ length: monthLength(year, index) }, (_, day) => {
          const key = dateKey(year, index, day + 1), count = counts.get(key) ?? 0, label = caption(key, count);
          return <button disabled={!history} aria-label={label} title={label} class={count ? "has-publications" : ""} onMouseEnter={() => setInspected(label)} onFocus={() => setInspected(label)} onClick={() => { setMonth(index); setScale("days"); setInspected(label); }}><i style={{ transform: `scale(${count ? .38 + .62 * Math.sqrt(count / maxDay) : 0})` }}/><span>{day + 1}</span></button>;
        })}</div></section>;
      })}</div>
      {!history && <div class="publication-loading" role={error ? "alert" : "status"}>{error ? <><strong>Publication history could not be loaded.</strong><span>{error}</span><button class="secondary-button" onClick={() => { setError(""); setAttempt(value => value + 1); }}>Retry</button></> : <><span class="spinner"/>Loading publication dates…</>}</div>}
    </div>
    <footer class="publication-footnote">Collected articles only · Publication dates in UTC · Circle size shows articles per day.<br/>{history ? `${history.undated} without a publication date · ${history.conflicting} with conflicting dates (excluded from chart).` : "Articles without a publication date are reported separately."}</footer>
  </section>;
}
