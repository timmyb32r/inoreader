import { wireFixture } from "./wire-fixtures.mjs";
import { expect, test } from "@playwright/test";

test("publication calendar keeps controls fixed through loading and every chart scale", async ({
  page,
}) => {
  const days = Array.from({ length: 270 }, (_, index) => {
    const date = new Date(Date.UTC(2026, 0, 1 + index))
      .toISOString()
      .slice(0, 10);
    return { date, count: index % 7 === 0 ? 0 : ((index * 7) % 13) + 1 };
  }).filter((day) => day.count > 0);
  let release!: () => void;
  const ready = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/publication-history", async (route) => {
    await ready;
    await route.fulfill({
      json: wireFixture({
        days: [{ date: "2025-03-01", count: 4 }, ...days],
        undated: 7,
        conflicting: 0,
      }),
    });
  });
  await page.goto("/subscriptions/sub");
  const chart = page.getByRole("region", {
    name: "Publication history",
    exact: true,
  });
  await chart.scrollIntoViewIfNeeded();
  await expect(chart).toHaveAttribute("aria-busy", "true");
  const before = await chart.boundingBox();
  const controls = chart.getByRole("group", { name: "Chart interval" });
  const controlsBefore = await controls.boundingBox();
  release();
  await expect(chart).toHaveAttribute("aria-busy", "false");
  expect(await chart.boundingBox()).toEqual(before);
  expect(await controls.boundingBox()).toEqual(controlsBefore);
  for (const scale of ["Days", "Years", "Months"]) {
    await chart.getByRole("button", { name: scale, exact: true }).click();
    await expect(
      chart.getByRole("button", { name: scale, exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    const bar = chart
      .getByRole("group", { name: `Articles by ${scale.toLowerCase()}` })
      .getByRole("button")
      .first();
    const barBefore = await bar.boundingBox();
    await bar.hover();
    expect(await chart.getByRole("tooltip").isVisible()).toBe(true);
    expect(await chart.getByRole("tooltip").textContent()).toBe(
      await bar.getAttribute("aria-label"),
    );
    expect(await bar.boundingBox()).toEqual(barBefore);
    expect(await controls.boundingBox()).toEqual(controlsBefore);
    expect(await chart.boundingBox()).toEqual(before);
  }
  await page.screenshot({
    path: "/tmp/inoreader-publication-history.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await chart.scrollIntoViewIfNeeded();
  expect(
    await chart.evaluate(
      (element) => element.scrollWidth <= element.clientWidth,
    ),
  ).toBe(true);
});
