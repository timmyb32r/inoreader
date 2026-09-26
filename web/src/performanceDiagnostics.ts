type ResourceRow = {
  resource: string;
  type: string;
  durationMs: number;
  waitMs: number;
  downloadMs: number;
  transferKiB: number | "cache/unknown";
};

let enabled = false;
let readyReported = false;

export function startPerformanceDiagnostics() {
  enabled = true;
  const report = () => reportNavigation("Document loaded");
  if (document.readyState === "complete") window.setTimeout(report, 0);
  else window.addEventListener("load", report, { once: true });
}

export function reportLibraryReady() {
  if (!enabled || readyReported) return;
  readyReported = true;
  window.requestAnimationFrame(() => window.setTimeout(() => {
    const elapsed = round(performance.now());
    console.groupCollapsed(`[perf] Reader rendered in ${elapsed} ms`);
    console.table(navigationRows());
    const resources = resourceRows(performance.getEntriesByType("resource") as PerformanceResourceTiming[]);
    console.table(resources);
    console.info(`[perf] ${resources.length} resources; sorted slowest first`);
    console.groupEnd();
  }, 0));
}

export function reportApiRequest(path: string, method: string, startedAt: number, status: number | "network-error") {
  if (!enabled) return;
  const duration = round(performance.now() - startedAt);
  const absolute = new URL(path, location.href).href;
  const timing = (performance.getEntriesByName(absolute, "resource").at(-1) as PerformanceResourceTiming | undefined);
  const transfer = timing?.transferSize ? `${round(timing.transferSize / 1024)} KiB` : "cache/unknown";
  const message = `[perf] API ${method} ${path} — ${duration} ms, status ${status}, transfer ${transfer}`;
  if (duration >= 1000 || status === "network-error" || status >= 400) console.warn(message);
  else console.info(message);
}

export function resourceRows(entries: PerformanceResourceTiming[]): ResourceRow[] {
  return entries.map((entry) => ({
    resource: resourceName(entry.name),
    type: entry.initiatorType || "other",
    durationMs: round(entry.duration),
    waitMs: round(Math.max(0, entry.responseStart - entry.requestStart)),
    downloadMs: round(Math.max(0, entry.responseEnd - entry.responseStart)),
    transferKiB: entry.transferSize ? round(entry.transferSize / 1024) : "cache/unknown" as const,
  })).sort((left, right) => right.durationMs - left.durationMs);
}

function reportNavigation(label: string) {
  if (!enabled) return;
  const navigation = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
  const elapsed = navigation ? round(navigation.loadEventEnd || performance.now()) : round(performance.now());
  console.info(`[perf] ${label} in ${elapsed} ms`);
}

function navigationRows() {
  const navigation = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
  if (!navigation) return [];
  return [{
    phase: "navigation",
    dnsMs: round(navigation.domainLookupEnd - navigation.domainLookupStart),
    connectMs: round(navigation.connectEnd - navigation.connectStart),
    serverWaitMs: round(navigation.responseStart - navigation.requestStart),
    downloadMs: round(navigation.responseEnd - navigation.responseStart),
    domInteractiveMs: round(navigation.domInteractive),
    loadMs: round(navigation.loadEventEnd || performance.now()),
  }];
}

function resourceName(value: string) {
  try {
    const url = new URL(value, location.href);
    return `${url.pathname}${url.search}`;
  } catch {
    return value;
  }
}

const round = (value: number) => Math.round(value * 10) / 10;
