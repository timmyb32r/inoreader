import { wireFixture } from "./wire-fixtures.mjs";
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/api/bootstrap*", async (route) => {
    const data = await (await route.fetch()).json();
    await route.fulfill({
      json: wireFixture({
        ...data,
        subscriptions: [
          ...data.subscriptions,
          {
            id: "broken",
            name: "Broken source",
            status: "active",
            count: 0,
            needsAttention: true,
            attentionReason: "Repeated fetch failures",
          },
        ],
      }),
    });
  });
});

test("latest article hover, keyboard focus and press highlight without moving targets", async ({
  page,
}) => {
  await page.goto("/subscriptions/sub");
  const row = page
    .getByRole("dialog", { name: "Subscription details", exact: true })
    .getByRole("button", { name: /Async Rust without the hidden machinery/ });
  await row.scrollIntoViewIfNeeded();
  const before = await row.boundingBox();
  const next = page.getByRole("button", { name: "View all articles" });
  const nextBefore = await next.boundingBox();
  const color = () =>
    row.evaluate((el) => getComputedStyle(el).backgroundColor);
  const idle = await color();
  await row.hover();
  expect(await color()).not.toBe(idle);
  await page.screenshot({ path: "/tmp/inoreader-latest-hover.png" });
  expect(await row.boundingBox()).toEqual(before);
  expect(await next.boundingBox()).toEqual(nextBefore);
  await page.mouse.down();
  expect(await row.boundingBox()).toEqual(before);
  expect(await next.boundingBox()).toEqual(nextBefore);
  await page.mouse.move(1, 1);
  await page.mouse.up();
  await next.focus();
  await page.keyboard.press("Tab");
  await expect(row).toBeFocused();
  expect(await color()).not.toBe(idle);
  expect(await row.boundingBox()).toEqual(before);
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("heading", {
      name: "Async Rust without the hidden machinery",
      level: 1,
    }),
  ).toBeVisible();
});

test("attention column is red and sorts problems to the top and back", async ({
  page,
}) => {
  await page.goto("/subscriptions");
  const header = page.getByRole("columnheader", { name: "Needs attention" });
  const rows = page.locator(".subscription-table tbody tr");
  await header.getByRole("button").click();
  await expect(header).toHaveAttribute("aria-sort", "ascending");
  await expect(rows.first()).toContainText("Broken source");
  const badge = rows.first().locator(".subscription-attention");
  await expect(badge).toHaveText("!Needs attention");
  expect(await badge.evaluate((el) => getComputedStyle(el).color)).toBe(
    "rgb(192, 75, 80)",
  );
  await header.getByRole("button").click();
  await expect(header).toHaveAttribute("aria-sort", "descending");
  await expect(rows.first()).toContainText("This Week in Rust");
  await page.reload();
  await expect(
    page.getByRole("columnheader", { name: "Needs attention" }),
  ).toHaveAttribute("aria-sort", "descending");
});

test("sidebar collapse spans its width and Settings keeps its target stable on hover", async ({
  page,
}) => {
  await page.goto("/reader");
  const sidebar = page.getByRole("complementary", {
    name: "Reader navigation",
  });
  const toggle = page.getByRole("button", { name: "Collapse sidebar" });
  const settings = page.getByRole("button", { name: "Settings", exact: true });
  await expect(settings).toBeVisible();
  const bounds = (await toggle.boundingBox())!;
  const sidebarBounds = (await sidebar.boundingBox())!;
  expect(bounds.width).toBeGreaterThanOrEqual(sidebarBounds.width - 2);
  const settingsBefore = await settings.boundingBox();
  await page.mouse.move(
    bounds.x + bounds.width - 8,
    bounds.y + bounds.height / 2,
  );
  expect(await settings.boundingBox()).toEqual(settingsBefore);
  await page.mouse.down();
  expect(await toggle.boundingBox()).toEqual(bounds);
  await page.mouse.up();
  await expect(
    page.getByRole("button", { name: "Expand sidebar" }),
  ).toHaveAttribute("aria-expanded", "false");
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(await settings.boundingBox()).toEqual(settingsBefore);
});

test("collapsed subscription count opens the catalog and returns to the same reader", async ({
  page,
}) => {
  await page.goto("/reader");
  const subscriptions = page.getByRole("button", {
    name: "Subscriptions 2",
    exact: true,
  });
  await expect(subscriptions).toContainText("Subscriptions");
  const badge = subscriptions.locator("em");
  const appearance = () =>
    badge.evaluate((el) => {
      const style = getComputedStyle(el);
      return {
        width: style.width,
        height: style.height,
        font: style.font,
        color: style.color,
        background: style.backgroundColor,
        border: style.border,
        radius: style.borderRadius,
        padding: style.padding,
      };
    });
  const expanded: Record<string, Awaited<ReturnType<typeof appearance>>> = {};
  expanded.light = await appearance();
  await page.getByRole("button", { name: "Use dark theme" }).click();
  expanded.dark = await appearance();
  await page.getByRole("button", { name: "Use light theme" }).click();
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(subscriptions).toBeVisible();
  await expect(subscriptions.locator("span")).toBeHidden();
  await expect(subscriptions.locator("em")).toHaveText("2");
  await expect(subscriptions).toHaveCSS("border-width", "0px");
  const before = await subscriptions.boundingBox();
  expect(before!.width).toBe(before!.height);
  const settings = page.getByRole("button", { name: "Settings", exact: true });
  const settingsBefore = await settings.boundingBox();
  for (const theme of ["light", "dark"]) {
    if (theme === "dark")
      await page.getByRole("button", { name: "Use dark theme" }).click();
    await page.mouse.move(0, 0);
    expect(await appearance()).toEqual(expanded[theme]);
    const idle = await badge.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    await subscriptions.hover();
    await expect(badge).not.toHaveCSS("background-color", idle);
    const hover = await badge.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    await page.mouse.down();
    await expect(badge).not.toHaveCSS("background-color", hover);
    expect(await subscriptions.boundingBox()).toEqual(before);
    expect(await settings.boundingBox()).toEqual(settingsBefore);
    await page.mouse.up();
    await expect(
      page.getByRole("dialog", { name: "Subscriptions", exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Close subscriptions" }).click();
    await expect(page).toHaveURL(
      (url) => url.pathname === "/reader" && url.searchParams.has("article"),
    );
    await expect(subscriptions).toBeFocused();
    expect(await subscriptions.boundingBox()).toEqual(before);
    expect(await settings.boundingBox()).toEqual(settingsBefore);
  }
  await subscriptions.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Subscriptions", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(subscriptions.locator("span")).toBeVisible();
});

test("favicon is a real multi-resolution ICO served from the public path", async ({
  page,
  request,
}) => {
  await page.goto("/reader");
  await expect(page.locator('link[rel="icon"]')).toHaveAttribute(
    "href",
    "/favicon.ico",
  );
  const response = await request.get("/favicon.ico");
  expect(response.status()).toBe(200);
  expect([...(await response.body()).subarray(0, 6)]).toEqual([
    0, 0, 1, 0, 4, 0,
  ]);
});
