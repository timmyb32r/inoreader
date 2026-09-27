export function formatArticleDate(value: string): string {
  // A source may provide only a day or a local clock. Do not invent midnight
  // or let the browser interpret that clock in the reader's local timezone.
  const local =
    /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}:\d{2}:\d{2})(\.\d+)?)?$/.exec(value);
  if (local) {
    const month = [
      "jan",
      "feb",
      "mar",
      "apr",
      "may",
      "jun",
      "jul",
      "aug",
      "sep",
      "oct",
      "nov",
      "dec",
    ][Number(local[2]) - 1];
    return `${local[1]}-${month}-${local[3]}${local[4] ? ` ${local[4]}` : ""}`;
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const months = [
    "jan",
    "feb",
    "mar",
    "apr",
    "may",
    "jun",
    "jul",
    "aug",
    "sep",
    "oct",
    "nov",
    "dec",
  ];
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${date.getUTCFullYear()}-${months[date.getUTCMonth()]}-${pad(date.getUTCDate())} ${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}:${pad(date.getUTCSeconds())}`;
}
