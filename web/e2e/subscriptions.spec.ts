import { expect, test } from "@playwright/test";

test("subscription catalog navigates to durable details without the retired note editor", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^Subscriptions \d+$/ }).click();
  await expect(page).toHaveURL(/\/subscriptions$/);
  await expect(
    page.getByRole("heading", { name: "Subscriptions" }),
  ).toBeVisible();

  const search = page.getByRole("searchbox", { name: "Search subscriptions" });
  await search.fill("This Week");
  await expect(
    page.getByRole("link", { name: "This Week in Rust" }),
  ).toBeVisible();
  await search.fill("not present");
  await expect(page.getByText("No subscriptions in this view")).toBeVisible();
  await search.fill("");

  await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url().endsWith("/api/subscriptions/sub") &&
        response.request().method() === "GET",
    ),
    page.getByRole("link", { name: "This Week in Rust" }).click(),
  ]);
  await expect(page).toHaveURL(/\/subscriptions\/sub$/);
  await expect(
    page.getByRole("textbox", { name: "Personal note" }),
  ).toHaveCount(0);
  await expect(page.getByText("Wiki page", { exact: true })).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("textbox", { name: "Personal note" }),
  ).toHaveCount(0);
});

test("latest articles query owns its subscription scope and keeps controls fixed while loading", async ({
  page,
}) => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let requested = false;
  await page.route("**/api/articles?**", async (route) => {
    const url = new URL(route.request().url());
    if (url.searchParams.get("view") !== "subscription")
      return route.continue();
    expect(url.searchParams.get("subscription_id")).toBe("sub");
    expect(url.searchParams.has("cursor")).toBe(false);
    requested = true;
    await gate;
    return route.continue();
  });
  await page.goto("/subscriptions/sub");
  const content = page.locator(".recent-articles-content");
  await expect(content).toHaveAttribute("aria-busy", "true");
  await expect(content).toContainText("Loading latest articles");
  expect(requested).toBe(true);
  const open = page.getByRole("button", { name: "View all articles" });
  await open.scrollIntoViewIfNeeded();
  const before = await open.boundingBox();
  const region = await content.boundingBox();
  release();
  await expect(content).toHaveAttribute("aria-busy", "false");
  await expect(content).toContainText(
    "Async Rust without the hidden machinery",
  );
  expect(await open.boundingBox()).toEqual(before);
  expect(await content.boundingBox()).toEqual(region);
});

test("full text retry responds immediately, stays fixed and admits only one request", async ({
  page,
}) => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let calls = 0;
  await page.route("**/api/articles/2/full-text/refresh?**", async (route) => {
    calls++;
    await gate;
    await route.fulfill({ status: 204 });
  });
  await page.goto("/reader");
  await page
    .getByRole("heading", {
      name: "The durable queue is the product",
      level: 2,
    })
    .click();
  const retry = page.getByRole("button", { name: "Try again" });
  await retry.scrollIntoViewIfNeeded();
  const before = await retry.boundingBox();
  const toolbar = page.locator(".reader-toolbar");
  const toolbarBefore = await toolbar.boundingBox();
  await retry.click();
  await expect(retry).toHaveAttribute("aria-busy", "true");
  await expect(retry).toBeDisabled();
  expect(await retry.boundingBox()).toEqual(before);
  expect(await toolbar.boundingBox()).toEqual(toolbarBefore);
  await retry.dispatchEvent("click");
  await expect.poll(() => calls).toBe(1);
  release();
  await expect(retry).toBeEnabled();
  expect(await retry.boundingBox()).toEqual(before);
  expect(await toolbar.boundingBox()).toEqual(toolbarBefore);
});
