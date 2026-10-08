import { sumUsd, periodDates, validDates } from "./requestStatistics";
it("retains exact decimal dollars without binary floats", () => {
  expect(sumUsd(["0.123456789", "0.000000001", "3"])).toBe("3.123456790");
  expect(sumUsd([])).toBe("0");
});
it("includes empty periods and crosses month/year boundaries", () => {
  expect(periodDates("2025-12-31", "2026-01-02", "day")).toEqual([
    "2025-12-31",
    "2026-01-01",
    "2026-01-02",
  ]);
  expect(periodDates("2025-12-31", "2026-02-01", "month")).toEqual([
    "2025-12-01",
    "2026-01-01",
    "2026-02-01",
  ]);
  expect(periodDates("2025-12-31", "2026-02-01", "year")).toEqual([
    "2025-01-01",
    "2026-01-01",
  ]);
});
it("rejects invalid and reversed dates without rewriting them", () => {
  expect(validDates("2026-02-30", "2026-03-01")).toBe(false);
  expect(validDates("2026-03-01", "2026-02-01")).toBe(false);
  expect(validDates("2026-02-01", "2026-02-01")).toBe(true);
});
