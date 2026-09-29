import { expect, test } from "@playwright/test";
test("home reading counts replace time without shifting controls", async ({
  page,
}) => {
  let release!: () => void;
  const gate = new Promise<void>((r) => {
    release = r;
  });
  let calls = 0;
  await page.route("**/api/workspaces/*/reading-activity?*", async (r) => {
    calls++;
    await gate;
    const day = await page.evaluate(() => {
      const now = new Date();
      return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    });
    await r.fulfill({ json: { days: [{ day, count: 17, arrived: 23 }] } });
  });
  await page.goto("/");
  const grid = page.getByRole("grid", { name: "Daily articles marked read" });
  await expect(grid).toHaveAttribute("aria-busy", "true");
  const feed = page.getByRole("button", { name: "Open feed", exact: true });
  const flow = page.getByRole("region", {
    name: "Articles received and read over the last 30 days",
  });
  const flowBefore = await flow.boundingBox();
  const before = {
    feed: await feed.boundingBox(),
    grid: await grid.boundingBox(),
  };
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  expect(calls).toBe(1);
  release();
  await expect(grid).toHaveAttribute("aria-busy", "false");
  await expect(
    page.getByRole("gridcell", { name: /: 17 articles$/ }),
  ).toBeVisible();
  expect(await feed.boundingBox()).toEqual(before.feed);
  expect(await grid.boundingBox()).toEqual(before.grid);
  expect(await flow.boundingBox()).toEqual(flowBefore);
  const day = page.getByRole("group", { name: /: 23 arrived, 17 read$/ });
  const dayBefore = await day.boundingBox();
  await day.hover();
  await expect(page.getByRole("tooltip")).toContainText(
    "Arrived: 23 · Read: 17",
  );
  expect(await day.boundingBox()).toEqual(dayBefore);
  expect(await grid.boundingBox()).toEqual(before.grid);
  await page.screenshot({ path: "/tmp/reader-flow-chart.png", fullPage: true });
});
