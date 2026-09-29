import { localDay, readPeriodForDay } from "./readPeriod";
import {
  readArticlePagePosition,
  writeArticlePagePosition,
} from "./readerLocation";

describe("reading history location", () => {
  it("rejects malformed and overflowing calendar dates", () => {
    for (const day of ["invalid", "2026-02-30", "2026-13-01", "26-1-1"])
      expect(() => readPeriodForDay(day)).toThrow();
  });
  it("roundtrips the selected period and cursor without touching article identifiers", () => {
    const period = readPeriodForDay("2026-09-29");
    expect(localDay(new Date(period.from))).toBe("2026-09-29");
    expect(localDay(new Date(period.until))).toBe("2026-09-30");
    history.replaceState({}, "", "/reader");
    writeArticlePagePosition("feed", null, "cursor", "older", 2, period);
    expect(readArticlePagePosition()).toMatchObject({
      readPeriod: period,
      cursor: "cursor",
      batch: 2,
    });
    writeArticlePagePosition("feed", null);
    expect(readArticlePagePosition().readPeriod).toBeUndefined();
  });
});
