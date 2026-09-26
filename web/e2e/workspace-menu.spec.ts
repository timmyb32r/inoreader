import { expect, test } from "@playwright/test";

test("collapsed workspace menu fits its content without moving navigation", async ({ page }) => {
  await page.route("**/api/bootstrap*", async route => {
    const response = await route.fetch();
    const data = await response.json();
    await route.fulfill({ json: { ...data, account: { ...data.account, id: "fixture" }, articlePage: { articles: data.articles, total: data.articles.length, unreadTotal: 2 } } });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  const trigger = page.locator(".workspace-button");
  const navigation = page.locator('.nav-block button').nth(1);
  const before = await navigation.boundingBox();
  await trigger.click();
  await expect(trigger).toHaveAttribute("aria-expanded", "true");
  const menu = page.locator(".workspace-menu");
  await expect(menu).toBeVisible();
  const bounds = await menu.boundingBox();
  expect(bounds!.width).toBeGreaterThanOrEqual(240);
  expect(await navigation.boundingBox()).toEqual(before);
  expect(await menu.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await expect(menu.getByRole("button", { name: "Archive workspace" })).toBeVisible();
  await trigger.click();
  await expect(menu).toHaveCount(0);
  expect(await navigation.boundingBox()).toEqual(before);
});
