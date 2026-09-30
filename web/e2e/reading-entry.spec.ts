import { expect, test } from "@playwright/test";

test("reading entry buttons keep matching geometry and fit narrow panes", async ({
  page,
}) => {
  await page.goto("/reader");
  const primary = page.getByRole("button", {
    name: "Reading mode",
    exact: true,
  });
  const secondary = page.getByRole("button", {
    name: "One by one",
    exact: true,
  });
  for (const width of [1440, 390, 320]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(primary).toBeVisible();
    await expect(secondary).toBeVisible();
    const a = (await primary.boundingBox())!,
      b = (await secondary.boundingBox())!;
    expect(a.height).toBe(b.height);
    expect(a.y).toBe(b.y);
    expect(a.x + a.width).toBeLessThan(b.x);
    expect(b.x + b.width).toBeLessThanOrEqual(width);
    const geometry = await secondary.boundingBox();
    await primary.hover();
    expect(await secondary.boundingBox()).toEqual(geometry);
    await primary.focus();
    expect(await secondary.boundingBox()).toEqual(geometry);
  }
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.screenshot({ path: "/tmp/reading-entry-desktop.png" });
  await secondary.click();
  await expect(page).toHaveURL(/\/reading\?/);
});
