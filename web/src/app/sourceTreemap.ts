/** Deterministic binary treemap: exact proportional area, no hidden small sources.
 * Zero contributions have no area and remain in the accessible ranking. */
export type WeightedSource = { key: string; count: number };
export type SourceRectangle = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
};
export function sourceTreemap(
  values: WeightedSource[],
  width: number,
  height: number,
): SourceRectangle[] {
  if (
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  )
    throw new Error("Treemap dimensions must be positive");
  const keys = new Set<string>();
  for (const value of values) {
    if (
      !Number.isSafeInteger(value.count) ||
      value.count < 0 ||
      keys.has(value.key)
    )
      throw new Error(
        "Treemap sources require unique identities and nonnegative integer counts",
      );
    keys.add(value.key);
  }
  const rows = values
    .filter((value) => value.count > 0)
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key));
  const total = rows.reduce((sum, value) => sum + value.count, 0);
  if (!Number.isSafeInteger(total))
    throw new Error("Treemap contribution count exceeds exact numeric range");
  const rectangles: SourceRectangle[] = [];
  const partition = (
    start: number,
    end: number,
    sum: number,
    x: number,
    y: number,
    w: number,
    h: number,
  ) => {
    if (start === end) return;
    if (end - start === 1) {
      rectangles.push({ key: rows[start].key, x, y, width: w, height: h });
      return;
    }
    let split = start + 1,
      left = rows[start].count;
    while (
      split < end - 1 &&
      Math.abs(left + rows[split].count - sum / 2) < Math.abs(left - sum / 2)
    )
      left += rows[split++].count;
    const fraction = left / sum;
    if (w >= h) {
      partition(start, split, left, x, y, w * fraction, h);
      partition(
        split,
        end,
        sum - left,
        x + w * fraction,
        y,
        w * (1 - fraction),
        h,
      );
    } else {
      partition(start, split, left, x, y, w, h * fraction);
      partition(
        split,
        end,
        sum - left,
        x,
        y + h * fraction,
        w,
        h * (1 - fraction),
      );
    }
  };
  partition(0, rows.length, total, 0, 0, width, height);
  return rectangles;
}
