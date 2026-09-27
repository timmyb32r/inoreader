import type { Subscription } from "./data";
export type CatalogView = "current" | "attention" | "archived";
export type SortDirection = "asc" | "desc";
export type Column =
  | "attention"
  | "name"
  | "kind"
  | "status"
  | "updated"
  | "unread"
  | "url"
  | "note"
  | "interval"
  | "error"
  | "added";
export const defaults: Column[] = [
  "name",
  "attention",
  "kind",
  "status",
  "updated",
  "unread",
];
export const labels: Record<Column, string> = {
  attention: "Needs attention",
  name: "Name",
  kind: "Type",
  status: "Status",
  updated: "Last updated",
  unread: "Unread",
  url: "URL",
  note: "Personal note",
  interval: "Update frequency",
  error: "Current error",
  added: "Added",
};
const memoryLayouts = new Map<string, string>();
export type Layout = {
  columns: Column[];
  widths: Partial<Record<Column, number>>;
};
export const defaultWidths: Record<Column, number> = {
  attention: 160,
  name: 220,
  kind: 110,
  status: 110,
  updated: 160,
  unread: 85,
  url: 280,
  note: 240,
  interval: 150,
  error: 240,
  added: 160,
};
export function readLayout(key: string): Layout {
  try {
    const raw =
      globalThis.localStorage?.getItem(key) ?? memoryLayouts.get(key) ?? "";
    const v = JSON.parse(raw);
    if (Array.isArray(v) && v.includes("name"))
      return { columns: withAttention(v), widths: {} };
    if (Array.isArray(v?.columns) && v.columns.includes("name"))
      return { ...v, columns: withAttention(v.columns) };
  } catch {}
  return { columns: defaults, widths: {} };
}
export function writeLayout(key: string, value: Layout) {
  const raw = JSON.stringify(value);
  memoryLayouts.set(key, raw);
  try {
    globalThis.localStorage?.setItem(key, raw);
  } catch {}
}
export function readSort(key: string): {
  column: Column;
  direction: SortDirection;
} {
  try {
    const raw =
      globalThis.localStorage?.getItem(key) ?? memoryLayouts.get(key) ?? "";
    if (raw === "name" || raw === "updated" || raw === "unread")
      return { column: raw, direction: raw === "name" ? "asc" : "desc" };
    const value = JSON.parse(raw);
    if (
      (Object.keys(labels) as string[]).includes(value?.column) &&
      (value.direction === "asc" || value.direction === "desc")
    )
      return value;
  } catch {}
  return { column: "name", direction: "asc" };
}
export function writeSort(
  key: string,
  column: Column,
  direction: SortDirection,
) {
  const value = JSON.stringify({ column, direction });
  memoryLayouts.set(key, value);
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {}
}
export function move<T>(values: T[], from: number, to: number) {
  const copy = [...values];
  const [item] = copy.splice(from, 1);
  copy.splice(to, 0, item);
  return copy;
}
export function toggle(set: Set<string>, id: string) {
  const n = new Set(set);
  n.has(id) ? n.delete(id) : n.add(id);
  return n;
}
export function formatKind(v?: Subscription["sourceType"]) {
  return v
    ? v.replaceAll("_", " ").replace(/^./, (x) => x.toUpperCase())
    : "Unknown";
}
export function isProblem(subscription: Subscription) {
  return !!(
    subscription.needsAttention ||
    subscription.attentionReason ||
    subscription.incomplete
  );
}
function sortableValue(
  subscription: Subscription,
  column: Column,
): string | number {
  switch (column) {
    case "attention":
      return isProblem(subscription) ? 0 : 1;
    case "name":
      return subscription.name;
    case "kind":
      return formatKind(subscription.sourceType);
    case "status":
      return subscription.status;
    case "updated":
      return subscription.lastUpdate ?? "";
    case "unread":
      return subscription.unreadCount ?? 0;
    case "url":
      return subscription.sourceUrl ?? "";
    case "note":
      return subscription.personalNote ?? "";
    case "interval":
      return subscription.pollingInterval ?? "";
    case "error":
      return subscription.error ?? "";
    case "added":
      return subscription.createdAt ?? "";
  }
}
export function compareSubscriptions(
  a: Subscription,
  b: Subscription,
  column: Column,
  direction: SortDirection,
) {
  const left = sortableValue(a, column),
    right = sortableValue(b, column);
  const order =
    typeof left === "number" && typeof right === "number"
      ? left - right
      : String(left).localeCompare(String(right), undefined, {
          numeric: true,
          sensitivity: "base",
        });
  return (direction === "asc" ? order : -order) || a.name.localeCompare(b.name);
}
function withAttention(columns: Column[]): Column[] {
  return columns.includes("attention")
    ? columns
    : [...columns.slice(0, 1), "attention", ...columns.slice(1)];
}
