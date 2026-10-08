import {
  advanceSession,
  initialSession,
  readingPlan,
  readSession,
  sessionUrl,
} from "./readingSession";
it("validates exact minute plans and explicitly skips zero phases", () => {
  const plan = readingPlan("45", "15");
  expect(initialSession(plan)).toEqual({
    mode: "smart",
    remainingMs: 2700000,
    paused: false,
    finished: false,
  });
  expect(initialSession(readingPlan("0", "15")).mode).toBe("random");
  for (const pair of [
    ["0", "0"],
    ["-1", "15"],
    ["1.5", "15"],
    ["NaN", "1"],
    ["", "1"],
    [String(Number.MAX_SAFE_INTEGER), "1"],
  ])
    expect(() => readingPlan(...(pair as [string, string]))).toThrow();
});
it("changes modes only at an explicit article boundary and completes the second phase", () => {
  const plan = readingPlan("45", "15"),
    first = initialSession(plan);
  expect(advanceSession(first, plan)).toBe(first);
  const second = advanceSession({ ...first, remainingMs: 0 }, plan);
  expect(second).toEqual({ ...first, mode: "random", remainingMs: 900000 });
  expect(advanceSession({ ...second, remainingMs: 0 }, plan).finished).toBe(
    true,
  );
});
it("restores an owned session progress without accepting corruption or a different plan", () => {
  const plan = readingPlan("45", "15"),
    progress = { ...initialSession(plan), remainingMs: 1000, paused: true };
  expect(readSession(JSON.stringify({ plan, progress }), plan)).toEqual(
    progress,
  );
  for (const raw of [
    "{}",
    JSON.stringify({ plan, progress: { ...progress, remainingMs: -1 } }),
    JSON.stringify({
      plan,
      progress: { ...progress, remainingMs: 0, finished: true },
    }),
    JSON.stringify({ plan, progress: { ...progress, remainingMs: Infinity } }),
    JSON.stringify({ plan: { smart: 1, random: 1 }, progress }),
  ])
    expect(() => readSession(raw, plan)).toThrow();
  const url = new URL(
    sessionUrl("private-workspace", plan),
    "https://reader.test",
  );
  expect(url.searchParams.get("smart")).toBe("45");
  expect(url.searchParams.get("random")).toBe("15");
  expect(url.searchParams.get("session")).toBeTruthy();
});
