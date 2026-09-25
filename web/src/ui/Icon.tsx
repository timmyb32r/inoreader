import type { JSX } from "preact";

export type IconName = "inbox" | "unread" | "star" | "later" | "trash" | "feed" | "search" | "settings" | "sun" | "moon" | "plus" | "refresh" | "dots" | "check" | "arrow" | "close" | "external" | "rule" | "globe" | "pause" | "archive" | "menu";

const paths: Record<IconName, JSX.Element> = {
  inbox: <><path d="M4 5h16v14H4z"/><path d="M4 14h4l2 3h4l2-3h4"/></>,
  unread: <><circle cx="12" cy="12" r="8"/><path d="m9.5 12 1.7 1.7 3.8-4"/></>,
  star: <path d="m12 3 2.7 5.5 6.1.9-4.4 4.3 1 6-5.4-2.8-5.4 2.8 1-6-4.4-4.3 6.1-.9z"/>,
  later: <><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>,
  trash: <><path d="M5 7h14M9 7V4h6v3M7 7l1 13h8l1-13"/><path d="M10 11v5M14 11v5"/></>,
  feed: <><path d="M5 18a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z"/><path d="M4 11a7 7 0 0 1 7 7M4 5a13 13 0 0 1 13 13"/></>,
  search: <><circle cx="11" cy="11" r="6"/><path d="m16 16 4 4"/></>,
  settings: <><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.4-2.4 1A8 8 0 0 0 15 6l-.3-2.6h-4L10.5 6A8 8 0 0 0 9 7.1l-2.4-1-2 3.4 2 1.5a7 7 0 0 0 0 2l-2 1.5 2 3.4 2.4-1A8 8 0 0 0 10.5 18l.3 2.6h4L15 18a8 8 0 0 0 1.5-1.1l2.4 1 2-3.4-2-1.5a7 7 0 0 0 .1-1Z"/></>,
  sun: <><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></>,
  moon: <path d="M20 15.2A8.5 8.5 0 0 1 8.8 4 8.5 8.5 0 1 0 20 15.2Z"/>,
  plus: <path d="M12 5v14M5 12h14"/>, refresh: <><path d="M20 7v5h-5"/><path d="M18.5 16a8 8 0 1 1 .5-8l1 4"/></>, dots: <><circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/></>,
  check: <path d="m5 12 4 4L19 6"/>, arrow: <path d="m9 18 6-6-6-6"/>, close: <><path d="m6 6 12 12M18 6 6 18"/></>, external: <><path d="M14 5h5v5M19 5l-8 8"/><path d="M17 13v6H5V7h6"/></>,
  rule: <><path d="M4 6h10M4 12h16M4 18h8"/><circle cx="17" cy="6" r="2"/><circle cx="15" cy="18" r="2"/></>, globe: <><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></>, pause: <><path d="M9 7v10M15 7v10"/></>, archive: <><path d="M4 7h16v13H4zM3 4h18v3M9 11h6"/></>, menu: <path d="M4 7h16M4 12h16M4 17h16"/>,
};

export function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{paths[name]}</svg>;
}
