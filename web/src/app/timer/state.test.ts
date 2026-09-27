import { describe, expect, it } from "vitest";
import { formatDuration, parseDuration, readTimer, remaining } from "./state";
describe("session countdown contract", () => {
  it("uses a wall-clock deadline and stops at zero", () => {
    const state = readTimer(
      '{"mode":"running","duration":60000,"deadline":120000}',
    );
    expect(remaining(state, 61000)).toBe(59000);
    expect(remaining(state, 180000)).toBe(0);
    expect(formatDuration(59001)).toBe("00:01:00");
  });
  it("preserves paused milliseconds across reload", () => {
    const state = readTimer(
      '{"mode":"paused","duration":60000,"remaining":55123}',
    );
    expect(remaining(state, 999999)).toBe(55123);
  });
  it("validates editor and stored state without coercion", () => {
    expect(parseDuration("01:00:00")).toBe(3600000);
    for (const value of [
      "00:00:00",
      "01:60:00",
      "-1:00:00",
      "1:00:00",
      "100:00:00",
    ])
      expect(() => parseDuration(value)).toThrow();
    for (const value of [
      "{}",
      "null",
      '{"mode":"paused","duration":1000,"remaining":2000}',
      '{"mode":"running","duration":1000,"deadline":"123"}',
      '{"mode":"idle","duration":1}',
    ])
      expect(() => readTimer(value)).toThrow();
  });
});
