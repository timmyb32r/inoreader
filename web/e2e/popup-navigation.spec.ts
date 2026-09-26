import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/api/bootstrap*", async route => {
    const data = await (await route.fetch()).json();
    await route.fulfill({ json: { ...data, account: { ...data.account, id: "fixture" }, articlePage: { articles: data.articles, total: data.articles.length, unreadTotal: 2 } } });
  });
  await page.route("**/api/articles?*", async route => {
    const articles = await (await route.fetch()).json();
    await route.fulfill({ json: { articles, total: articles.length, unreadTotal: 2 } });
  });
});

test("direct subscription popup returns to the same reader with stable geometry", async ({ page }) => {
  await page.goto("/reader");
  await page.getByRole("navigation", { name: "Subscriptions", exact: true }).getByRole("button", { name: /This Week in Rust/ }).click();
  const heading = page.getByRole("link", { name: "Open settings for This Week in Rust" });
  await expect(page).toHaveURL(/subscription=sub/);
  const url = page.url();
  const bounds = await heading.boundingBox();
  await heading.click();
  await expect(page.getByRole("dialog", { name: "Subscription details", exact: true })).toBeVisible();
  expect(await heading.boundingBox()).toEqual(bounds);
  await page.getByRole("button", { name: "Close subscriptions" }).click();
  await expect(page).toHaveURL(url);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await heading.boundingBox()).toEqual(bounds);
  await expect(heading).toBeFocused();
  await heading.click();
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(url);
});

test("catalog returns to Home and nested dialogs close only one level", async ({ page }) => {
  await page.goto("/");
  const home = page.getByRole("button", { name: "Home", exact: true });
  const bounds = await home.boundingBox();
  await page.getByRole("button", { name: /^Subscriptions \d/ }).click();
  await page.getByRole("searchbox", { name: "Search subscriptions" }).fill("Rust");
  await page.getByRole("link", { name: "This Week in Rust", exact: true }).click();
  await page.getByRole("button", { name: "Edit name" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Edit custom name", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "Subscription details", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("searchbox", { name: "Search subscriptions" })).toHaveValue("Rust");
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL("/");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await home.boundingBox()).toEqual(bounds);
});
