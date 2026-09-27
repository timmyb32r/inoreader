import type { Subscription } from "../api/viewModels";
import {
  compareSubscriptions,
  isProblem,
  readLayout,
  readSort,
  writeLayout,
  writeSort,
} from "./subscriptionCatalogModel";

const healthy: Subscription = {
  id: "a",
  name: "Alpha",
  status: "active",
  count: 1,
};
const problem: Subscription = {
  ...healthy,
  id: "b",
  name: "Bravo",
  needsAttention: true,
};

describe("subscription catalog contracts", () => {
  it("uses the same attention classification for indicators, sorting and filters", () => {
    expect(isProblem(healthy)).toBe(false);
    expect(isProblem({ ...healthy, error: "Transient timeout" })).toBe(false);
    for (const flags of [
      { needsAttention: true },
      { incomplete: true },
      { attentionReason: "Repeated failures" },
    ]) {
      const item = { ...healthy, id: problem.id, name: problem.name, ...flags };
      expect(isProblem(item)).toBe(true);
      expect(
        compareSubscriptions(item, healthy, "attention", "asc"),
      ).toBeLessThan(0);
      expect(
        compareSubscriptions(item, healthy, "attention", "desc"),
      ).toBeGreaterThan(0);
    }
  });
  it("adds the mandatory attention column without losing saved order or widths", () => {
    writeLayout("layout-existing", {
      columns: ["url", "name", "note"],
      widths: { name: 320, note: 240 },
    });
    const layout = readLayout("layout-existing");
    expect(layout.columns).toEqual(["url", "attention", "name", "note"]);
    expect(layout.widths).toEqual({ name: 320, note: 240 });
    writeLayout("layout-existing", layout);
    expect(readLayout("layout-existing")).toEqual(layout);
  });
  it("persists attention sorting only for its own workspace", () => {
    writeSort("sort-a", "attention", "desc");
    expect(readSort("sort-a")).toEqual({
      column: "attention",
      direction: "desc",
    });
    expect(readSort("sort-b")).toEqual({ column: "name", direction: "asc" });
  });
});
