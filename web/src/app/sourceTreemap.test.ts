import { describe, expect, it } from "vitest";
import { sourceTreemap } from "./sourceTreemap";
describe("source contribution treemap", () => {
  it("preserves proportional areas, bounds and every small source without overlaps", () => {
    const values = Array.from({ length: 211 }, (_, index) => ({
      key: String(index),
      count: index + 1,
    }));
    const total = values.reduce((sum, value) => sum + value.count, 0);
    const boxes = sourceTreemap(values, 1000, 500);
    expect(boxes).toHaveLength(values.length);
    for (const box of boxes) {
      expect((box.width * box.height) / 500000).toBeCloseTo(
        values.find((value) => value.key === box.key)!.count / total,
        12,
      );
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.y).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(1000.000001);
      expect(box.y + box.height).toBeLessThanOrEqual(500.000001);
      for (const other of boxes)
        if (box !== other)
          expect(
            Math.min(box.x + box.width, other.x + other.width) -
              Math.max(box.x, other.x) <=
              0.000001 ||
              Math.min(box.y + box.height, other.y + other.height) -
                Math.max(box.y, other.y) <=
                0.000001,
          ).toBe(true);
    }
    expect(sourceTreemap([...values].reverse(), 1000, 500)).toEqual(boxes);
  });
  it("handles empty and singleton sets and rejects invalid counts or duplicate identities", () => {
    expect(sourceTreemap([], 100, 50)).toEqual([]);
    expect(
      sourceTreemap(
        [
          { key: "a", count: 1 },
          { key: "b", count: 0 },
        ],
        100,
        50,
      ),
    ).toEqual([{ key: "a", x: 0, y: 0, width: 100, height: 50 }]);
    for (const count of [-1, NaN, Infinity, 0.5])
      expect(() => sourceTreemap([{ key: "a", count }], 100, 50)).toThrow();
    expect(() =>
      sourceTreemap(
        [
          { key: "a", count: 1 },
          { key: "a", count: 2 },
        ],
        100,
        50,
      ),
    ).toThrow();
  });
});
