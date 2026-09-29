import { expect, test } from "@playwright/test";

test("reader navigation and article states", async ({ page }) => {
  await page.goto("/reader");
  await expect(
    page.getByRole("heading", { name: /^Feed \(\d+\)$/ }),
  ).toBeVisible();
  await page
    .getByRole("heading", {
      name: "The durable queue is the product",
      level: 2,
    })
    .click();
  await expect(
    page.getByRole("heading", {
      name: "The durable queue is the product",
      level: 1,
    }),
  ).toBeVisible();
  const reader = page.getByRole("article", { name: "Article reader" });
  await reader.getByRole("button", { name: "Read later" }).click();
  await expect(
    reader.getByRole("button", { name: "Remove from later" }),
  ).toBeVisible();
});

test("pause validates reason without moving the dialog footer", async ({
  page,
}) => {
  await page.goto("/reader");
  await page
    .getByRole("navigation", { name: "Subscriptions" })
    .getByRole("button", { name: /This Week in Rust/ })
    .click();
  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("button", { name: "Pause This Week in Rust" }).click();
  const submit = page.getByRole("button", { name: "Pause subscription" });
  const before = await submit.boundingBox();
  await page.getByLabel(/Reason/).focus();
  await page.getByLabel(/Reason/).blur();
  await expect(
    page.getByText("Enter a reason before continuing."),
  ).toBeVisible();
  expect(await submit.boundingBox()).toEqual(before);
  await page.getByLabel(/Reason/).fill("Reading backlog first");
  await submit.click();
  await expect(page.getByText("Subscription paused")).toBeVisible();
});

test("theme and responsive panels remain usable", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/reader");
  await page.getByRole("button", { name: "Use dark theme" }).click();
  await expect(page.locator(".app")).toHaveAttribute("data-theme", "dark");
  await page
    .getByRole("heading", {
      name: "Async Rust without the hidden machinery",
      level: 2,
    })
    .click();
  await expect(
    page.getByRole("heading", {
      name: "Async Rust without the hidden machinery",
      level: 1,
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Back to articles" }).click();
  await expect(
    page.getByRole("heading", { name: /^Feed \(\d+\)$/ }),
  ).toBeVisible();
});

test("keyboard shortcut opens the unified search page", async ({ page }) => {
  await page.route("**/api/search/limits", (r) =>
    r.fulfill({
      json: { query_bytes: 512, page_size: 25, excerpt_characters: 220 },
    }),
  );
  await page.goto("/reader");
  await expect(
    page.getByRole("button", { name: "Search", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Control+k");
  await expect(page).toHaveURL(/\/search$/);
  await expect(page.getByRole("searchbox")).toBeVisible();
});
