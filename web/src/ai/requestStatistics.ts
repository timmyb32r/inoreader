import type { AiRequestStatistics } from "../api/generated";
export type Bucket = AiRequestStatistics["bucket"];
export const modes = [
  "summary",
  "terms",
  "ranking",
  "verification",
  "chat",
  "translation",
] as const;
export const labels: Record<(typeof modes)[number], string> = {
  summary: "Пересказы",
  terms: "Термины / сущности",
  ranking: "Оценка интереса",
  verification: "Проверка фактов",
  chat: "Обсуждение",
  translation: "Перевод",
};
export function sumUsd(values: string[]): string {
  const scale = Math.max(
    0,
    ...values.map((value) => value.split(".")[1]?.length ?? 0),
  );
  const units = values
    .reduce((total, value) => {
      const [whole, fraction = ""] = value.split(".");
      return total + BigInt(whole + fraction.padEnd(scale, "0"));
    }, 0n)
    .toString()
    .padStart(scale + 1, "0");
  return scale ? `${units.slice(0, -scale)}.${units.slice(-scale)}` : units;
}
export function isoDate(date: Date): string {
  return date.toISOString().slice(0, 10);
}
export function moscowToday(): string {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone: "Europe/Moscow",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  return ["year", "month", "day"]
    .map((type) => parts.find((part) => part.type === type)!.value)
    .join("-");
}
export function validDates(from: string, until: string): boolean {
  const valid = (value: string) =>
    /^\d{4}-\d{2}-\d{2}$/.test(value) &&
    !Number.isNaN(Date.parse(value)) &&
    isoDate(new Date(value)) === value;
  return valid(from) && valid(until) && from <= until;
}
export function periodDates(
  from: string,
  until: string,
  bucket: Bucket,
): string[] {
  const cursor = new Date(`${from}T00:00:00Z`);
  if (bucket !== "day") cursor.setUTCDate(1);
  if (bucket === "year") cursor.setUTCMonth(0);
  const dates: string[] = [];
  while (isoDate(cursor) <= until) {
    dates.push(isoDate(cursor));
    if (bucket === "day") cursor.setUTCDate(cursor.getUTCDate() + 1);
    else if (bucket === "month") cursor.setUTCMonth(cursor.getUTCMonth() + 1);
    else cursor.setUTCFullYear(cursor.getUTCFullYear() + 1);
  }
  return dates;
}
