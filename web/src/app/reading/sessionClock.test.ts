import { describe, expect, it } from "vitest";
import { initialSession, advanceSession, sessionUrl } from "./readingSession";
import {
  sessionAt,
  startClock,
  sessionFromUrl,
  restoreClock,
} from "./sessionClock";
describe("absolute reading session clock", () => {
  it("charges hidden time and time away exactly once and keeps article phase until next", () => {
    const running = startClock(initialSession({ smart: 1, random: 1 }), 1000);
    expect(sessionAt(running, 31000).remainingMs).toBe(30000);
    const expired = sessionAt(running, 90000);
    expect(expired.mode).toBe("smart");
    expect(expired.remainingMs).toBe(0);
    const random = startClock(
      advanceSession(expired, { smart: 1, random: 1 }),
      90000,
    );
    expect(random.mode).toBe("random");
    expect(random.deadline).toBe(150000);
  });
  it("pauses without charging time away and resumes at the saved remainder", () => {
    const active = startClock(initialSession({ smart: 1, random: 0 }), 1000);
    const paused = {
      ...sessionAt(active, 11000),
      paused: true,
      deadline: undefined,
    };
    expect(sessionAt(paused, 999999).remainingMs).toBe(50000);
    expect(startClock({ ...paused, paused: false }, 20000).deadline).toBe(
      70000,
    );
  });
  it("restores owned article, position and deadline without resetting elapsed time", () => {
    const url = sessionUrl("mine", { smart: 1, random: 1 }) + "&article=news";
    const active = sessionFromUrl(url, ["mine"])!;
    active.progress = startClock(active.progress, 1000);
    active.scroll = 430;
    const restored = restoreClock(JSON.stringify(active), ["mine"])!;
    expect(sessionAt(restored.progress, 31000).remainingMs).toBe(30000);
    expect(restored.url).toBe(url);
    expect(restored.scroll).toBe(430);
    expect(() => restoreClock(JSON.stringify(active), ["other"])).toThrow();
    expect(() =>
      restoreClock(
        JSON.stringify({ ...active, url: "https://evil.test/reading" }),
        ["mine"],
      ),
    ).toThrow();
    expect(() =>
      restoreClock(JSON.stringify({ ...active, scroll: -1 }), ["mine"]),
    ).toThrow();
  });
  it("rejects unrepresentable deadlines instead of narrowing time", () => {
    expect(() =>
      startClock(
        {
          ...initialSession({ smart: 1, random: 0 }),
          remainingMs: Number.MAX_SAFE_INTEGER,
        },
        1000,
      ),
    ).toThrow();
  });
});
