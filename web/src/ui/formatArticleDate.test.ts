import { describe, expect, it } from "vitest";
import { formatArticleDate } from "./formatArticleDate";

describe("publication date precision", () => {
  it("keeps date-only and timezone-less declarations without inventing a UTC instant", () => {
    expect(formatArticleDate("2026-09-27")).toBe("2026-sep-27");
    expect(formatArticleDate("2026-09-27T10:02:03")).toBe(
      "2026-sep-27 10:02:03",
    );
    expect(formatArticleDate("2026-09-27T10:02:03+03:00")).toBe(
      "2026-sep-27 07:02:03",
    );
  });
});
