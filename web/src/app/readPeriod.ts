import type { ReadPeriod } from "../api/client";

/** Calendar-day boundaries use the same browser timezone as the Home chart.
 * Advancing the date (not adding 24 hours) preserves DST-short/long days. */
export function readPeriodForDay(day: string): ReadPeriod {
  const from = new Date(`${day}T00:00:00`);
  if (
    !/^\d{4}-\d{2}-\d{2}$/.test(day) ||
    !Number.isFinite(from.getTime()) ||
    localDay(from) !== day
  )
    throw new Error("Choose a valid calendar day");
  const until = new Date(from);
  until.setDate(until.getDate() + 1);
  return { from: from.toISOString(), until: until.toISOString() };
}
export function localDay(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
